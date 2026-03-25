//! The `voice` module implements polyphonic synthesis and sample playback.
//!
//! A `Voice` represents a single active audio grain or sample slice being rendered.
//! This module handles the per-voice DSP operations including variable-rate resampling,
//! ADSR envelopes, panning, and basic filtering (HPF/LPF).

use core::f32::consts::TAU;

use crate::SampleTrigger;
use crate::routing::TrackId;
use crate::sample_bank::PlaybackSample;
use crate::synth::{AnalogVoice, AnalogVoiceParams, OscShape};

const MAX_SAMPLE_EDGE_RAMP_FRAMES: u32 = 32;
const ANALOG_BASE_FREQUENCY_HZ: f32 = 220.0;
const ANALOG_OUTPUT_TRIM: f32 = 0.35;

/// Built-in synthesized drum voices used by the current live playback path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceKind {
    /// Synthesized bass drum placeholder for `bd`.
    KickLike,
    /// Synthesized snare placeholder for `sn`.
    SnareLike,
    /// Synthesized clap placeholder for `cp`.
    ClapLike,
    /// Synthesized hi-hat placeholder for `hh`.
    HiHatLike,
    /// Analog saw voice placeholder for `saw`.
    AnalogSaw,
    /// Analog pulse voice placeholder for `pulse`.
    AnalogPulse,
    /// Analog triangle voice placeholder for `tri`.
    AnalogTri,
    /// Analog noise voice placeholder for `noise`.
    AnalogNoise,
}

impl VoiceKind {
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::KickLike => "bd",
            Self::SnareLike => "sn",
            Self::ClapLike => "cp",
            Self::HiHatLike => "hh",
            Self::AnalogSaw => "saw",
            Self::AnalogPulse => "pulse",
            Self::AnalogTri => "tri",
            Self::AnalogNoise => "noise",
        }
    }

    /// Resolves a phase-one drum token to the current synthesized fallback.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "bd" => Some(Self::KickLike),
            "sn" => Some(Self::SnareLike),
            "cp" => Some(Self::ClapLike),
            "hh" => Some(Self::HiHatLike),
            "saw" => Some(Self::AnalogSaw),
            "pulse" => Some(Self::AnalogPulse),
            "tri" => Some(Self::AnalogTri),
            "noise" => Some(Self::AnalogNoise),
            _ => None,
        }
    }

    const fn is_drum_voice(self) -> bool {
        matches!(
            self,
            Self::KickLike | Self::SnareLike | Self::ClapLike | Self::HiHatLike
        )
    }
}

#[derive(Clone, Debug)]
pub struct ActiveVoice {
    track_id: TrackId,
    state: ActiveVoiceState,
    left_gain: f64,
    right_gain: f64,
}

#[derive(Clone, Debug)]
enum ActiveVoiceState {
    DrumSynth {
        kind: VoiceKind,
        frame_index: u32,
        duration_frames: u32,
        sample_rate_hz: f64,
        noise_state: u32,
    },
    AnalogSynth {
        voice: AnalogVoice,
        params: AnalogVoiceParams,
        frame_index: u32,
        duration_frames: u32,
    },
    Sample {
        frames: std::sync::Arc<[f32]>,
        frame_position: f64,
        frame_step: f64,
        frame_limit: f64,
        gain: f64,
        high_pass: Option<OnePoleHighPass>,
        low_pass: Option<OnePoleLowPass>,
        rendered_frames: u32,
        total_output_frames: u32,
        edge_ramp_frames: u32,
    },
}

