//! Math helpers for scalar DSP primitives.

/// A normalized unit-interval phase accumulator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseAccumulator {
    phase: f32,
}

impl Default for PhaseAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseAccumulator {
    /// Creates a new phase accumulator starting at `0.0`.
    #[must_use]
    pub const fn new() -> Self {
        Self { phase: 0.0 }
    }

    /// Returns the current wrapped phase in `[0.0, 1.0)`.
    #[must_use]
    pub const fn phase(&self) -> f32 {
        self.phase
    }

    /// Resets the phase accumulator to the start of the cycle.
    pub const fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Advances the phase by `step` turns and returns the wrapped phase.
    pub fn advance(&mut self, step: f32) -> f32 {
        if !step.is_finite() {
            return self.phase;
        }

        self.phase = (self.phase + step).rem_euclid(1.0);
        self.phase
    }
}
