use core::f32::consts::TAU;

/// Built-in synthesized drum voices available before WAV support lands.
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
    /// Resolves a phase-one sample token to a built-in synthesized voice.
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
    kind: VoiceKind,
    frame_index: u32,
    duration_frames: u32,
    sample_rate_hz: f64,
    noise_state: u32,
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
            kind,
            frame_index: 0,
            duration_frames: duration_frames.max(1),
            sample_rate_hz: f64::from(sample_rate),
            noise_state: 0x00C0_FFEE_u32,
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn next_sample(&mut self) -> Option<f32> {
        if self.frame_index >= self.duration_frames {
            return None;
        }

        let progress = f64::from(self.frame_index) / f64::from(self.duration_frames);
        let time = f64::from(self.frame_index) / self.sample_rate_hz;
        let envelope = 1.0 - progress;
        let sample = match self.kind {
            VoiceKind::KickLike => {
                let frequency = (-120.0_f64).mul_add(progress, 160.0);
                (time * frequency * f64::from(TAU)).sin() * envelope.powi(3) * 0.85
            }
            VoiceKind::SnareLike => self.next_noise() * envelope.powi(2) * 0.65,
            VoiceKind::ClapLike => {
                let burst = if progress < 0.12 || (0.2..0.32).contains(&progress) {
                    1.0
                } else {
                    0.5
                };
                self.next_noise() * envelope.powi(2) * burst * 0.55
            }
            VoiceKind::HiHatLike => self.next_noise().signum() * envelope.powi(2) * 0.35,
        };

        self.frame_index = self.frame_index.saturating_add(1);
        Some(sample as f32)
    }

    fn next_noise(&mut self) -> f64 {
        self.noise_state = self
            .noise_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let normalized = f64::from((self.noise_state >> 8) & 0x00FF_FFFF) / 16_777_215.0;
        (normalized * 2.0) - 1.0
    }
}
