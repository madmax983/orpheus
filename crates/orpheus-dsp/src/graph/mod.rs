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
mod chiptune;
mod combinators;
mod filters;
mod genesis_fm;
mod genesis_psg;
mod helpers;
mod node;
mod primitives;
mod processor;
mod sample_player;

// Trait + error
pub use node::{GraphError, Node};

// Processor
pub use processor::Processor;

// Primitives (new nodes)
pub use primitives::{
    AdsrNode, ArNode, ConstNode, DelayNode, FractionalDelayNode, MAX_FRACTIONAL_DELAY_SECONDS,
    OnePoleNode, PanNode, PassthroughNode, SineNode, SumNode, WireNode, adsr, ar, constant,
    delay_line, fdelay, one_pole, pan, passthrough, sine, sum, wire, wire_with_inputs,
};

// Filters (multi-mode: TPT SVF + RBJ biquad)
pub use filters::{
    BiquadMode, BiquadNode, FILTER_MAX_GAIN_DB, FILTER_MAX_Q, FILTER_MIN_FREQUENCY_HZ,
    FILTER_MIN_Q, SvfNode, biquad, svf,
};

// Chiptune (NES-authentic tone generators, ported from madmax983/nes apu.rs)
pub use chiptune::{
    NoiseNesNode, PulseNesNode, TriNesNode, advance_lfsr, noise_nes, pulse_nes, tri_nes,
};

// Genesis FM (YM2612 4-operator FM voice, ported from madmax983/genesoxide ym2612.rs)
pub use genesis_fm::{
    FmGenesisNode, FmOp, FmPatch, FmTables, bell, brass, drum, ebass, epiano, fm_genesis, lead,
    preset_by_name,
};

// Genesis PSG (SN76489 tone + noise, ported from madmax983/genesoxide psg.rs)
pub use genesis_psg::{PsgNoiseNode, PsgToneNode, advance_psg_lfsr, psg_noise, psg_tone};

// Adapters (existing synth/ primitive wrappers)
pub use adapters::{
    GainNode, LadderFilterNode, MixNode, NoiseNode, PulseNode, SawNode, SoftSatNode, TriNode,
    gain_node, ladder_filter, mix_node, noise, pulse, saw, soft_sat, tri,
};

// Sample playback (one-shot, looped, crossfaded-loop, and pitched players
// over preloaded bank buffers)
pub use sample_player::{
    LOOP_CROSSFADE_MAX_BUFFER_FRACTION, LOOP_CROSSFADE_SECONDS, SamplePlayerNode, sample_player,
    sample_player_looped, sample_player_looped_crossfaded, sample_player_pitched,
    sample_player_with_options,
};

// Combinators
pub use combinators::{Mrg, Par, Rec, Seq, Spl, feedback, merge, par, seq, split};

// Helpers (ergonomic composition utilities)
pub use helpers::{Bind, bind, pipe};
