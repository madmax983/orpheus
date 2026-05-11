//! The `voice` module implements polyphonic synthesis and sample playback.
//!
//! A `Voice` represents a single active audio grain or sample slice being rendered.
//! This module handles the per-voice DSP operations including variable-rate resampling,
//! ADSR envelopes, panning, and basic filtering (HPF/LPF).

use core::f32::consts::TAU;

use crate::SampleTrigger;
use crate::effects::ReverbState;
use crate::pedal::PedalInstance;
use crate::routing::ReverbSpec;
use crate::routing::TrackId;
use crate::sample_bank::PlaybackSample;
use crate::synth::{AnalogVoice, AnalogVoiceParams, OscShape};

const MAX_SAMPLE_EDGE_RAMP_FRAMES: u32 = 32;
/// Default reference frequency for analog-voice playback rate = 1.0.
///
/// A3 (220 Hz) historically — kept stable so existing analog-voice renders
/// reproduce bit-for-bit. Override at runtime with
/// [`crate::EngineCommand::SetReferenceFrequency`].
pub const DEFAULT_ANALOG_BASE_FREQUENCY_HZ: f32 = 220.0;
const ANALOG_OUTPUT_TRIM: f32 = 0.35;
const INSERT_CHORUS_BUFFER_FRAMES: usize = 64;
const INSERT_REVERB_MAX_COMB_LENGTH: u32 = 307;
const INSERT_DECAY_REPEAT_CAP: u32 = 32;

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
    /// The canonical string identifier bridging the language REPL to the DSP engine.
    ///
    /// When users type `"bd"` or `"sn"` in the Orpheus language environment, the
    /// evaluator embeds these strings into the resulting playback sequence. The `SampleBank`
    /// maps these specific string tokens to this enum, enabling the audio engine to dispatch
    /// rendering to the correct fast-path synthesizer voice without executing expensive string
    /// comparisons on the audio thread.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::VoiceKind;
    ///
    /// assert_eq!(VoiceKind::KickLike.token(), "bd");
    /// assert_eq!(VoiceKind::SnareLike.token(), "sn");
    /// ```
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

#[derive(Debug)]
pub struct ActiveVoice {
    track_id: TrackId,
    state: ActiveVoiceState,
    pedal: Option<PedalInstance>,
    insert_effects: InsertEffectsState,
    tail_frames_remaining: u32,
    left_gain: f64,
    right_gain: f64,
}

#[derive(Debug)]
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
        frames_per_cycle: u64,
        trigger: &SampleTrigger,
        duration_frames: u32,
        base_hz: f32,
    ) -> Self {
        let (left_gain, right_gain) = stereo_gains_for_pan(trigger.pan());
        let pedal = trigger
            .pedal_program()
            .cloned()
            .map(|program| PedalInstance::new(program, sample_rate as f32));
        let insert_effects =
            InsertEffectsState::from_trigger(trigger, sample_rate, frames_per_cycle);
        let tail_frames_remaining = pedal
            .as_ref()
            .map_or(0, PedalInstance::tail_frames)
            .saturating_add(insert_effects.tail_frames());

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
                pedal,
                insert_effects,
                tail_frames_remaining,
                left_gain,
                right_gain,
            };
        }

        Self {
            track_id,
            state: ActiveVoiceState::AnalogSynth {
                voice: AnalogVoice::new(sample_rate as f32),
                params: analog_voice_params(kind, trigger, base_hz),
                frame_index: 0,
                duration_frames: duration_frames.max(1),
            },
            pedal,
            insert_effects,
            tail_frames_remaining,
            left_gain,
            right_gain,
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn from_sample(
        track_id: TrackId,
        sample: &PlaybackSample,
        output_sample_rate: u32,
        frames_per_cycle: u64,
        trigger: &SampleTrigger,
    ) -> Self {
        let frame_count_u32 = u32::try_from(sample.frames().len())
            .unwrap_or_else(|_| panic!("sample frame count exceeded supported playback range"));
        let frame_count = f64::from(frame_count_u32);
        let slice_start = snap_slice_boundary(trigger.slice_start() * frame_count);
        let slice_end = snap_slice_boundary(trigger.slice_end() * frame_count);
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
        let pedal = trigger
            .pedal_program()
            .cloned()
            .map(|program| PedalInstance::new(program, output_sample_rate as f32));
        let insert_effects =
            InsertEffectsState::from_trigger(trigger, output_sample_rate, frames_per_cycle);
        let tail_frames_remaining = pedal
            .as_ref()
            .map_or(0, PedalInstance::tail_frames)
            .saturating_add(insert_effects.tail_frames());
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
            pedal,
            insert_effects,
            tail_frames_remaining,
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
        let dry_sample = self.next_mono_sample();
        if dry_sample.is_none() && self.tail_frames_remaining == 0 {
            return None;
        }

        let mono_sample = if let Some(sample) = dry_sample {
            sample
        } else {
            self.tail_frames_remaining = self.tail_frames_remaining.saturating_sub(1);
            0.0
        };
        let mono_sample = self
            .pedal
            .as_mut()
            .map_or(mono_sample, |pedal| pedal.process_sample(mono_sample));
        let dry_left = (f64::from(mono_sample) * self.left_gain) as f32;
        let dry_right = (f64::from(mono_sample) * self.right_gain) as f32;

        Some(self.insert_effects.process_frame(dry_left, dry_right))
    }
}

