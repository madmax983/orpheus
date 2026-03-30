//! Faust-style block diagram algebra for composing DSP graphs.
//!
//! This module provides a combinator system inspired by Faust's five operators
//! (sequential, parallel, split, merge, recursive) for wiring fundamental DSP
//! blocks into composite audio processors.
//!
//! Parameters flow as signal inputs (Faust model): a filter's cutoff frequency
//! is an input channel, not a special configuration field. Channel counts are
//! validated at graph construction time.

mod adapters;
mod combinators;
mod helpers;
mod node;
mod primitives;
mod processor;

// Trait + error
pub use node::{GraphError, Node};

// Processor
pub use processor::Processor;

// Primitives (new nodes)
pub use primitives::{
    ConstNode, DelayNode, OnePoleNode, PassthroughNode, SineNode, SumNode, WireNode, constant,
    delay_line, one_pole, passthrough, sine, sum, wire,
};

// Adapters (existing synth/ primitive wrappers)
pub use adapters::{
    GainNode, LadderFilterNode, NoiseNode, PulseNode, SawNode, SoftSatNode, TriNode, gain_node,
    ladder_filter, noise, pulse, saw, soft_sat, tri,
};

// Combinators
pub use combinators::{Mrg, Par, Rec, Seq, Spl, feedback, merge, par, seq, split};

// Helpers (ergonomic composition utilities)
pub use helpers::{Bind, bind, pipe};