impl ActiveVoice {
    #[allow(clippy::cast_precision_loss)]
    pub fn from_trigger(
        track_id: TrackId,
        kind: VoiceKind,
        sample_rate: u32,
        trigger: &SampleTrigger,
        duration_frames: u32,
    ) -> Self {
        let (left_gain, right_gain) = stereo_gains_for_pan(trigger.pan());

        if kind.is_drum_voice() {
            return Self {
                track_id,
                state: ActiveVoiceState::DrumSynth {
                    kind,
                    frame_index: 0,
                    duration_frames: drum_duration_frames(kind, sample_rate),
                    sample_rate_hz: f64::from(sample_rate),
                    noise_state: 0x00C0_FFEE_u32,
                },
                left_gain,
                right_gain,
            };
        }

        Self {
            track_id,
            state: ActiveVoiceState::AnalogSynth {
                voice: AnalogVoice::new(sample_rate as f32),
                params: analog_voice_params(kind, trigger),
                frame_index: 0,
                duration_frames: duration_frames.max(1),
            },
            left_gain,
            right_gain,
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn from_sample(
        track_id: TrackId,
        sample: &PlaybackSample,
        output_sample_rate: u32,
        trigger: &SampleTrigger,
    ) -> Self {
        let frame_count_u32 = u32::try_from(sample.frames().len())
            .unwrap_or_else(|_| panic!("sample frame count exceeded supported playback range"));
        let frame_count = f64::from(frame_count_u32);
        let slice_start = trigger.slice_start() * frame_count;
        let slice_end = trigger.slice_end() * frame_count;
        let frame_step =
            (f64::from(sample.sample_rate_hz()) / f64::from(output_sample_rate)) * trigger.rate();
        let output_frame_count = if frame_step.abs() <= f64::EPSILON {
            1
        } else {
            (((slice_end - slice_start) / frame_step.abs()).ceil()).clamp(1.0, f64::from(u32::MAX))
                as u32
        };
        let edge_ramp_frames = output_frame_count
            .div_ceil(2)
            .clamp(1, MAX_SAMPLE_EDGE_RAMP_FRAMES);
        let (left_gain, right_gain) = stereo_gains_for_pan(trigger.pan());
        let (frame_position, frame_limit) = if frame_step.is_sign_negative() {
            (slice_end, slice_start)
        } else {
            (slice_start, slice_end)
        };
        Self {
            track_id,
            state: ActiveVoiceState::Sample {
                frames: sample.frames().clone(),
                frame_position,
                frame_step,
                frame_limit,
                gain: trigger.gain(),
                high_pass: trigger
                    .hpf_cutoff_hz()
                    .map(|cutoff_hz| OnePoleHighPass::new(output_sample_rate, cutoff_hz)),
                low_pass: trigger
                    .lpf_cutoff_hz()
                    .map(|cutoff_hz| OnePoleLowPass::new(output_sample_rate, cutoff_hz)),
                rendered_frames: 0,
                total_output_frames: output_frame_count,
                edge_ramp_frames,
            },
            left_gain,
            right_gain,
        }
    }

    #[must_use]
    pub const fn track_id(&self) -> TrackId {
        self.track_id
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn next_mono_sample(&mut self) -> Option<f32> {
        match &mut self.state {
            ActiveVoiceState::DrumSynth {
                kind,
                frame_index,
                duration_frames,
                sample_rate_hz,
                noise_state,
            } => {
                if *frame_index >= *duration_frames {
                    return None;
                }

                let progress = f64::from(*frame_index) / f64::from(*duration_frames);
                let time = f64::from(*frame_index) / *sample_rate_hz;
                let envelope = 1.0 - progress;
                let sample = match kind {
                    VoiceKind::KickLike => {
                        let frequency = (-120.0_f64).mul_add(progress, 160.0);
                        (time * frequency * f64::from(TAU)).sin() * envelope.powi(3) * 0.85
                    }
                    VoiceKind::SnareLike => next_noise(noise_state) * envelope.powi(2) * 0.65,
                    VoiceKind::ClapLike => {
                        let burst = if progress < 0.12 || (0.2..0.32).contains(&progress) {
                            1.0
                        } else {
                            0.5
                        };
                        next_noise(noise_state) * envelope.powi(2) * burst * 0.55
                    }
                    VoiceKind::HiHatLike => {
                        next_noise(noise_state).signum() * envelope.powi(2) * 0.35
                    }
                    VoiceKind::AnalogSaw
                    | VoiceKind::AnalogPulse
                    | VoiceKind::AnalogTri
                    | VoiceKind::AnalogNoise => {
                        unreachable!("analog voices should not enter the drum synth path")
                    }
                };

                *frame_index = frame_index.saturating_add(1);
                Some(sample as f32)
            }
            ActiveVoiceState::AnalogSynth {
                voice,
                params,
                frame_index,
                duration_frames,
            } => {
                if *frame_index >= *duration_frames {
                    return None;
                }

                let envelope = sample_edge_envelope(
                    *frame_index,
                    *duration_frames,
                    synth_edge_ramp_frames(*duration_frames),
                ) as f32;
                let sample = voice.next_sample(params) * envelope;
                *frame_index = frame_index.saturating_add(1);
                Some(sample)
            }
            ActiveVoiceState::Sample {
                frames,
                frame_position,
                frame_step,
                frame_limit,
                gain,
                high_pass,
                low_pass,
                rendered_frames,
                total_output_frames,
                edge_ramp_frames,
            } => {
                if *rendered_frames >= *total_output_frames {
                    return None;
                }
                let index = if frame_step.is_sign_negative() {
                    if *frame_position <= *frame_limit {
                        return None;
                    }
                    (frame_position.ceil() as usize).checked_sub(1)?
                } else {
                    if *frame_position >= *frame_limit {
                        return None;
                    }
                    frame_position.floor() as usize
                };
                let mut sample = f64::from(*frames.get(index)?) * *gain;
                if let Some(filter) = low_pass {
                    sample = filter.process(sample);
                }
                if let Some(filter) = high_pass {
                    sample = filter.process(sample);
                }
                let envelope =
                    sample_edge_envelope(*rendered_frames, *total_output_frames, *edge_ramp_frames);
                let sample = (sample * envelope) as f32;
                *frame_position += *frame_step;
                *rendered_frames = rendered_frames.saturating_add(1);
                Some(sample)
            }
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn next_stereo_frame(&mut self) -> Option<(f32, f32)> {
        let sample = self.next_mono_sample()?;
        Some((
            (f64::from(sample) * self.left_gain) as f32,
            (f64::from(sample) * self.right_gain) as f32,
        ))
    }
}

fn next_noise(noise_state: &mut u32) -> f64 {
    *noise_state = noise_state
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223);
    let normalized = f64::from((*noise_state >> 8) & 0x00FF_FFFF) / 16_777_215.0;
    (normalized * 2.0) - 1.0
}

fn stereo_gains_for_pan(pan: f64) -> (f64, f64) {
    let pan = pan.clamp(-1.0, 1.0);
    let left = if pan > 0.0 { 1.0 - pan } else { 1.0 };
    let right = if pan < 0.0 { 1.0 + pan } else { 1.0 };
    (left, right)
}

const fn drum_duration_frames(kind: VoiceKind, sample_rate: u32) -> u32 {
    match kind {
        VoiceKind::KickLike => sample_rate / 3,
        VoiceKind::SnareLike => sample_rate / 5,
        VoiceKind::ClapLike => sample_rate / 6,
        VoiceKind::HiHatLike => sample_rate / 8,
        VoiceKind::AnalogSaw
        | VoiceKind::AnalogPulse
        | VoiceKind::AnalogTri
        | VoiceKind::AnalogNoise => 1,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn analog_voice_params(kind: VoiceKind, trigger: &SampleTrigger) -> AnalogVoiceParams {
    AnalogVoiceParams {
        osc_shape: match kind {
            VoiceKind::AnalogSaw => OscShape::Saw,
            VoiceKind::AnalogPulse => OscShape::Pulse,
            VoiceKind::AnalogTri => OscShape::Tri,
            VoiceKind::AnalogNoise => OscShape::Noise,
            VoiceKind::KickLike
            | VoiceKind::SnareLike
            | VoiceKind::ClapLike
            | VoiceKind::HiHatLike => {
                unreachable!("drum voices do not produce analog voice parameters")
            }
        },
        freq_hz: analog_frequency_hz(trigger),
        pulse_width: sanitize_pulse_width(trigger.pulse_width()),
        cutoff_hz: trigger.lpf_cutoff_hz().unwrap_or(1_200.0) as f32,
        resonance: sanitize_unit_f32(trigger.resonance(), 0.2),
        drive: sanitize_non_negative_f32(trigger.drive(), 1.0),
        gain: sanitize_non_negative_f32(trigger.gain(), 1.0) * ANALOG_OUTPUT_TRIM,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn analog_frequency_hz(trigger: &SampleTrigger) -> f32 {
    let rate = if trigger.rate().is_finite() {
        trigger.rate().abs() as f32
    } else {
        1.0
    };
    (ANALOG_BASE_FREQUENCY_HZ * rate).max(0.0)
}

#[allow(clippy::cast_possible_truncation, clippy::missing_const_for_fn)]
fn sanitize_unit_f32(value: f64, default: f32) -> f32 {
    if value.is_finite() {
        (value as f32).clamp(0.0, 1.0)
    } else {
        default
    }
}

#[allow(clippy::cast_possible_truncation, clippy::missing_const_for_fn)]
fn sanitize_non_negative_f32(value: f64, default: f32) -> f32 {
    if value.is_finite() {
        (value as f32).max(0.0)
    } else {
        default
    }
}

#[allow(clippy::cast_possible_truncation, clippy::missing_const_for_fn)]
fn sanitize_pulse_width(value: f64) -> f32 {
    if value.is_finite() {
        (value as f32).clamp(0.01, 0.99)
    } else {
        0.5
    }
}

fn sample_edge_envelope(frame_index: u32, total_frames: u32, ramp_frames: u32) -> f64 {
    let attack = normalized_edge_gain(frame_index, ramp_frames);
    let release = normalized_edge_gain(
        total_frames.saturating_sub(frame_index.saturating_add(1)),
        ramp_frames,
    );
    attack.min(release)
}

fn normalized_edge_gain(distance_from_edge: u32, ramp_frames: u32) -> f64 {
    ((f64::from(distance_from_edge) + 0.5) / f64::from(ramp_frames)).min(1.0)
}

const fn synth_edge_ramp_frames(total_frames: u32) -> u32 {
    let half_frames = total_frames.div_ceil(2);
    if half_frames == 0 {
        1
    } else if half_frames > MAX_SAMPLE_EDGE_RAMP_FRAMES {
        MAX_SAMPLE_EDGE_RAMP_FRAMES
    } else {
        half_frames
    }
}

#[derive(Clone, Copy, Debug)]
struct OnePoleLowPass {
    alpha: f64,
    state: f64,
}

impl OnePoleLowPass {
    fn new(sample_rate_hz: u32, cutoff_hz: f64) -> Self {
        let omega = (core::f64::consts::TAU * normalized_cutoff_hz(cutoff_hz, sample_rate_hz))
            / f64::from(sample_rate_hz);
        Self {
            alpha: omega / (1.0 + omega),
            state: 0.0,
        }
    }

    fn process(&mut self, input: f64) -> f64 {
        self.state += self.alpha * (input - self.state);
        self.state
    }
}

#[derive(Clone, Copy, Debug)]
struct OnePoleHighPass {
    alpha: f64,
    previous_input: f64,
    previous_output: f64,
}

impl OnePoleHighPass {
    fn new(sample_rate_hz: u32, cutoff_hz: f64) -> Self {
        let omega = (core::f64::consts::TAU * normalized_cutoff_hz(cutoff_hz, sample_rate_hz))
            / f64::from(sample_rate_hz);
        Self {
            alpha: 1.0 / (1.0 + omega),
            previous_input: 0.0,
            previous_output: 0.0,
        }
    }

    fn process(&mut self, input: f64) -> f64 {
        let output = self.alpha * (self.previous_output + input - self.previous_input);
        self.previous_input = input;
        self.previous_output = output;
        output
    }
}

fn normalized_cutoff_hz(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let nyquist = (f64::from(sample_rate_hz) / 2.0) - 1.0;
    cutoff_hz.clamp(1.0, nyquist.max(1.0))
}
