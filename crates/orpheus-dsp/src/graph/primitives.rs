//! Leaf DSP nodes for the graph combinator system.
//!
//! These are the fundamental building blocks that don't wrap existing `synth/`
//! primitives — they're either new (sine, one-pole, delay) or structural
//! utilities (constant, passthrough, wire, sum).

use std::f32::consts::TAU;

use super::node::Node;
use crate::synth::PhaseAccumulator;

// ---------------------------------------------------------------------------
// ConstNode
// ---------------------------------------------------------------------------

/// A node that produces a constant signal. 0 inputs, 1 output.
#[derive(Debug, Clone)]
pub struct ConstNode {
    value: f32,
}

impl Node for ConstNode {
    fn inputs(&self) -> u32 {
        0
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, _inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        outputs[0][..frames].fill(self.value);
    }
    fn reset(&mut self) {}
}

/// Creates a constant-signal node. 0 inputs, 1 output.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, constant};
///
/// let mut c = constant(0.5);
/// assert_eq!(c.inputs(), 0);
/// assert_eq!(c.outputs(), 1);
/// ```
#[must_use]
pub const fn constant(value: f32) -> ConstNode {
    ConstNode { value }
}

// ---------------------------------------------------------------------------
// SineNode
// ---------------------------------------------------------------------------

/// A sine oscillator. 1 input (freq\_hz), 1 output (audio).
#[derive(Debug, Clone)]
pub struct SineNode {
    phase: PhaseAccumulator,
    sample_rate_hz: f32,
}

impl Node for SineNode {
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
            let f = freq[i];
            let step = if f.is_finite() && f > 0.0 {
                f / self.sample_rate_hz
            } else {
                0.0
            };
            out[i] = (self.phase.phase() * TAU).sin();
            self.phase.advance(step);
        }
    }
    fn reset(&mut self) {
        self.phase.reset();
    }
}

/// Creates a sine oscillator node. 1 input (freq\_hz), 1 output.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, sine};
///
/// let mut osc = sine(44100.0);
/// assert_eq!(osc.inputs(), 1); // Frequency input
/// assert_eq!(osc.outputs(), 1);
/// ```
#[must_use]
pub fn sine(sample_rate_hz: f32) -> SineNode {
    SineNode {
        phase: PhaseAccumulator::new(),
        sample_rate_hz: if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
            sample_rate_hz
        } else {
            48_000.0
        },
    }
}

// ---------------------------------------------------------------------------
// DelayNode
// ---------------------------------------------------------------------------

/// A fixed-length delay line. 1 input, 1 output.
#[derive(Debug, Clone)]
pub struct DelayNode {
    buffer: Vec<f32>,
    write_index: usize,
    delay_samples: usize,
}

impl Node for DelayNode {
    fn inputs(&self) -> u32 {
        1
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let inp = inputs[0];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.buffer[self.write_index];
            self.buffer[self.write_index] = inp[i];
            self.write_index += 1;
            if self.write_index >= self.delay_samples {
                self.write_index = 0;
            }
        }
    }
    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_index = 0;
    }
}

/// Creates a fixed-length delay node. 1 input, 1 output.
///
/// `delay_samples` must be >= 1 (clamped).
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, delay_line};
///
/// let mut delay = delay_line(44100); // 1 second at 44.1kHz
/// assert_eq!(delay.inputs(), 1);
/// assert_eq!(delay.outputs(), 1);
/// ```
#[must_use]
pub fn delay_line(delay_samples: usize) -> DelayNode {
    let delay_samples = delay_samples.max(1);
    DelayNode {
        buffer: vec![0.0; delay_samples],
        write_index: 0,
        delay_samples,
    }
}

// ---------------------------------------------------------------------------
// OnePoleNode
// ---------------------------------------------------------------------------

/// A simple one-pole low-pass filter. 2 inputs (audio, cutoff\_hz), 1 output.
#[derive(Debug, Clone)]
pub struct OnePoleNode {
    sample_rate_hz: f32,
    state: f32,
}

impl Node for OnePoleNode {
    fn inputs(&self) -> u32 {
        2
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let audio = inputs[0];
        let cutoff = inputs[1];
        let out = &mut outputs[0];
        for i in 0..frames {
            let c = cutoff[i].clamp(0.0, self.sample_rate_hz * 0.5);
            let g = 1.0 - (-TAU * c / self.sample_rate_hz).exp();
            self.state += g * (audio[i] - self.state);
            out[i] = self.state;
        }
    }
    fn reset(&mut self) {
        self.state = 0.0;
    }
}

