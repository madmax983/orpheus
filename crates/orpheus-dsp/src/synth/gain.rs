#![allow(clippy::float_cmp)]
#![allow(
    clippy::suboptimal_flops,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_map,
    clippy::redundant_closure_for_method_calls,
    clippy::default_constructed_unit_structs
)]
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
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::Gain;
    ///
    /// let mut gain = Gain::new();
    ///
    /// // Attenuate by half
    /// assert_eq!(gain.process(1.0, 0.5), 0.5);
    ///
    /// // Phase inversion
    /// assert_eq!(gain.process(1.0, -1.0), -1.0);
    /// ```
    #[must_use]
    pub fn process(&mut self, input: f32, amount: f32) -> f32 {
        input * amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_new_and_default_initialize_state() {
        let g1 = Gain::new();
        let g2 = Gain::default();
        assert_eq!(g1, g2);
    }

    #[test]
    fn gain_reset_is_a_no_op() {
        let mut gain = Gain::new();
        gain.reset(); // Should not panic or change state meaningfully
    }

    #[test]
    fn gain_process_applies_linear_multiplier() {
        let mut gain = Gain::new();

        assert_eq!(gain.process(1.0, 0.0), 0.0, "Zero gain mutes input");
        assert_eq!(gain.process(1.0, 1.0), 1.0, "Unity gain passes input");
        assert_eq!(gain.process(1.0, 0.5), 0.5, "Half gain halves input");
        assert_eq!(gain.process(1.0, 2.0), 2.0, "Gain > 1.0 boosts input");
        assert_eq!(gain.process(1.0, -1.0), -1.0, "Negative gain inverts phase");

        // Ensure state isn't changing behavior between calls
        assert_eq!(gain.process(0.5, 0.5), 0.25);
    }
}
