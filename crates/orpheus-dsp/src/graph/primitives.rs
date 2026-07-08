//! Leaf DSP nodes for the graph combinator system.
//!
//! These are the fundamental building blocks that don't wrap existing `synth/`
//! primitives — they're either new (sine, one-pole, delay) or structural
//! utilities (constant, passthrough, wire, sum).

use std::f32::consts::{FRAC_PI_4, TAU};

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
// Envelope core (shared by AdsrNode and ArNode)
// ---------------------------------------------------------------------------

/// Falls back to 48 kHz for non-finite or non-positive sample rates.
fn sanitize_sample_rate(sample_rate_hz: f32) -> f32 {
    if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
        sample_rate_hz
    } else {
        48_000.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Per-sample linear envelope state machine shared by [`AdsrNode`] and [`ArNode`].
///
/// Segments are linear ramps (matching the piecewise-linear style of the other
/// leaf nodes; no existing envelope code suggested exponential curves):
///
/// - Attack: rises to 1.0 with slope `1 / (attack_s * sr)`, so a fresh trigger
///   reaches full level in exactly `attack_s` seconds.
/// - Decay: falls from 1.0 to `sustain_level` in `decay_s` seconds.
/// - Release: falls from the level at gate-off to 0.0 in `release_s` seconds
///   (the slope is captured when the gate falls, so shorter-than-full levels
///   still take the full `release_s` to fade).
///
/// A gate rising edge (or a high gate while idle) starts the attack from the
/// *current* level, so retriggering mid-release never produces a click.
/// Non-finite or non-positive segment times jump the segment in one sample.
#[derive(Debug, Clone)]
struct EnvCore {
    sample_rate_hz: f32,
    stage: EnvStage,
    level: f32,
    release_step: f32,
    prev_gate: f32,
}

impl EnvCore {
    const fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            stage: EnvStage::Idle,
            level: 0.0,
            release_step: 0.0,
            prev_gate: 0.0,
        }
    }

    const fn reset(&mut self) {
        self.stage = EnvStage::Idle;
        self.level = 0.0;
        self.release_step = 0.0;
        self.prev_gate = 0.0;
    }

    /// Advances the envelope by one sample and returns the new level.
    fn tick(
        &mut self,
        gate: f32,
        attack_s: f32,
        decay_s: f32,
        sustain_level: f32,
        release_s: f32,
    ) -> f32 {
        let rising = gate > 0.0 && self.prev_gate <= 0.0;
        self.prev_gate = gate;

        if rising || (gate > 0.0 && self.stage == EnvStage::Idle) {
            // (Re)trigger: restart the attack from the current level.
            self.stage = EnvStage::Attack;
        } else if gate <= 0.0
            && matches!(
                self.stage,
                EnvStage::Attack | EnvStage::Decay | EnvStage::Sustain
            )
        {
            self.stage = EnvStage::Release;
            self.release_step = if release_s.is_finite() && release_s > 0.0 {
                self.level / (release_s * self.sample_rate_hz)
            } else {
                1.0
            };
        }

        match self.stage {
            EnvStage::Idle => self.level = 0.0,
            EnvStage::Attack => {
                let step = if attack_s.is_finite() && attack_s > 0.0 {
                    1.0 / (attack_s * self.sample_rate_hz)
                } else {
                    1.0
                };
                self.level += step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = EnvStage::Decay;
                }
            }
            EnvStage::Decay => {
                let sustain = sustain_level.clamp(0.0, 1.0);
                let step = if decay_s.is_finite() && decay_s > 0.0 {
                    (1.0 - sustain) / (decay_s * self.sample_rate_hz)
                } else {
                    1.0
                };
                self.level -= step;
                if self.level <= sustain {
                    self.level = sustain;
                    self.stage = EnvStage::Sustain;
                }
            }
            EnvStage::Sustain => self.level = sustain_level.clamp(0.0, 1.0),
            EnvStage::Release => {
                self.level -= self.release_step;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = EnvStage::Idle;
                }
            }
        }

        self.level
    }
}