/// Creates a one-pole low-pass filter node. 2 inputs (audio, cutoff\_hz), 1 output.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, one_pole};
///
/// let mut filter = one_pole(44100.0);
/// assert_eq!(filter.inputs(), 2); // Input 0: audio, Input 1: cutoff frequency
/// assert_eq!(filter.outputs(), 1);
/// ```
#[must_use]
pub fn one_pole(sample_rate_hz: f32) -> OnePoleNode {
    OnePoleNode {
        sample_rate_hz: if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
            sample_rate_hz
        } else {
            48_000.0
        },
        state: 0.0,
    }
}

// ---------------------------------------------------------------------------
// PassthroughNode
// ---------------------------------------------------------------------------

/// Identity node. N inputs, N outputs — copies input to output.
#[derive(Debug, Clone)]
pub struct PassthroughNode {
    channels: u32,
}

impl Node for PassthroughNode {
    fn inputs(&self) -> u32 {
        self.channels
    }
    fn outputs(&self) -> u32 {
        self.channels
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        for (out_ch, in_ch) in outputs.iter_mut().zip(inputs.iter()) {
            out_ch[..frames].copy_from_slice(&in_ch[..frames]);
        }
    }
    fn reset(&mut self) {}
}

/// Creates a passthrough (identity) node with `channels` inputs and outputs.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, passthrough};
///
/// // A stereo passthrough node (2 inputs, 2 outputs).
/// let mut pass = passthrough(2);
/// assert_eq!(pass.inputs(), 2);
/// assert_eq!(pass.outputs(), 2);
/// ```
#[must_use]
pub const fn passthrough(channels: u32) -> PassthroughNode {
    PassthroughNode { channels }
}

// ---------------------------------------------------------------------------
// SumNode
// ---------------------------------------------------------------------------

/// Sums N input channels into 1 output channel.
#[derive(Debug, Clone)]
pub struct SumNode {
    input_count: u32,
}

impl Node for SumNode {
    fn inputs(&self) -> u32 {
        self.input_count
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let out = &mut outputs[0];
        out[..frames].fill(0.0);
        for in_ch in inputs {
            for i in 0..frames {
                out[i] += in_ch[i];
            }
        }
    }
    fn reset(&mut self) {}
}

/// Creates a summing node. N inputs, 1 output.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, sum};
///
/// // A node that sums 4 inputs down to a single mono output.
/// let mut mixer = sum(4);
/// assert_eq!(mixer.inputs(), 4);
/// assert_eq!(mixer.outputs(), 1);
/// ```
#[must_use]
pub const fn sum(input_count: u32) -> SumNode {
    SumNode { input_count }
}

// ---------------------------------------------------------------------------
// WireNode
// ---------------------------------------------------------------------------

/// Channel selector/reorder node. Reads from `mapping.len()` distinct
/// input indices and writes them as outputs in the mapped order.
///
/// `mapping[i]` is the input channel index that feeds output channel `i`.
/// The number of inputs is `max(mapping) + 1`.
#[derive(Debug, Clone)]
pub struct WireNode {
    mapping: Vec<u32>,
    input_count: u32,
}

impl Node for WireNode {
    fn inputs(&self) -> u32 {
        self.input_count
    }
    fn outputs(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation)] // mapping len bounded by construction
        {
            self.mapping.len() as u32
        }
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        for (out_idx, &in_idx) in self.mapping.iter().enumerate() {
            outputs[out_idx][..frames].copy_from_slice(&inputs[in_idx as usize][..frames]);
        }
    }
    fn reset(&mut self) {}
}

/// Creates a channel routing node.
///
/// `mapping[i]` specifies which input channel feeds output channel `i`.
/// The node requires `max(mapping) + 1` inputs and produces `mapping.len()` outputs.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{Node, wire};
///
/// // A wire that swaps stereo channels:
/// // Output 0 takes Input 1, Output 1 takes Input 0.
/// let mut swap = wire(&[1, 0]);
/// assert_eq!(swap.inputs(), 2);
/// assert_eq!(swap.outputs(), 2);
/// ```
///
/// # Panics
///
/// Panics if `mapping` is empty.
#[must_use]
pub fn wire(mapping: &[u32]) -> WireNode {
    assert!(!mapping.is_empty(), "wire mapping must not be empty");
    let input_count = mapping.iter().copied().max().unwrap_or(0) + 1;
    WireNode {
        mapping: mapping.to_vec(),
        input_count,
    }
}
