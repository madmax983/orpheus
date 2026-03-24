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
