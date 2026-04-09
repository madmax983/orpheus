#![allow(clippy::float_cmp)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_accumulator_new_and_default_initialize_to_zero() {
        let p1 = PhaseAccumulator::new();
        let p2 = PhaseAccumulator::default();
        assert_eq!(p1, p2);
        assert_eq!(p1.phase(), 0.0);
    }

    #[test]
    fn phase_accumulator_reset_resets_to_zero() {
        let mut p = PhaseAccumulator::new();
        p.advance(0.5);
        assert_eq!(p.phase(), 0.5);
        p.reset();
        assert_eq!(p.phase(), 0.0);
    }

    #[test]
    fn phase_accumulator_advance_wraps_around_one() {
        let mut p = PhaseAccumulator::new();

        assert_eq!(p.advance(0.25), 0.25);
        assert_eq!(p.advance(0.5), 0.75);

        // Wrap around
        let wrapped = p.advance(0.5);
        // Using approximate equality because of floats
        assert!(
            (wrapped - 0.25).abs() < f32::EPSILON,
            "Expected 0.25, got {wrapped}"
        );
        assert_eq!(p.phase(), wrapped);
    }

    #[test]
    fn phase_accumulator_advance_handles_negative_steps() {
        let mut p = PhaseAccumulator::new();

        let val = p.advance(-0.25);
        assert!(
            (val - 0.75).abs() < f32::EPSILON,
            "Expected 0.75, got {val}"
        );

        let val2 = p.advance(-1.5);
        assert!(
            (val2 - 0.25).abs() < f32::EPSILON,
            "Expected 0.25, got {val2}"
        );
    }

    #[test]
    fn phase_accumulator_advance_handles_large_steps() {
        let mut p = PhaseAccumulator::new();

        let val = p.advance(10.25);
        assert!(
            (val - 0.25).abs() < f32::EPSILON,
            "Expected 0.25, got {val}"
        );
    }

    #[test]
    fn phase_accumulator_advance_ignores_non_finite_steps() {
        let mut p = PhaseAccumulator::new();
        p.advance(0.5);

        assert_eq!(p.advance(f32::NAN), 0.5);
        assert_eq!(p.advance(f32::INFINITY), 0.5);
        assert_eq!(p.advance(f32::NEG_INFINITY), 0.5);
        assert_eq!(p.phase(), 0.5);
    }
}
