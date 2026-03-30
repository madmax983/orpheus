//! Filter primitives for the analog DSP kernel.

use std::f32::consts::TAU;

/// A stable four-stage ladder-style low-pass filter approximation.
#[derive(Debug, Clone, PartialEq)]
pub struct LadderFilter {
    sample_rate_hz: f32,
    stage1: f32,
    stage2: f32,
    stage3: f32,
    stage4: f32,
}

impl LadderFilter {
    /// Creates a new ladder filter for `sample_rate_hz`.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz: if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
                sample_rate_hz
            } else {
                48_000.0
            },
            stage1: 0.0,
            stage2: 0.0,
            stage3: 0.0,
            stage4: 0.0,
        }
    }

    /// Clears all filter state.
    pub const fn reset(&mut self) {
        self.stage1 = 0.0;
        self.stage2 = 0.0;
        self.stage3 = 0.0;
        self.stage4 = 0.0;
    }

    fn cutoff_coefficient(&self, cutoff_hz: f32) -> f32 {
        let nyquist = self.sample_rate_hz * 0.5;
        let clamped = if cutoff_hz.is_finite() {
            cutoff_hz.clamp(0.0, nyquist * 0.99)
        } else {
            0.0
        };

        1.0 - (-TAU * clamped / self.sample_rate_hz).exp()
    }

    fn zero_cutoff_decay(&self) -> f32 {
        (1.0 - (32.0 / self.sample_rate_hz)).clamp(0.0, 1.0)
    }

    /// Processes one sample through the ladder approximation.
    #[must_use]
    pub fn process(&mut self, input: f32, cutoff_hz: f32, resonance: f32) -> f32 {
        let drive = if input.is_finite() { input } else { 0.0 };
        let finite_cutoff = if cutoff_hz.is_finite() {
            cutoff_hz
        } else {
            0.0
        };
        let resonance_amount = if resonance.is_finite() {
            resonance.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if finite_cutoff <= 0.0 {
            let decay = self.zero_cutoff_decay();
            self.stage1 *= decay;
            self.stage2 *= decay;
            self.stage3 *= decay;
            self.stage4 *= decay;
            return self.stage4;
        }

        let g = self.cutoff_coefficient(finite_cutoff);

        let feedback = resonance_amount * 4.0 * self.stage4;
        let mut stage_input = (drive - feedback).tanh();

        self.stage1 += g * (stage_input - self.stage1);
        stage_input = self.stage1.tanh();

        self.stage2 += g * (stage_input - self.stage2);
        stage_input = self.stage2.tanh();

        self.stage3 += g * (stage_input - self.stage3);
        stage_input = self.stage3.tanh();

        self.stage4 += g * (stage_input - self.stage4);
        self.stage4
    }
}
