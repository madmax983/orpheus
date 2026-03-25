//! Gain helpers for scalar DSP primitives.

/// A stateless gain stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gain;

impl Gain {
    /// Creates a new gain helper.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Resets the gain stage.
    pub const fn reset(&mut self) {}

    /// Applies linear gain to a single sample.
    #[must_use]
    pub fn process(&mut self, input: f32, amount: f32) -> f32 {
        input * amount
    }
}
