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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_new_and_default_initialize_state() {
        let m1 = Mix::new();
        let m2 = Mix;
        assert_eq!(m1, m2);
    }

    #[test]
    fn mix_reset_is_a_no_op() {
        let mut mix = Mix::new();
        mix.reset(); // Should not panic
    }

    #[test]
    fn blend_crossfades_linearly_between_inputs() {
        assert_eq!(Mix::blend(1.0, 0.0, 0.0), 1.0, "Balance 0 is 100% left");
        assert_eq!(Mix::blend(1.0, 0.0, 1.0), 0.0, "Balance 1 is 100% right");
        assert_eq!(Mix::blend(1.0, 0.0, 0.5), 0.5, "Balance 0.5 is 50/50 mix");
        assert_eq!(
            Mix::blend(-1.0, 1.0, 0.5),
            0.0,
            "Balance 0.5 of inverted inputs is zero"
        );
    }

    #[test]
    fn blend_clamps_out_of_bounds_balance() {
        assert_eq!(
            Mix::blend(1.0, 0.0, -0.5),
            1.0,
            "Balance < 0 is clamped to 0"
        );
        assert_eq!(
            Mix::blend(1.0, 0.0, 1.5),
            0.0,
            "Balance > 1 is clamped to 1"
        );
    }

    #[test]
    fn process_delegates_to_blend() {
        let mut mix = Mix::new();
        assert_eq!(mix.process(1.0, -1.0, 0.25), Mix::blend(1.0, -1.0, 0.25));
    }
}
