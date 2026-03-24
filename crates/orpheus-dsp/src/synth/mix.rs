//! Mixing helpers for scalar DSP primitives.

/// A stateless linear crossfader.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mix;

impl Mix {
    /// Creates a new linear mixer helper.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Resets the mixer.
    pub const fn reset(&mut self) {}

    /// Blends between `left` and `right` with a clamped `balance` in `[0, 1]`.
    #[must_use]
    pub fn blend(left: f32, right: f32, balance: f32) -> f32 {
        let clamped = balance.clamp(0.0, 1.0);
        clamped.mul_add(right - left, left)
    }

    /// Processes a single blend step.
    #[must_use]
    pub fn process(&mut self, left: f32, right: f32, balance: f32) -> f32 {
        Self::blend(left, right, balance)
    }
}
