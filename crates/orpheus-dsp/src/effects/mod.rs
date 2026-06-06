//! DSP implementations for stereo bus effects.
//!
//! This module contains the stateful implementations of global bus effects
//! (e.g., [`DelayState`] and [`ReverbState`]) that are hosted on the master bus
//! or auxiliary send buses.
//!
//! # Architecture
//! Effects receive a stereo input frame and produce a stereo output frame.
//! They are designed to operate lock-free on the audio thread, relying on
//! synchronization methods (like [`BusEffectState::sync_timing`]) to receive
//! parameter updates from the language runtime without allocating.

use crate::engine::EngineError;
use crate::routing::BusEffectSpec;

mod delay;
mod reverb;

pub use delay::DelayState;
pub use reverb::ReverbState;

/// A stateful instance of a global stereo bus effect.
///
/// This enum wraps the specific implementation details of effects like
/// delay or reverb. It provides a unified interface for the DSP routing
/// graph to instantiate and process audio through any supported effect type.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{BusEffectSpec, DelaySpec};
/// use orpheus_pattern::Rational;
/// // BusEffectState is not publicly exported at the crate root, but is
/// // instantiated internally by the routing DSP graph using from_spec:
///
/// // Create a delay specification
/// let spec = BusEffectSpec::Delay(DelaySpec::new(
///     Rational::new(1, 4).unwrap(),
///     0.5,
///     0.2
/// ));
/// ```
#[derive(Debug)]
pub enum BusEffectState {
    Delay(DelayState),
    Reverb(ReverbState),
}

impl BusEffectState {
    /// Constructs a new stateful effect instance configured by the given specification.
    ///
    /// This instantiates the underlying effect implementation (e.g., [`DelayState`] or
    /// [`ReverbState`]) based on the enum variant of the provided [`BusEffectSpec`].
    pub fn from_spec(spec: &BusEffectSpec, frames_per_cycle: u64) -> Result<Self, EngineError> {
        match spec {
            BusEffectSpec::Delay(delay) => {
                Ok(Self::Delay(DelayState::new(delay, frames_per_cycle)?))
            }
            BusEffectSpec::Reverb(reverb) => Ok(Self::Reverb(ReverbState::new(reverb))),
        }
    }

    /// Dynamically updates the parameters of the underlying effect implementation.
    ///
    /// The runtime ensures that the `spec` matches the internal state variant.
    /// It delegates the update to the specific effect's synchronization logic.
    pub fn sync_timing(
        &mut self,
        spec: &BusEffectSpec,
        frames_per_cycle: u64,
    ) -> Result<(), EngineError> {
        match (self, spec) {
            (Self::Delay(state), BusEffectSpec::Delay(delay)) => {
                state.sync_timing(delay, frames_per_cycle)
            }
            (Self::Reverb(state), BusEffectSpec::Reverb(reverb)) => {
                let _ = frames_per_cycle;
                state.sync_spec(reverb);
                Ok(())
            }
            _ => unreachable!("bus effect state kind must match the hosted spec"),
        }
    }

    /// Processes a single stereo frame through the underlying effect implementation.
    #[must_use]
    pub fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        match self {
            Self::Delay(state) => state.process_frame(input_left, input_right),
            Self::Reverb(state) => state.process_frame(input_left, input_right),
        }
    }

    /// Clears the internal delay or history buffers of the underlying effect,
    /// instantly stopping any trailing audio tail.
    pub fn reset(&mut self) {
        match self {
            Self::Delay(state) => state.reset(),
            Self::Reverb(state) => state.reset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "bus effect state kind must match the hosted spec")]
    fn test_bus_effect_sync_timing_panics_on_mismatched_spec() {
        let reverb_spec = crate::ReverbSpec::new(0.5, 0.5, 0.5);
        let mut state = BusEffectState::Reverb(crate::effects::reverb::ReverbState::new(&reverb_spec));
        let delay_spec = crate::DelaySpec::new(orpheus_pattern::Rational::one(), 0.5, 0.5);
        let spec = BusEffectSpec::Delay(delay_spec);

        let _ = state.sync_timing(&spec, 88200);
    }
}