#[derive(Debug, Default)]
struct InsertEffectsState {
    chorus: Option<InsertChorusState>,
    delay: Option<InsertDelayState>,
    reverb: Option<InsertReverbState>,
    compressor: Option<InsertCompressorState>,
}

impl InsertEffectsState {
    #[allow(clippy::cast_precision_loss)]
    fn from_trigger(trigger: &SampleTrigger, sample_rate: u32, frames_per_cycle: u64) -> Self {
        Self {
            chorus: InsertChorusState::new(trigger, sample_rate),
            delay: InsertDelayState::new(trigger, frames_per_cycle),
            reverb: InsertReverbState::new(trigger),
            compressor: InsertCompressorState::new(trigger),
        }
    }

    fn tail_frames(&self) -> u32 {
        [
            self.chorus
                .as_ref()
                .map_or(0, InsertChorusState::tail_frames),
            self.delay.as_ref().map_or(0, InsertDelayState::tail_frames),
            self.reverb
                .as_ref()
                .map_or(0, InsertReverbState::tail_frames),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }

    fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let mut frame = (input_left, input_right);
        if let Some(chorus) = self.chorus.as_mut() {
            frame = chorus.process_frame(frame.0, frame.1);
        }
        if let Some(delay) = self.delay.as_mut() {
            frame = delay.process_frame(frame.0, frame.1);
        }
        if let Some(reverb) = self.reverb.as_mut() {
            frame = reverb.process_frame(frame.0, frame.1);
        }
        if let Some(compressor) = self.compressor.as_mut() {
            frame = compressor.process_frame(frame.0, frame.1);
        }
        frame
    }
}

#[derive(Debug)]
struct InsertDelayState {
    buffer: Vec<(f32, f32)>,
    write_index: usize,
    feedback: f32,
    mix: f32,
    tail_frames: u32,
}

impl InsertDelayState {
    fn new(trigger: &SampleTrigger, frames_per_cycle: u64) -> Option<Self> {
        let mix = sanitize_unit_f32(trigger.delay_mix(), 0.0);
        if mix <= f32::EPSILON {
            return None;
        }

        let delay_frames = cycle_fraction_to_frames(trigger.delay_time(), frames_per_cycle)?;
        let feedback = sanitize_unit_f32(trigger.delay_feedback(), 0.35);
        let repeat_count = decay_repeat_count(feedback);
        let buffer_len = usize::try_from(delay_frames).ok()?;

        Some(Self {
            buffer: vec![(0.0, 0.0); buffer_len],
            write_index: 0,
            feedback,
            mix,
            tail_frames: delay_frames.saturating_mul(repeat_count),
        })
    }

    const fn tail_frames(&self) -> u32 {
        self.tail_frames
    }

    fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let (delayed_left, delayed_right) = self.buffer[self.write_index];
        self.buffer[self.write_index] = (
            delayed_left.mul_add(self.feedback, input_left),
            delayed_right.mul_add(self.feedback, input_right),
        );
        self.write_index += 1;
        if self.write_index == self.buffer.len() {
            self.write_index = 0;
        }

