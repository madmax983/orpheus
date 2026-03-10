use core::f32::consts::TAU;

use crate::sample_bank::PlaybackSample;

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
    },
}

impl ActiveVoice {
    pub fn new(kind: VoiceKind, sample_rate: u32) -> Self {
        let duration_frames = match kind {
            VoiceKind::KickLike => sample_rate / 3,
            VoiceKind::SnareLike => sample_rate / 5,
            VoiceKind::ClapLike => sample_rate / 6,
            VoiceKind::HiHatLike => sample_rate / 8,
        };

        Self {
            state: ActiveVoiceState::Synth {
                kind,
                frame_index: 0,
                duration_frames: duration_frames.max(1),
                sample_rate_hz: f64::from(sample_rate),
                noise_state: 0x00C0_FFEE_u32,
            },
        }
    }

    pub fn from_sample(sample: &PlaybackSample, output_sample_rate: u32) -> Self {
        Self {
            state: ActiveVoiceState::Sample {
                frames: sample.frames().clone(),
                frame_position: 0.0,
                frame_step: f64::from(sample.sample_rate_hz()) / f64::from(output_sample_rate),
            },
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    pub fn next_sample(&mut self) -> Option<f32> {
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
            } => {
                let index = frame_position.floor() as usize;
                let sample = *frames.get(index)?;
                *frame_position += *frame_step;
                Some(sample)
            }
        }
    }
}

fn next_noise(noise_state: &mut u32) -> f64 {
    *noise_state = noise_state
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223);
    let normalized = f64::from((*noise_state >> 8) & 0x00FF_FFFF) / 16_777_215.0;
    (normalized * 2.0) - 1.0
}
