//! Adapters wrapping existing `synth/` primitives into graph-compatible [`Node`]s.
//!
//! Each adapter loops `0..frames` internally, calling the existing per-sample
//! method. This is the impedance match between the scalar DSP kernel and the
//! block-based graph system.

use super::node::Node;
use crate::synth::{Gain, LadderFilter, Noise, PulseOsc, SawOsc, SoftSat, TriOsc};

// ---------------------------------------------------------------------------
// SawNode
// ---------------------------------------------------------------------------

/// Band-limited saw oscillator. 1 input (freq\_hz), 1 output (audio).
pub struct SawNode {
    osc: SawOsc,
}

impl Node for SawNode {
    fn inputs(&self) -> u32 {
        1
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let freq = inputs[0];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.osc.next_sample(freq[i]);
        }
    }
    fn reset(&mut self) {
        self.osc.reset();
    }
}

/// Creates a band-limited saw oscillator node. 1 input (freq\_hz), 1 output.
#[must_use]
pub fn saw(sample_rate_hz: f32) -> SawNode {
    SawNode {
        osc: SawOsc::new(sample_rate_hz),
    }
}

// ---------------------------------------------------------------------------
// PulseNode
// ---------------------------------------------------------------------------

/// Band-limited pulse oscillator. 2 inputs (freq\_hz, pulse\_width), 1 output.
pub struct PulseNode {
    osc: PulseOsc,
}

impl Node for PulseNode {
    fn inputs(&self) -> u32 {
        2
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let freq = inputs[0];
        let pw = inputs[1];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.osc.next_sample(freq[i], pw[i]);
        }
    }
    fn reset(&mut self) {
        self.osc.reset();
    }
}

/// Creates a band-limited pulse oscillator node. 2 inputs (freq\_hz, pw), 1 output.
#[must_use]
pub fn pulse(sample_rate_hz: f32) -> PulseNode {
    PulseNode {
        osc: PulseOsc::new(sample_rate_hz),
    }
}

// ---------------------------------------------------------------------------
// TriNode
// ---------------------------------------------------------------------------

/// Triangle oscillator. 1 input (freq\_hz), 1 output (audio).
pub struct TriNode {
    osc: TriOsc,
}

impl Node for TriNode {
    fn inputs(&self) -> u32 {
        1
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let freq = inputs[0];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.osc.next_sample(freq[i]);
        }
    }
    fn reset(&mut self) {
        self.osc.reset();
    }
}

/// Creates a triangle oscillator node. 1 input (freq\_hz), 1 output.
#[must_use]
pub fn tri(sample_rate_hz: f32) -> TriNode {
    TriNode {
        osc: TriOsc::new(sample_rate_hz),
    }
}

// ---------------------------------------------------------------------------
// NoiseNode
// ---------------------------------------------------------------------------

/// Deterministic white noise. 0 inputs, 1 output (audio).
pub struct NoiseNode {
    rng: Noise,
    seed: u32,
}

impl Node for NoiseNode {
    fn inputs(&self) -> u32 {
        0
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, _inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let out = &mut outputs[0];
        for sample in &mut out[..frames] {
            *sample = self.rng.next_sample();
        }
    }
    fn reset(&mut self) {
        self.rng.reset(self.seed);
    }
}

/// Creates a deterministic white noise node. 0 inputs, 1 output.
#[must_use]
pub const fn noise(seed: u32) -> NoiseNode {
    NoiseNode {
        rng: Noise::new(seed),
        seed,
    }
}

// ---------------------------------------------------------------------------
// LadderFilterNode
// ---------------------------------------------------------------------------

/// 4-stage ladder low-pass filter. 3 inputs (audio, cutoff\_hz, resonance), 1 output.
pub struct LadderFilterNode {
    filter: LadderFilter,
}

impl Node for LadderFilterNode {
    fn inputs(&self) -> u32 {
        3
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let audio = inputs[0];
        let cutoff = inputs[1];
        let resonance = inputs[2];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.filter.process(audio[i], cutoff[i], resonance[i]);
        }
    }
    fn reset(&mut self) {
        self.filter.reset();
    }
}

/// Creates a ladder filter node. 3 inputs (audio, cutoff\_hz, resonance), 1 output.
#[must_use]
pub fn ladder_filter(sample_rate_hz: f32) -> LadderFilterNode {
    LadderFilterNode {
        filter: LadderFilter::new(sample_rate_hz),
    }
}

// ---------------------------------------------------------------------------
// GainNode
// ---------------------------------------------------------------------------

/// Linear gain stage. 2 inputs (audio, amount), 1 output.
pub struct GainNode {
    gain: Gain,
}

impl Node for GainNode {
    fn inputs(&self) -> u32 {
        2
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let audio = inputs[0];
        let amount = inputs[1];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.gain.process(audio[i], amount[i]);
        }
    }
    fn reset(&mut self) {
        self.gain.reset();
    }
}

/// Creates a gain node. 2 inputs (audio, amount), 1 output.
#[must_use]
pub const fn gain_node() -> GainNode {
    GainNode { gain: Gain::new() }
}

// ---------------------------------------------------------------------------
// SoftSatNode
// ---------------------------------------------------------------------------

/// Soft saturation stage. 2 inputs (audio, drive), 1 output.
pub struct SoftSatNode {
    sat: SoftSat,
}

impl Node for SoftSatNode {
    fn inputs(&self) -> u32 {
        2
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let audio = inputs[0];
        let drive = inputs[1];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.sat.process(audio[i], drive[i]);
        }
    }
    fn reset(&mut self) {
        self.sat.reset();
    }
}

/// Creates a soft saturation node. 2 inputs (audio, drive), 1 output.
#[must_use]
pub const fn soft_sat() -> SoftSatNode {
    SoftSatNode {
        sat: SoftSat::new(),
    }
}