// ---------------------------------------------------------------------------
// AdsrNode
// ---------------------------------------------------------------------------

/// A gate-driven linear ADSR envelope generator.
/// 5 inputs (gate, attack\_s, decay\_s, sustain\_level, release\_s), 1 output (level).
///
/// The gate opens on a rising edge (or any positive gate while idle) and
/// closes when it falls to <= 0. Retriggering while active restarts the
/// attack from the current level, avoiding clicks. See [`EnvCore`] for the
/// segment semantics.
#[derive(Debug, Clone)]
pub struct AdsrNode {
    core: EnvCore,
}

impl Node for AdsrNode {
    fn inputs(&self) -> u32 {
        5
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let gate = inputs[0];
        let attack = inputs[1];
        let decay = inputs[2];
        let sustain = inputs[3];
        let release = inputs[4];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self
                .core
                .tick(gate[i], attack[i], decay[i], sustain[i], release[i]);
        }
    }
    fn reset(&mut self) {
        self.core.reset();
    }
}

/// Creates an ADSR envelope node.
/// 5 inputs (gate, attack\_s, decay\_s, sustain\_level, release\_s), 1 output.
#[must_use]
pub fn adsr(sample_rate_hz: f32) -> AdsrNode {
    AdsrNode {
        core: EnvCore::new(sanitize_sample_rate(sample_rate_hz)),
    }
}

// ---------------------------------------------------------------------------
// ArNode
// ---------------------------------------------------------------------------

/// A gate-driven attack/release envelope generator.
/// 3 inputs (gate, attack\_s, release\_s), 1 output (level).
///
/// Semantically an ADSR with zero decay and full sustain, but exposed as its
/// own node so the unused decay/sustain channels don't appear as inputs.
#[derive(Debug, Clone)]
pub struct ArNode {
    core: EnvCore,
}

impl Node for ArNode {
    fn inputs(&self) -> u32 {
        3
    }
    fn outputs(&self) -> u32 {
        1
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let gate = inputs[0];
        let attack = inputs[1];
        let release = inputs[2];
        let out = &mut outputs[0];
        for i in 0..frames {
            out[i] = self.core.tick(gate[i], attack[i], 0.0, 1.0, release[i]);
        }
    }
    fn reset(&mut self) {
        self.core.reset();
    }
}

/// Creates an attack/release envelope node.
/// 3 inputs (gate, attack\_s, release\_s), 1 output.
#[must_use]
pub fn ar(sample_rate_hz: f32) -> ArNode {
    ArNode {
        core: EnvCore::new(sanitize_sample_rate(sample_rate_hz)),
    }
}

// ---------------------------------------------------------------------------
// PanNode
// ---------------------------------------------------------------------------

/// An equal-power stereo panner.
/// 2 inputs (audio, position in \[-1, 1\]), 2 outputs (left, right).
///
/// Uses the sin/cos equal-power law: total energy (L² + R²) is constant
/// across positions, and center (position 0) sits at -3 dB per side
/// (gain ≈ 0.7071). Position is clamped to \[-1, 1\].
#[derive(Debug, Clone)]
pub struct PanNode;

impl Node for PanNode {
    fn inputs(&self) -> u32 {
        2
    }
    fn outputs(&self) -> u32 {
        2
    }
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let audio = inputs[0];
        let position = inputs[1];
        let [left, right, ..] = outputs else {
            return;
        };
        for i in 0..frames {
            let pos = if position[i].is_finite() {
                position[i].clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let angle = (pos + 1.0) * FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            left[i] = audio[i] * cos;
            right[i] = audio[i] * sin;
        }
    }
    fn reset(&mut self) {}
}

/// Creates an equal-power stereo panner node.
/// 2 inputs (audio, position), 2 outputs (left, right).
#[must_use]
pub const fn pan() -> PanNode {
    PanNode
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