        (
            blend_dry_wet(input_left, delayed_left, self.mix),
            blend_dry_wet(input_right, delayed_right, self.mix),
        )
    }
}

#[derive(Debug)]
struct InsertReverbState {
    state: ReverbState,
    mix: f32,
    tail_frames: u32,
}

impl InsertReverbState {
    fn new(trigger: &SampleTrigger) -> Option<Self> {
        let mix = sanitize_unit_f32(trigger.reverb_mix(), 0.0);
        if mix <= f32::EPSILON {
            return None;
        }

        let room = sanitize_unit_f32(trigger.reverb_room(), 0.75);
        let damp = sanitize_unit_f32(trigger.reverb_damp(), 0.35);
        let feedback = room.mul_add(0.55, 0.35);

        Some(Self {
            state: ReverbState::new(&ReverbSpec::new(room, damp, 1.0)),
            mix,
            tail_frames: INSERT_REVERB_MAX_COMB_LENGTH.saturating_mul(decay_repeat_count(feedback)),
        })
    }

    const fn tail_frames(&self) -> u32 {
        self.tail_frames
    }

    fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let (wet_left, wet_right) = self.state.process_frame(input_left, input_right);
        (
            blend_dry_wet(input_left, wet_left, self.mix),
            blend_dry_wet(input_right, wet_right, self.mix),
        )
    }
}

#[derive(Debug)]
struct InsertChorusState {
    buffer: Vec<(f32, f32)>,
    write_index: usize,
    phase: f32,
    phase_step: f32,
    base_delay: f32,
    modulation_depth: f32,
    mix: f32,
    tail_frames: u32,
}

impl InsertChorusState {
    #[allow(clippy::cast_precision_loss)]
    fn new(trigger: &SampleTrigger, sample_rate: u32) -> Option<Self> {
        let mix = sanitize_unit_f32(trigger.chorus_mix(), 0.0);
        if mix <= f32::EPSILON {
            return None;
        }

        let depth = sanitize_unit_f32(trigger.chorus_depth(), 0.4);
        let rate = sanitize_non_negative_f32(trigger.chorus_rate(), 0.5);
        let base_delay = depth.mul_add(8.0, 4.0);
        let modulation_depth = depth.mul_add(6.0, 1.5);
        let rate_hz = rate.mul_add(5.75, 0.25);
        let phase_step = (TAU * rate_hz) / (sample_rate as f32);
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let tail_frames = (base_delay + modulation_depth).ceil() as u32;

        Some(Self {
            buffer: vec![(0.0, 0.0); INSERT_CHORUS_BUFFER_FRAMES],
            write_index: 0,
            phase: 0.0,
            phase_step,
            base_delay,
            modulation_depth,
            mix,
            tail_frames,
        })
    }

    const fn tail_frames(&self) -> u32 {
        self.tail_frames
    }

    fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        self.buffer[self.write_index] = (input_left, input_right);

        let left_delay = self
            .modulation_depth
            .mul_add(self.phase.sin().mul_add(0.5, 0.5), self.base_delay);
        let right_delay = self.modulation_depth.mul_add(
            TAU.mul_add(0.25, self.phase).sin().mul_add(0.5, 0.5),
            self.base_delay,
        );
        let wet_left = self.read_interpolated(left_delay, true);
        let wet_right = self.read_interpolated(right_delay, false);

        self.write_index += 1;
        if self.write_index == self.buffer.len() {
            self.write_index = 0;
        }

        self.phase += self.phase_step;
        if self.phase >= TAU {
            self.phase -= TAU;
        }

        (
            blend_dry_wet(input_left, wet_left, self.mix),
            blend_dry_wet(input_right, wet_right, self.mix),
        )
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn read_interpolated(&self, delay_frames: f32, left_channel: bool) -> f32 {
        let buffer_len = self.buffer.len() as f32;
        let read_position = ((self.write_index as f32) - delay_frames).rem_euclid(buffer_len);
        #[allow(clippy::cast_sign_loss)]
        let base_index = read_position.floor() as usize;
        let next_index = (base_index + 1) % self.buffer.len();
        let fraction = read_position - (base_index as f32);

        let (base_left, base_right) = self.buffer[base_index];
        let (next_left, next_right) = self.buffer[next_index];
        let base = if left_channel { base_left } else { base_right };
        let next = if left_channel { next_left } else { next_right };
        base.mul_add(1.0 - fraction, next * fraction)
    }
}

