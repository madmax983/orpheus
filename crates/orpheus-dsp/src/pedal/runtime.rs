//! Stateful runtime execution for virtual analog pedals.
//!
//! The `runtime` module takes an immutable [`PedalGraphProgram`](super::program::PedalGraphProgram)
//! and evaluates it over time. It maintains the necessary DSP state (like filter
//! histories, delay buffers, and phase accumulators) required by the static program
//! nodes.
//!
//! # Performance
//!
//! To optimize CPU usage, the runtime implements **control-rate sub-sampling**.
//! Nodes producing `Control` rate signals (like LFOs and Envelopes) are only
//! calculated every [`PEDAL_CONTROL_INTERVAL_SAMPLES`] frames (e.g. 16 frames).
//! Their output values are linearly interpolated for `Audio` rate nodes that
//! consume them.

#![allow(
    clippy::suboptimal_flops,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_map,
    clippy::redundant_closure_for_method_calls,
    clippy::default_constructed_unit_structs
)]
use std::sync::Arc;

use crate::command::PedalProgram;

use super::program::{
    ClipModel, FilterMode, NodeRef, PedalNode, PedalNodeKind, PedalStage, PreampModel, SignalKind,
    ToneModel,
};

/// Number of audio samples per control block evaluation.
pub const PEDAL_CONTROL_INTERVAL_SAMPLES: usize = 16;

/// Stateful runtime executing a `PedalGraphProgram`.
#[derive(Debug)]
pub struct PedalInstance {
    program: Arc<PedalProgram>,
    sample_rate_hz: f32,
    node_states: Vec<NodeState>,
    node_values: Vec<f32>,
    node_target_values: Vec<f32>,
    control_updates_each_sample: Vec<bool>,
    control_samples_until_update: usize,
    control_initialized: bool,
    tail_frames: u32,
}

impl PedalInstance {
    /// Create a new execution environment.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_dsp::{PedalInstance, PedalProgram};
    /// use std::sync::Arc;
    ///
    /// let program = Arc::new(PedalProgram::new("input |> output", "bypass"));
    /// let mut instance = PedalInstance::new(program, 44100.0);
    /// ```
    #[must_use]
    pub fn new(program: Arc<PedalProgram>, sample_rate_hz: f32) -> Self {
        let sample_rate_hz = sanitize_sample_rate(sample_rate_hz);
        let graph = program.graph();
        let control_updates_each_sample = derive_control_update_modes(graph.nodes());
        let node_states = graph
            .nodes()
            .iter()
            .map(|node| NodeState::for_node(node, sample_rate_hz))
            .collect::<Vec<_>>();
        let node_len = graph.nodes().len();
        let tail_frames = estimated_tail_frames(graph.nodes(), graph.output(), sample_rate_hz);
        let node_values = vec![0.0_f32; node_len];
        let node_target_values = vec![0.0_f32; node_len];
        Self {
            program,
            sample_rate_hz,
            node_states,
            node_values,
            node_target_values,
            control_updates_each_sample,
            control_samples_until_update: 0,
            control_initialized: false,
            tail_frames,
        }
    }

    #[doc(hidden)]
    pub fn reset(&mut self) {
        for (state, node) in self
            .node_states
            .iter_mut()
            .zip(self.program.graph().nodes().iter())
        {
            *state = NodeState::for_node(node, self.sample_rate_hz);
        }
        self.node_values.fill(0.0);
        self.node_target_values.fill(0.0);
        self.control_samples_until_update = 0;
        self.control_initialized = false;
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn tail_frames(&self) -> u32 {
        self.tail_frames
    }

    /// Evaluate a single audio frame.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_dsp::{PedalInstance, PedalProgram};
    /// use std::sync::Arc;
    ///
    /// let program = Arc::new(PedalProgram::new("input |> output", "bypass"));
    /// let mut instance = PedalInstance::new(program, 44100.0);
    /// let output = instance.process_sample(0.5);
    /// ```
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let graph = self.program.graph();
        if graph.is_bypass() {
            return sanitize_audio(input);
        }

        let nodes = graph.nodes();
        let update_control = self.control_samples_until_update == 0;
        if update_control {
            self.node_target_values.clone_from_slice(&self.node_values);
            for (index, node) in nodes.iter().enumerate() {
                if !matches!(node.signal_kind(), SignalKind::Control)
                    || self.control_updates_each_sample[index]
                {
                    continue;
                }

                let target = evaluate_node(
                    &mut self.node_states,
                    &self.node_target_values,
                    self.sample_rate_hz,
                    PEDAL_CONTROL_INTERVAL_SAMPLES,
                    index,
                    node,
                    input,
                );
                self.node_target_values[index] = target;
                if !self.control_initialized {
                    self.node_values[index] = target;
                }
            }
            self.control_samples_until_update = PEDAL_CONTROL_INTERVAL_SAMPLES;
        }

        if self.control_initialized {
            let smoothing = control_smoothing_coeff(self.sample_rate_hz);
            for (index, node) in nodes.iter().enumerate() {
                if !matches!(node.signal_kind(), SignalKind::Control)
                    || self.control_updates_each_sample[index]
                {
                    continue;
                }

                self.node_values[index] = smooth_control_value(
                    self.node_values[index],
                    self.node_target_values[index],
                    smoothing,
                );
            }
        }

        for (index, node) in nodes.iter().enumerate() {
            if !matches!(node.signal_kind(), SignalKind::Control)
                || !self.control_updates_each_sample[index]
            {
                continue;
            }

            let value = evaluate_node(
                &mut self.node_states,
                &self.node_values,
                self.sample_rate_hz,
                1,
                index,
                node,
                input,
            );
            self.node_values[index] = value;
            self.node_target_values[index] = value;
        }

        for (index, node) in nodes.iter().enumerate() {
            if matches!(node.signal_kind(), SignalKind::Control) {
                continue;
            }
            let value = evaluate_node(
                &mut self.node_states,
                &self.node_values,
                self.sample_rate_hz,
                1,
                index,
                node,
                input,
            );
            self.node_values[index] = value;
        }

        self.control_initialized = true;
        self.control_samples_until_update = self.control_samples_until_update.saturating_sub(1);

        resolve(&self.node_values, graph.output(), input)
    }

