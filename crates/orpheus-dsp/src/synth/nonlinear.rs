//! Nonlinear waveshaping helpers for scalar DSP primitives.

/// A bounded soft saturation stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SoftSat;

impl SoftSat {
    /// Creates a new soft saturation helper.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Resets the soft saturator.
    pub const fn reset(&mut self) {}

    /// Applies a bounded soft saturation curve.
    #[must_use]
    pub fn process(&mut self, input: f32, drive: f32) -> f32 {
        let finite_input = if input.is_finite() { input } else { 0.0 };
        let finite_drive = if drive.is_finite() {
            drive.max(0.0)
        } else {
            0.0
        };
        let shaped = finite_input * (1.0 + finite_drive);
        shaped.tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softsat_new_and_default_initialize_state() {
        let s1 = SoftSat::new();
        let s2 = SoftSat::default();
        assert_eq!(s1, s2);
    }

    #[test]
    fn softsat_reset_is_a_no_op() {
        let mut sat = SoftSat::new();
        sat.reset(); // Should not panic
    }

    #[test]
    fn process_passes_zero_input_unchanged() {
        let mut sat = SoftSat::new();
        assert_eq!(sat.process(0.0, 0.0), 0.0);
        assert_eq!(sat.process(0.0, 1.0), 0.0);
        assert_eq!(sat.process(0.0, 10.0), 0.0);
    }

    #[test]
    fn process_applies_soft_clipping_with_zero_drive() {
        let mut sat = SoftSat::new();
        // tanh(0) = 0
        // tanh(1) = 0.76159...
        // tanh(10) ~= 1.0
        assert_eq!(sat.process(0.0, 0.0), 0.0_f32.tanh());
        assert_eq!(sat.process(1.0, 0.0), 1.0_f32.tanh());
        assert_eq!(sat.process(-1.0, 0.0), (-1.0_f32).tanh());

        let saturated = sat.process(10.0, 0.0);
        assert!(
            saturated > 0.99 && saturated <= 1.0,
            "Large inputs clip softly to 1.0"
        );

        let saturated_neg = sat.process(-10.0, 0.0);
        assert!(
            saturated_neg < -0.99 && saturated_neg >= -1.0,
            "Large negative inputs clip softly to -1.0"
        );
    }

    #[test]
    fn process_amplifies_signal_with_positive_drive_before_clipping() {
        let mut sat = SoftSat::new();
        // input * (1 + drive)
        // input=1.0, drive=1.0 -> tanh(2.0)
        assert_eq!(sat.process(1.0, 1.0), 2.0_f32.tanh());
        // input=0.5, drive=3.0 -> tanh(0.5 * 4.0) = tanh(2.0)
        assert_eq!(sat.process(0.5, 3.0), 2.0_f32.tanh());
    }

    #[test]
    fn process_treats_negative_drive_as_zero() {
        let mut sat = SoftSat::new();
        // If drive < 0, it should max to 0.0
        // input=1.0, drive=-1.0 -> drive.max(0)=0.0 -> tanh(1.0 * 1.0)
        assert_eq!(
            sat.process(1.0, -1.0),
            1.0_f32.tanh(),
            "Negative drive is clamped to 0"
        );
        assert_eq!(
            sat.process(1.0, f32::NEG_INFINITY),
            1.0_f32.tanh(),
            "Negative infinity drive is clamped to 0"
        );
    }

    #[test]
    fn process_handles_non_finite_inputs() {
        let mut sat = SoftSat::new();
        // Non-finite input yields 0.0 (which results in tanh(0.0) = 0.0)
        assert_eq!(sat.process(f32::NAN, 1.0), 0.0, "NaN input yields 0.0");
        assert_eq!(
            sat.process(f32::INFINITY, 1.0),
            0.0,
            "Infinity input yields 0.0"
        );
        assert_eq!(
            sat.process(f32::NEG_INFINITY, 1.0),
            0.0,
            "Neg Infinity input yields 0.0"
        );
    }

    #[test]
    fn process_handles_non_finite_drive() {
        let mut sat = SoftSat::new();
        // Non-finite drive yields 0.0
        // input=1.0, non-finite drive -> tanh(1.0 * (1.0 + 0.0)) = tanh(1.0)
        assert_eq!(
            sat.process(1.0, f32::NAN),
            1.0_f32.tanh(),
            "NaN drive yields 0.0 drive"
        );
        assert_eq!(
            sat.process(1.0, f32::INFINITY),
            1.0_f32.tanh(),
            "Infinity drive yields 0.0 drive"
        );
    }
}
