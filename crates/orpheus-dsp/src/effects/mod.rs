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

/// Represents the state of a stereo bus effect instance.
///
/// `BusEffectState` holds the DSP state for effects like delay or reverb.
/// It provides methods to instantiate from a [`BusEffectSpec`], sync timing
/// and parameters without allocation, process stereo frames, and reset state.
///
/// # Examples
/// ```
/// use orpheus_dsp::effects::BusEffectState;
/// use orpheus_dsp::routing::{BusEffectSpec, DelaySpec};
/// use orpheus_pattern::Rational;
///
/// let spec = BusEffectSpec::Delay(DelaySpec::new(Rational::new(1, 4).unwrap(), 0.5, 0.2));
/// let mut state = BusEffectState::from_spec(&spec, 44100).unwrap();
/// let output = state.process_frame(1.0, 1.0);
/// state.reset();
/// ```
#[derive(Debug)]
pub enum BusEffectState {
    Delay(DelayState),
    Reverb(ReverbState),
}

impl BusEffectState {
    pub fn from_spec(spec: &BusEffectSpec, frames_per_cycle: u64) -> Result<Self, EngineError> {
        match spec {
            BusEffectSpec::Delay(delay) => {
                Ok(Self::Delay(DelayState::new(delay, frames_per_cycle)?))
            }
            BusEffectSpec::Reverb(reverb) => Ok(Self::Reverb(ReverbState::new(reverb))),
        }
    }

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

    #[must_use]
    pub fn process_frame(&mut self, input_left: f32, input_right: f32) -> (f32, f32) {
        match self {
            Self::Delay(state) => state.process_frame(input_left, input_right),
            Self::Reverb(state) => state.process_frame(input_left, input_right),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Delay(state) => state.reset(),
            Self::Reverb(state) => state.reset(),
        }
    }
}