    /// Evaluate an entire block of audio.
    pub fn process_buffer(&mut self, input: &[f32], output: &mut [f32]) {
        for (index, out) in output.iter_mut().enumerate() {
            let input_sample = input.get(index).copied().unwrap_or_default();
            *out = self.process_sample(input_sample);
        }
    }
}

fn estimated_tail_frames(nodes: &[PedalNode], output: NodeRef, sample_rate_hz: f32) -> u32 {
    let mut memo = vec![None; nodes.len()];
    tail_frames_for_reference(nodes, output, sample_rate_hz, &mut memo)
}

fn tail_frames_for_reference(
    nodes: &[PedalNode],
    reference: NodeRef,
    sample_rate_hz: f32,
    memo: &mut [Option<u32>],
) -> u32 {
    match reference {
        NodeRef::Input => 0,
        NodeRef::Node(index) => {
            if let Some(Some(cached)) = memo.get(index) {
                return *cached;
            }

            let Some(node) = nodes.get(index) else {
                return 0;
            };
            if matches!(node.signal_kind(), SignalKind::Control) {
                if let Some(slot) = memo.get_mut(index) {
                    *slot = Some(0);
                }
                return 0;
            }

            let tail = match node.kind() {
                PedalNodeKind::Constant { .. }
                | PedalNodeKind::Lfo { .. }
                | PedalNodeKind::EnvFollow { .. } => 0,
                PedalNodeKind::Add { left, right } | PedalNodeKind::Mul { left, right } => {
                    tail_frames_for_reference(nodes, *left, sample_rate_hz, memo).max(
                        tail_frames_for_reference(nodes, *right, sample_rate_hz, memo),
                    )
                }
                PedalNodeKind::Stage(stage) => tail_frames_for_reference(
                    nodes,
                    stage_input_reference(stage),
                    sample_rate_hz,
                    memo,
                )
                .saturating_add(stage_own_tail_frames(nodes, stage, sample_rate_hz)),
                PedalNodeKind::Mix { inputs } => inputs
                    .iter()
                    .map(|reference| {
                        tail_frames_for_reference(nodes, *reference, sample_rate_hz, memo)
                    })
                    .max()
                    .unwrap_or(0),
                PedalNodeKind::Feedback {
                    input,
                    amount,
                    delay_samples,
                    tone_hz_bits,
                } => tail_frames_for_reference(nodes, *input, sample_rate_hz, memo).saturating_add(
                    feedback_own_tail_frames(
                        nodes,
                        *amount,
                        *delay_samples,
                        *tone_hz_bits,
                        sample_rate_hz,
                    ),
                ),
            };
            if let Some(slot) = memo.get_mut(index) {
                *slot = Some(tail);
            }
            tail
        }
    }
}

fn feedback_own_tail_frames(
    nodes: &[PedalNode],
    amount: NodeRef,
    delay_samples: usize,
    tone_hz_bits: Option<u32>,
    sample_rate_hz: f32,
) -> u32 {
    let delay_samples = u32::try_from(delay_samples.max(1)).unwrap_or(u32::MAX);
    let repeat_count = feedback_decay_repeat_count(feedback_tail_amount(nodes, amount));
    let tone_tail = if repeat_count == 0 {
        0
    } else {
        tone_hz_bits.map_or(0, |cutoff_hz| {
            one_pole_tail_frames(sample_rate_hz, f32::from_bits(cutoff_hz))
        })
    };
    delay_samples
        .saturating_mul(repeat_count)
        .saturating_add(tone_tail)
}

