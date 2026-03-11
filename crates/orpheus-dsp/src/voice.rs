use core::f32::consts::TAU;

use crate::SampleTrigger;
use crate::sample_bank::PlaybackSample;

const MAX_SAMPLE_EDGE_RAMP_FRAMES: u32 = 32;

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
}

impl VoiceKind {
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::KickLike => "bd",
            Self::SnareLike => "sn",
            Self::ClapLike => "cp",
            Self::HiHatLike => "hh",
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
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActiveVoice {
    state: ActiveVoiceState,
    left_gain: f64,
    right_gain: f64,
}

#[derive(Clone, Debug)]
enum ActiveVoiceState {
    Synth {
        kind: VoiceKind,
        frame_index: u32,
        duration_frames: u32,
        sample_rate_hz: f64,
        noise_state: u32,
    },
    Sample {
        frames: std::sync::Arc<[f32]>,
        frame_position: f64,
        frame_step: f64,
        frame_limit: f64,
        gain: f64,
        rendered_frames: u32,
        total_output_frames: u32,
        edge_ramp_frames: u32,
    },
}

impl ActiveVoice {
    pub fn new_with_pan(kind: VoiceKind, sample_rate: u32, pan: f64) -> Self {
        let duration_frames = match kind {
            VoiceKind::KickLike => sample_rate / 3,
            VoiceKind::SnareLike => sample_rate / 5,
            VoiceKind::ClapLike => sample_rate / 6,
            VoiceKind::HiHatLike => sample_rate / 8,
        };
        let (left_gain, right_gain) = stereo_gains_for_pan(pan);

        Self {
            state: ActiveVoiceState::Synth {
                kind,
                frame_index: 0,
                duration_frames: duration_frames.max(1),
                sample_rate_hz: f64::from(sample_rate),
                noise_state: 0x00C0_FFEE_u32,
            },
            left_gain,
            right_gain,
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn from_sample(
        sample: &PlaybackSample,
        output_sample_rate: u32,
        trigger: &SampleTrigger,
    ) -> Self {
        let frame_count_u32 = u32::try_from(sample.frames().len())
            .unwrap_or_else(|_| panic!("sample frame count exceeded supported playback range"));
        let frame_count = f64::from(frame_count_u32);
        let frame_position = trigger.slice_start() * frame_count;
        let frame_limit = trigger.slice_end() * frame_count;
        let frame_step =
            (f64::from(sample.sample_rate_hz()) / f64::from(output_sample_rate)) * trigger.rate();
        let output_frame_count = (((frame_limit - frame_position) / frame_step).ceil())
            .clamp(1.0, f64::from(u32::MAX)) as u32;
        let edge_ramp_frames = output_frame_count
            .div_ceil(2)
            .clamp(1, MAX_SAMPLE_EDGE_RAMP_FRAMES);
        let (left_gain, right_gain) = stereo_gains_for_pan(trigger.pan());
        Self {
            state: ActiveVoiceState::Sample {
                frames: sample.frames().clone(),
                frame_position,
                frame_step,
                frame_limit,
                gain: trigger.gain(),
                rendered_frames: 0,
                total_output_frames: output_frame_count,
                edge_ramp_frames,
            },
            left_gain,
            right_gain,
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn next_mono_sample(&mut self) -> Option<f32> {
        match &mut self.state {
            ActiveVoiceState::Synth {
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
                };

                *frame_index = frame_index.saturating_add(1);
                Some(sample as f32)
            }
            ActiveVoiceState::Sample {
                frames,
                frame_position,
                frame_step,
                frame_limit,
                gain,
                rendered_frames,
                total_output_frames,
                edge_ramp_frames,
            } => {
                if *frame_position >= *frame_limit {
                    return None;
                }
                let index = frame_position.floor() as usize;
                let envelope =
                    sample_edge_envelope(*rendered_frames, *total_output_frames, *edge_ramp_frames);
                let sample = (f64::from(*frames.get(index)?) * *gain * envelope) as f32;
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
