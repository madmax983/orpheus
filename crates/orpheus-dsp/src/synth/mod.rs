//! Reusable scalar DSP primitives for analog-style voice construction.
//!
//! This module is the small verified-ish kernel for later voice and graph work.
//! The first slice intentionally stays narrow: phase helpers, gain, mixing, and
//! bounded nonlinearity.

mod filter;
mod gain;
mod math;
mod mix;
mod nonlinear;
mod osc;

pub use filter::LadderFilter;
pub use gain::Gain;
pub use math::PhaseAccumulator;
pub use mix::Mix;
pub use nonlinear::SoftSat;
pub use osc::{Noise, PulseOsc, SawOsc, TriOsc};