const fn stage_input_reference(stage: &PedalStage) -> NodeRef {
    match stage {
        PedalStage::Buffer { input }
        | PedalStage::Preamp { input, .. }
        | PedalStage::Gain { input, .. }
        | PedalStage::Clip { input, .. }
        | PedalStage::Tone { input, .. }
        | PedalStage::Filter { input, .. }
        | PedalStage::Eq { input, .. }
        | PedalStage::Level { input, .. }
        | PedalStage::Sag { input, .. }
        | PedalStage::Bias { input, .. } => *input,
    }
}

fn stage_own_tail_frames(nodes: &[PedalNode], stage: &PedalStage, sample_rate_hz: f32) -> u32 {
    match stage {
        PedalStage::Tone {
            model: ToneModel::Neutral,
            ..
        } => 0,
        PedalStage::Tone { cutoff_hz, .. } | PedalStage::Filter { cutoff_hz, .. } => {
            one_pole_tail_frames(
                sample_rate_hz,
                resolve_tail_cutoff_hz(nodes, *cutoff_hz).unwrap_or(20.0),
            )
        }
        PedalStage::Eq { .. } => one_pole_tail_frames(sample_rate_hz, 220.0),
        _ => 0,
    }
}

fn feedback_tail_amount(nodes: &[PedalNode], amount: NodeRef) -> f32 {
    if let Some(value) = control_exact_constant(nodes, amount) {
        return value.max(0.0).clamp(0.0, 0.98);
    }
    let range = control_range(nodes, amount);
    range.max.max(0.0).clamp(0.0, 0.98)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn feedback_decay_repeat_count(amount: f32) -> u32 {
    if amount <= f32::EPSILON {
        return 0;
    }

    let repeats = 1.0e-3_f32.log(amount).ceil();
    if repeats.is_finite() {
        (repeats as u32).max(1)
    } else {
        u32::MAX
    }
}

fn resolve_tail_cutoff_hz(nodes: &[PedalNode], reference: NodeRef) -> Option<f32> {
    match reference {
        NodeRef::Input => None,
        NodeRef::Node(index) => nodes
            .get(index)
            .and_then(|node| node.kind().constant_value())
            .map(sanitize_non_negative),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn one_pole_tail_frames(sample_rate_hz: f32, cutoff_hz: f32) -> u32 {
    let sample_rate_hz = sanitize_sample_rate(sample_rate_hz);
    let cutoff_hz = cutoff_hz.clamp(20.0, sample_rate_hz * 0.45);
    let per_sample_decay = (-core::f32::consts::TAU * cutoff_hz / sample_rate_hz).exp();
    let frames = 1.0e-3_f32.log(per_sample_decay).ceil();
    if frames.is_finite() {
        (frames as u32).max(1)
    } else {
        u32::MAX
    }
}

fn control_range(nodes: &[PedalNode], reference: NodeRef) -> ControlRange {
    let mut visiting = vec![false; nodes.len()];
    control_range_with(nodes, reference, &mut visiting)
}

fn control_exact_constant(nodes: &[PedalNode], reference: NodeRef) -> Option<f32> {
    let mut visiting = vec![false; nodes.len()];
    control_linear_expr_with(nodes, reference, &mut visiting)?.constant_value()
}

fn control_linear_expr_with(
    nodes: &[PedalNode],
    reference: NodeRef,
    visiting: &mut [bool],
) -> Option<LinearControlExpr> {
    match reference {
        NodeRef::Input => Some(LinearControlExpr::variable(NodeRef::Input)),
        NodeRef::Node(index) => {
            let node = nodes.get(index)?;
            if matches!(node.signal_kind(), SignalKind::Audio) {
                return Some(LinearControlExpr::variable(reference));
            }
            if visiting.get(index).copied().unwrap_or(false) {
                return None;
            }

            visiting[index] = true;
            let expr = match node.kind() {
                PedalNodeKind::Constant { value_bits } => {
                    Some(LinearControlExpr::constant(f32::from_bits(*value_bits)))
                }
                PedalNodeKind::Add { left, right } => Some(
                    control_linear_expr_with(nodes, *left, visiting)?
                        .add(control_linear_expr_with(nodes, *right, visiting)?),
                ),
                PedalNodeKind::Mul { left, right } => {
                    let left_expr = control_linear_expr_with(nodes, *left, visiting)?;
                    let right_expr = control_linear_expr_with(nodes, *right, visiting)?;
                    if let Some(scale) = left_expr.constant_value() {
                        Some(right_expr.scale(scale))
                    } else {
                        right_expr
                            .constant_value()
                            .map(|scale| left_expr.scale(scale))
                    }
                }
                PedalNodeKind::Lfo { .. }
                | PedalNodeKind::EnvFollow { .. }
                | PedalNodeKind::Stage(_)
                | PedalNodeKind::Mix { .. }
                | PedalNodeKind::Feedback { .. } => Some(LinearControlExpr::variable(reference)),
            };
            visiting[index] = false;
            expr
        }
    }
}

fn control_range_with(
    nodes: &[PedalNode],
    reference: NodeRef,
    visiting: &mut [bool],
) -> ControlRange {
    match reference {
        NodeRef::Input => ControlRange::unbounded(),
        NodeRef::Node(index) => {
            let Some(node) = nodes.get(index) else {
                return ControlRange::unbounded();
            };
            if visiting.get(index).copied().unwrap_or(false) {
                return ControlRange::unbounded();
            }

            visiting[index] = true;
            let range = match node.kind() {
                PedalNodeKind::Constant { value_bits } => {
                    let value = f32::from_bits(*value_bits);
                    ControlRange::new(value, value)
                }
                PedalNodeKind::Lfo {
                    depth_bits,
                    offset_bits,
                    ..
                } => {
                    let offset = f32::from_bits(*offset_bits);
                    let depth = f32::from_bits(*depth_bits).abs();
                    ControlRange::new(offset - depth, offset + depth)
                }
                PedalNodeKind::EnvFollow { input, .. } => {
                    let source_range = control_range_with(nodes, *input, visiting);
                    let upper = source_range.max_abs();
                    ControlRange::new(0.0, upper)
                }
                PedalNodeKind::Add { left, right } => control_range_with(nodes, *left, visiting)
                    .add(control_range_with(nodes, *right, visiting)),
                PedalNodeKind::Mul { left, right } => control_range_with(nodes, *left, visiting)
                    .mul(control_range_with(nodes, *right, visiting)),
                _ => ControlRange::unbounded(),
            };
            visiting[index] = false;
            range
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ControlRange {
    min: f32,
    max: f32,
}

impl ControlRange {
    const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    const fn unbounded() -> Self {
        Self::new(f32::NEG_INFINITY, f32::INFINITY)
    }

    const fn add(self, other: Self) -> Self {
        Self::new(self.min + other.min, self.max + other.max)
    }

    fn mul(self, other: Self) -> Self {
        let candidates = [
            multiply_range_bound(self.min, other.min),
            multiply_range_bound(self.min, other.max),
            multiply_range_bound(self.max, other.min),
            multiply_range_bound(self.max, other.max),
        ];
        let min = candidates.into_iter().fold(f32::INFINITY, f32::min);
        let max = candidates.into_iter().fold(f32::NEG_INFINITY, f32::max);
        Self::new(min, max)
    }

    const fn max_abs(self) -> f32 {
        if self.min.is_infinite() || self.max.is_infinite() {
            f32::INFINITY
        } else {
            self.min.abs().max(self.max.abs())
        }
    }
}

fn multiply_range_bound(left: f32, right: f32) -> f32 {
    if left == 0.0 || right == 0.0 {
        return 0.0;
    }
    if left.is_infinite() || right.is_infinite() {
        let sign = left.signum() * right.signum();
        return if sign.is_sign_negative() {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        };
    }
    left * right
}

#[derive(Clone, Debug)]
struct LinearControlExpr {
    constant: f32,
    terms: Vec<(NodeRef, f32)>,
}

impl LinearControlExpr {
    const fn constant(value: f32) -> Self {
        Self {
            constant: value,
            terms: Vec::new(),
        }
    }

    fn variable(reference: NodeRef) -> Self {
        Self {
            constant: 0.0,
            terms: vec![(reference, 1.0)],
        }
    }

    fn add(mut self, other: Self) -> Self {
        self.constant += other.constant;
        for (reference, coeff) in other.terms {
            self.add_term(reference, coeff);
        }
        self
    }

    fn scale(mut self, factor: f32) -> Self {
        self.constant *= factor;
        for (_, coeff) in &mut self.terms {
            *coeff *= factor;
        }
        self.terms.retain(|(_, coeff)| coeff.abs() > 1.0e-6);
        self
    }

    fn constant_value(&self) -> Option<f32> {
        if self.terms.is_empty() {
            Some(self.constant)
        } else {
            None
        }
    }

    fn add_term(&mut self, reference: NodeRef, coeff: f32) {
        if coeff.abs() <= 1.0e-6 {
            return;
        }
        if let Some((_, existing)) = self
            .terms
            .iter_mut()
            .find(|(existing_reference, _)| *existing_reference == reference)
        {
            *existing += coeff;
            if existing.abs() <= 1.0e-6 {
                *existing = 0.0;
            }
            self.terms.retain(|(_, coeff)| coeff.abs() > 1.0e-6);
            return;
        }
        self.terms.push((reference, coeff));
    }
}

fn derive_control_update_modes(nodes: &[PedalNode]) -> Vec<bool> {
    let mut updates_each_sample = vec![false; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        updates_each_sample[index] = match node.kind() {
            PedalNodeKind::EnvFollow { .. } => true,
            PedalNodeKind::Add { left, right } | PedalNodeKind::Mul { left, right }
                if matches!(node.signal_kind(), SignalKind::Control) =>
            {
                reference_updates_each_sample(nodes, &updates_each_sample, *left)
                    || reference_updates_each_sample(nodes, &updates_each_sample, *right)
            }
            _ => false,
        };
    }
    updates_each_sample
}

fn reference_updates_each_sample(
    nodes: &[PedalNode],
    updates_each_sample: &[bool],
    reference: NodeRef,
) -> bool {
    match reference {
        NodeRef::Input => true,
        NodeRef::Node(index) => nodes.get(index).is_some_and(|node| {
            matches!(node.signal_kind(), SignalKind::Audio)
                || updates_each_sample.get(index).copied().unwrap_or(false)
        }),
    }
}

const fn control_smoothing_coeff(sample_rate_hz: f32) -> f32 {
    let _ = sample_rate_hz;
    1.0
}

fn smooth_control_value(current: f32, target: f32, coeff: f32) -> f32 {
    let current = sanitize_audio(current);
    let target = sanitize_audio(target);
    if (target - current).abs() <= 1.0e-6 {
        target
    } else {
        sanitize_audio((target - current).mul_add(coeff, current))
    }
}

#[allow(clippy::cast_precision_loss)]
fn evaluate_node(
    node_states: &mut [NodeState],
    node_values: &[f32],
    sample_rate_hz: f32,
    sample_step: usize,
    index: usize,
    node: &PedalNode,
    input: f32,
) -> f32 {
    match node.kind() {
        PedalNodeKind::Constant { value_bits } => f32::from_bits(*value_bits),
        PedalNodeKind::Lfo {
            rate_hz_bits,
            depth_bits,
            offset_bits,
        } => {
            let NodeState::Lfo { phase } = &mut node_states[index] else {
                return 0.0;
            };
            let value = phase
                .sin()
                .mul_add(f32::from_bits(*depth_bits), f32::from_bits(*offset_bits));
            let increment =
                core::f32::consts::TAU * f32::from_bits(*rate_hz_bits) * (sample_step as f32)
                    / sample_rate_hz.max(1.0);
            *phase = (*phase + increment).rem_euclid(core::f32::consts::TAU);
            value
        }
        PedalNodeKind::EnvFollow {
            input: source,
            attack_ms_bits,
            release_ms_bits,
        } => {
            let target = resolve(node_values, *source, input).abs();
            let NodeState::EnvFollow { envelope } = &mut node_states[index] else {
                return target;
            };
            let effective_sample_rate = sample_rate_hz / (sample_step.max(1) as f32);
            let attack = smoothing_coeff(f32::from_bits(*attack_ms_bits), effective_sample_rate);
            let release = smoothing_coeff(f32::from_bits(*release_ms_bits), effective_sample_rate);
            let coeff = if target > *envelope { attack } else { release };
            *envelope += (target - *envelope) * coeff;
            *envelope
        }
        PedalNodeKind::Add { left, right } => {
            sanitize_audio(resolve(node_values, *left, input) + resolve(node_values, *right, input))
        }
        PedalNodeKind::Mul { left, right } => {
            sanitize_audio(resolve(node_values, *left, input) * resolve(node_values, *right, input))
        }
        PedalNodeKind::Stage(stage) => stage.process(node_states, node_values, index, input),
        PedalNodeKind::Mix { inputs } => sanitize_audio(
            inputs
                .iter()
                .map(|reference| resolve(node_values, *reference, input))
                .sum::<f32>(),
        ),
        PedalNodeKind::Feedback {
            input: source,
            amount,
            delay_samples,
            tone_hz_bits,
        } => {
            let signal = resolve(node_values, *source, input);
            let amount = resolve(node_values, *amount, input).clamp(0.0, 0.98);
            let NodeState::Feedback {
                buffer,
                write_index,
                low_pass,
            } = &mut node_states[index]
            else {
                return signal;
            };
            let delayed = buffer[*write_index];
            let feedback_signal = tone_hz_bits.as_ref().map_or(delayed, |cutoff_hz| {
                low_pass
                    .as_mut()
                    .expect("feedback tone filter state should exist")
                    .process_with_cutoff(delayed, f32::from_bits(*cutoff_hz))
            });
            buffer[*write_index] = sanitize_audio(feedback_signal.mul_add(amount, signal));
            *write_index += 1;
            if *write_index >= (*delay_samples).max(1) {
                *write_index = 0;
            }
            sanitize_audio(feedback_signal.mul_add(amount, signal))
        }
    }
}

struct StageContext<'a> {
    node_states: &'a mut [NodeState],
    node_values: &'a [f32],
    index: usize,
    input: f32,
}

impl PedalStage {
    #[allow(clippy::too_many_lines)]
    fn process(
        &self,
        node_states: &mut [NodeState],
        node_values: &[f32],
        index: usize,
        input: f32,
    ) -> f32 {
        match self {
            Self::Buffer { input: source } => resolve(node_values, *source, input),
            Self::Gain {
                input: source,
                amount,
            }
            | Self::Level {
                input: source,
                amount,
            } => sanitize_audio(
                resolve(node_values, *source, input) * resolve(node_values, *amount, input),
            ),
            Self::Preamp {
                input: source,
                gain,
                model,
            } => preamp_sample(
                resolve(node_values, *source, input),
                resolve(node_values, *gain, input),
                *model,
            ),
            Self::Clip {
                input: source,
                drive,
                model,
            } => clip_sample(
                resolve(node_values, *source, input),
                resolve(node_values, *drive, input),
                *model,
            ),
            Self::Tone {
                input: source,
                cutoff_hz,
                resonance,
                model,
            } => process_tone_stage(
                &mut StageContext {
                    node_states,
                    node_values,
                    index,
                    input,
                },
                *source,
                *cutoff_hz,
                *resonance,
                *model,
            ),
            Self::Filter {
                input: source,
                kind,
                cutoff_hz,
                resonance,
            } => process_filter_stage(
                &mut StageContext {
                    node_states,
                    node_values,
                    index,
                    input,
                },
                *source,
                *kind,
                *cutoff_hz,
                *resonance,
            ),
            Self::Eq {
                input: source,
                low,
                mid,
                high,
            } => process_eq_stage(
                &mut StageContext {
                    node_states,
                    node_values,
                    index,
                    input,
                },
                *source,
                *low,
                *mid,
                *high,
            ),
            Self::Sag {
                input: source,
                amount,
            } => process_sag_stage(
                &mut StageContext {
                    node_states,
                    node_values,
                    index,
                    input,
                },
                *source,
                *amount,
            ),
            Self::Bias {
                input: source,
                amount,
            } => sanitize_audio(
                resolve(node_values, *source, input) + resolve(node_values, *amount, input),
            ),
        }
    }
}

fn process_tone_stage(
    ctx: &mut StageContext,
    source: NodeRef,
    cutoff_hz: NodeRef,
    resonance: NodeRef,
    model: ToneModel,
) -> f32 {
    let signal = resolve(ctx.node_values, source, ctx.input);
    if matches!(model, ToneModel::Neutral) {
        return signal;
    }
    let cutoff = resolve(ctx.node_values, cutoff_hz, ctx.input);
    let resonance = resolve(ctx.node_values, resonance, ctx.input);
    let NodeState::Tone {
        low_pass,
        high_pass,
    } = &mut ctx.node_states[ctx.index]
    else {
        return signal;
    };
    let low = low_pass.process_with_cutoff(signal, cutoff);
    let high = high_pass.process_with_cutoff(signal, cutoff.max(60.0));
    let mid = signal - low - high;
    match model {
        ToneModel::Neutral => unreachable!("neutral tone bypasses filter state"),
        ToneModel::MidHump => {
            sanitize_audio(low.mul_add(0.55, mid * resonance.mul_add(0.9, 1.0)) + high * 0.12)
        }
        ToneModel::ScoopedStack => {
            sanitize_audio(low.mul_add(0.75, mid * resonance.mul_add(-0.15, 0.25)) + high * 0.85)
        }
    }
}

fn process_filter_stage(
    ctx: &mut StageContext,
    source: NodeRef,
    kind: FilterMode,
    cutoff_hz: NodeRef,
    resonance: NodeRef,
) -> f32 {
    let signal = resolve(ctx.node_values, source, ctx.input);
    let cutoff = resolve(ctx.node_values, cutoff_hz, ctx.input);
    let resonance = resolve(ctx.node_values, resonance, ctx.input).clamp(0.0, 0.95);
    match kind {
        FilterMode::LowPass => {
            let NodeState::LowPass(low_pass) = &mut ctx.node_states[ctx.index] else {
                return signal;
            };
            let resonant_input = sanitize_audio((low_pass.state * resonance).mul_add(-0.9, signal));
            low_pass.process_with_cutoff(resonant_input, cutoff)
        }
        FilterMode::HighPass => {
            let NodeState::HighPass(high_pass) = &mut ctx.node_states[ctx.index] else {
                return signal;
            };
            let resonant_input =
                sanitize_audio((high_pass.prev_output * resonance).mul_add(-0.75, signal));
            high_pass.process_with_cutoff(resonant_input, cutoff)
        }
    }
}

fn process_sag_stage(ctx: &mut StageContext, source: NodeRef, amount: NodeRef) -> f32 {
    let signal = resolve(ctx.node_values, source, ctx.input);
    let amount = resolve(ctx.node_values, amount, ctx.input).clamp(0.0, 1.0);
    let NodeState::Sag { envelope } = &mut ctx.node_states[ctx.index] else {
        return signal;
    };
    *envelope += (signal.abs() - *envelope) * 0.01;
    let reduction = 1.0 - (*envelope * amount * 0.35).clamp(0.0, 0.8);
    sanitize_audio(signal * reduction)
}

fn process_eq_stage(
    ctx: &mut StageContext,
    source: NodeRef,
    low: NodeRef,
    mid: NodeRef,
    high: NodeRef,
) -> f32 {
    let signal = resolve(ctx.node_values, source, ctx.input);
    let low_gain = resolve(ctx.node_values, low, ctx.input);
    let mid_gain = resolve(ctx.node_values, mid, ctx.input);
    let high_gain = resolve(ctx.node_values, high, ctx.input);
    let NodeState::Eq {
        low_pass,
        high_pass,
    } = &mut ctx.node_states[ctx.index]
    else {
        return signal;
    };
    let low_band = low_pass.process_with_cutoff(signal, 220.0);
    let high_band = high_pass.process_with_cutoff(signal, 3_200.0);
    let mid_band = signal - low_band - high_band;
    sanitize_audio((low_band * low_gain) + (mid_band * mid_gain) + (high_band * high_gain))
}

fn resolve(node_values: &[f32], reference: NodeRef, input: f32) -> f32 {
    match reference {
        NodeRef::Input => sanitize_audio(input),
        NodeRef::Node(index) => node_values.get(index).copied().unwrap_or_default(),
    }
}

#[derive(Debug)]
enum NodeState {
    None,
    Lfo {
        phase: f32,
    },
    EnvFollow {
        envelope: f32,
    },
    LowPass(LowPassState),
    HighPass(HighPassState),
    Tone {
        low_pass: LowPassState,
        high_pass: HighPassState,
    },
    Eq {
        low_pass: LowPassState,
        high_pass: HighPassState,
    },
    Sag {
        envelope: f32,
    },
    Feedback {
        buffer: Vec<f32>,
        write_index: usize,
        low_pass: Option<LowPassState>,
    },
}

impl NodeState {
    fn for_node(node: &PedalNode, sample_rate_hz: f32) -> Self {
        match node.kind() {
            PedalNodeKind::Lfo { .. } => Self::Lfo { phase: 0.0 },
            PedalNodeKind::EnvFollow { .. } => Self::EnvFollow { envelope: 0.0 },
            PedalNodeKind::Stage(PedalStage::Tone { model, .. }) => match model {
                ToneModel::Neutral => Self::None,
                ToneModel::MidHump | ToneModel::ScoopedStack => Self::Tone {
                    low_pass: LowPassState::new(sample_rate_hz),
                    high_pass: HighPassState::new(sample_rate_hz),
                },
            },
            PedalNodeKind::Stage(PedalStage::Filter { kind, .. }) => match kind {
                FilterMode::LowPass => Self::LowPass(LowPassState::new(sample_rate_hz)),
                FilterMode::HighPass => Self::HighPass(HighPassState::new(sample_rate_hz)),
            },
            PedalNodeKind::Stage(PedalStage::Eq { .. }) => Self::Eq {
                low_pass: LowPassState::new(sample_rate_hz),
                high_pass: HighPassState::new(sample_rate_hz),
            },
            PedalNodeKind::Stage(PedalStage::Sag { .. }) => Self::Sag { envelope: 0.0 },
            PedalNodeKind::Feedback {
                delay_samples,
                tone_hz_bits,
                ..
            } => Self::Feedback {
                buffer: vec![0.0; (*delay_samples).max(1)],
                write_index: 0,
                low_pass: tone_hz_bits.map(|_| LowPassState::new(sample_rate_hz)),
            },
            PedalNodeKind::Constant { .. }
            | PedalNodeKind::Add { .. }
            | PedalNodeKind::Mul { .. }
            | PedalNodeKind::Mix { .. }
            | PedalNodeKind::Stage(
                PedalStage::Buffer { .. }
                | PedalStage::Preamp { .. }
                | PedalStage::Gain { .. }
                | PedalStage::Clip { .. }
                | PedalStage::Level { .. }
                | PedalStage::Bias { .. },
            ) => Self::None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LowPassState {
    sample_rate_hz: f32,
    state: f32,
}

impl LowPassState {
    const fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            state: 0.0,
        }
    }

    fn process_with_cutoff(&mut self, input: f32, cutoff_hz: f32) -> f32 {
        let cutoff_hz = cutoff_hz.clamp(20.0, self.sample_rate_hz * 0.45);
        let g = 1.0 - (-core::f32::consts::TAU * cutoff_hz / self.sample_rate_hz).exp();
        self.state += g * (sanitize_audio(input) - self.state);
        sanitize_audio(self.state)
    }
}

#[derive(Clone, Copy, Debug)]
struct HighPassState {
    sample_rate_hz: f32,
    prev_input: f32,
    prev_output: f32,
}

impl HighPassState {
    const fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            prev_input: 0.0,
            prev_output: 0.0,
        }
    }

    fn process_with_cutoff(&mut self, input: f32, cutoff_hz: f32) -> f32 {
        let cutoff_hz = cutoff_hz.clamp(20.0, self.sample_rate_hz * 0.45);
        let omega = core::f32::consts::TAU * cutoff_hz / self.sample_rate_hz;
        let alpha = 1.0 / (1.0 + omega);
        let output = alpha * (self.prev_output + input - self.prev_input);
        self.prev_input = input;
        self.prev_output = output;
        sanitize_audio(output)
    }
}

fn preamp_sample(input: f32, gain: f32, model: PreampModel) -> f32 {
    let drive = sanitize_non_negative(gain);
    match model {
        PreampModel::JfetClean => ((input * drive.mul_add(0.45, 1.0)).tanh()) * 0.92,
        PreampModel::OpampTight => ((input * drive.mul_add(0.8, 1.0)).tanh()) * 0.78,
    }
}

fn clip_sample(input: f32, drive: f32, model: ClipModel) -> f32 {
    let drive = sanitize_non_negative(drive);
    match model {
        ClipModel::SiliconHard => (input * drive.mul_add(1.6, 1.0)).tanh(),
        ClipModel::GermaniumSoft => {
            let shaped = input * drive.mul_add(0.9, 1.0);
            ((shaped * 1.6).atan() / core::f32::consts::FRAC_PI_2) * 0.95
        }
        ClipModel::RedLed => {
            let shaped = input * drive.mul_add(1.1, 1.0);
            let positive = (shaped * 0.8).clamp(-1.0, 1.0);
            let negative = (shaped * 1.05).clamp(-1.0, 1.0);
            if shaped >= 0.0 { positive } else { negative }
        }
    }
}

fn sanitize_sample_rate(sample_rate_hz: f32) -> f32 {
    if sample_rate_hz.is_finite() && sample_rate_hz > 1.0 {
        sample_rate_hz
    } else {
        48_000.0
    }
}

const fn sanitize_audio(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

const fn sanitize_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn smoothing_coeff(ms: f32, sample_rate_hz: f32) -> f32 {
    let seconds = if ms.is_finite() {
        ms.max(0.0) / 1_000.0
    } else {
        0.01
    };
    if seconds <= f32::EPSILON {
        1.0
    } else {
        1.0 - (-1.0 / (seconds * sample_rate_hz.max(1.0))).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32;

    #[test]
    fn should_sanitize_audio() {
        assert!((sanitize_audio(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((sanitize_audio(-1.0) - -1.0).abs() < f32::EPSILON);
        assert!((sanitize_audio(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_audio(f32::INFINITY) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_audio(f32::NEG_INFINITY) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_audio(f32::NAN) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn should_sanitize_non_negative() {
        assert!((sanitize_non_negative(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((sanitize_non_negative(-1.0) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_non_negative(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_non_negative(f32::INFINITY) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_non_negative(f32::NEG_INFINITY) - 0.0).abs() < f32::EPSILON);
        assert!((sanitize_non_negative(f32::NAN) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn should_calculate_smoothing_coeff() {
        assert!((smoothing_coeff(0.0, 48000.0) - 1.0).abs() < f32::EPSILON);
        assert!((smoothing_coeff(-10.0, 48000.0) - 1.0).abs() < f32::EPSILON);
        assert!((smoothing_coeff(10.0, 48000.0) - 0.002_081_155_8).abs() < 1e-6);
        assert!((smoothing_coeff(f32::NAN, 48000.0) - 0.002_081_155_8).abs() < 1e-6); // falls back to 0.01s (10ms)
        assert!((smoothing_coeff(f32::INFINITY, 48000.0) - 0.002_081_155_8).abs() < 1e-6); // falls back to 0.01s
    }

    #[test]
    fn should_process_lowpass() {
        let mut filter = LowPassState::new(48000.0);
        let out1 = filter.process_with_cutoff(1.0, 1000.0);
        assert!((out1 - 0.122_745_94).abs() < 1e-3);

        let mut filter_nan = LowPassState::new(48000.0);
        let out_nan = filter_nan.process_with_cutoff(f32::NAN, 1000.0);
        assert!((out_nan - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn should_process_highpass() {
        let mut filter = HighPassState::new(48000.0);
        let out1 = filter.process_with_cutoff(1.0, 1000.0);
        assert!((out1 - 0.884_163).abs() < 1e-3);

        let out_nan = filter.process_with_cutoff(f32::NAN, 1000.0);
        assert!((out_nan - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn should_apply_preamp_models() {
        let jfet = preamp_sample(1.0, 0.5, PreampModel::JfetClean);
        assert!(jfet > 0.0 && jfet < 1.0);
        let opamp = preamp_sample(1.0, 0.5, PreampModel::OpampTight);
        assert!(opamp > 0.0 && opamp < 1.0);

        let nan_jfet = preamp_sample(f32::NAN, 0.5, PreampModel::JfetClean);
        assert!(nan_jfet.is_nan()); // Preamp doesn't sanitize NaN input
    }

    #[test]
    fn should_apply_clip_models() {
        let silicon = clip_sample(1.0, 0.5, ClipModel::SiliconHard);
        assert!(silicon > 0.0 && silicon < 1.0);
        let germanium = clip_sample(1.0, 0.5, ClipModel::GermaniumSoft);
        assert!(germanium > 0.0 && germanium < 1.0);
        let led = clip_sample(1.0, 0.5, ClipModel::RedLed);
        assert!(led > 0.0 && led < 2.0);

        let nan_silicon = clip_sample(f32::NAN, 0.5, ClipModel::SiliconHard);
        assert!(nan_silicon.is_nan()); // Clip doesn't sanitize NaN input
    }
}