#[derive(Debug)]
struct InsertCompressorState {
    mix: f32,
    threshold: f32,
    ratio: f32,
    envelope: f32,
}

impl InsertCompressorState {
    fn new(trigger: &SampleTrigger) -> Option<Self> {
        let mix = sanitize_unit_f32(trigger.compressor_mix(), 0.0);
        if mix <= f32::EPSILON {
            return None;
        }

        Some(Self {
            mix,
            threshold: sanitize_unit_f32(trigger.compressor_threshold(), 0.5).max(1.0e-4),
            ratio: sanitize_ratio_f32(trigger.compressor_ratio(), 4.0),
            envelope: 0.0,
        })
    }

    fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        let peak = input_left.abs().max(input_right.abs());
        let smoothing = if peak > self.envelope { 0.35 } else { 0.08 };
        self.envelope = (peak - self.envelope).mul_add(smoothing, self.envelope);

        let gain = if self.envelope > self.threshold {
            let compressed = self
                .threshold
                .mul_add(1.0, (self.envelope - self.threshold) / self.ratio);
            compressed / self.envelope.max(f32::EPSILON)
        } else {
            1.0
        };
        let wet_left = input_left * gain;
        let wet_right = input_right * gain;

        (
            blend_dry_wet(input_left, wet_left, self.mix),
            blend_dry_wet(input_right, wet_right, self.mix),
        )
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

fn blend_dry_wet(dry: f32, wet: f32, mix: f32) -> f32 {
    dry.mul_add(1.0 - mix, wet * mix)
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
fn analog_voice_params(
    kind: VoiceKind,
    trigger: &SampleTrigger,
    base_hz: f32,
) -> AnalogVoiceParams {
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
        freq_hz: analog_frequency_hz(trigger, base_hz),
        pulse_width: sanitize_pulse_width(trigger.pulse_width()),
        cutoff_hz: trigger.lpf_cutoff_hz().unwrap_or(1_200.0) as f32,
        resonance: sanitize_unit_f32(trigger.resonance(), 0.2),
        drive: sanitize_non_negative_f32(trigger.drive(), 1.0),
        gain: sanitize_non_negative_f32(trigger.gain(), 1.0) * ANALOG_OUTPUT_TRIM,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn analog_frequency_hz(trigger: &SampleTrigger, base_hz: f32) -> f32 {
    let rate = if trigger.rate().is_finite() {
        trigger.rate().abs() as f32
    } else {
        1.0
    };
    (base_hz * rate).max(0.0)
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
fn sanitize_ratio_f32(value: f64, default: f32) -> f32 {
    if value.is_finite() {
        (value as f32).max(1.0)
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

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn cycle_fraction_to_frames(fraction: f64, frames_per_cycle: u64) -> Option<u32> {
    if !fraction.is_finite() || fraction <= 0.0 {
        return None;
    }

    let frames = (fraction * (frames_per_cycle as f64))
        .round()
        .clamp(1.0, f64::from(u32::MAX));
    #[allow(clippy::cast_sign_loss)]
    Some(frames as u32)
}

#[allow(clippy::cast_possible_truncation)]
fn decay_repeat_count(feedback: f32) -> u32 {
    if feedback <= f32::EPSILON {
        return 1;
    }
    if feedback >= 0.999 {
        return INSERT_DECAY_REPEAT_CAP;
    }

    let repeats = 1.0e-3_f32.log(feedback).ceil();
    if repeats.is_finite() {
        #[allow(clippy::cast_sign_loss)]
        (repeats as u32).clamp(1, INSERT_DECAY_REPEAT_CAP)
    } else {
        INSERT_DECAY_REPEAT_CAP
    }
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
        self.state = self.alpha.mul_add(input - self.state, self.state);
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

fn snap_slice_boundary(boundary: f64) -> f64 {
    let rounded = boundary.round();
    if (boundary - rounded).abs() <= 1.0e-9 {
        rounded
    } else {
        boundary
    }
}
