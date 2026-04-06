//! Lock-free audio effect implementations for the routing bus.
//!
//! This module houses the algorithms for global bus effects, such as Delay and Reverb.
//! These effects run lock-free on the audio thread, synchronized exactly to the
//! rational time cycles emitted by the pattern engine. They are driven by
//! specifications originating from the mixer state in `orpheus-lang`.

use crate::engine::EngineError;
use crate::routing::BusEffectSpec;

mod delay;
mod reverb;

pub use delay::DelayState;
pub use reverb::ReverbState;

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
