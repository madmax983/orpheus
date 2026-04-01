use std::sync::Arc;

use crate::command::PedalProgram;

use super::program::{
    ClipModel, FilterMode, NodeRef, PedalNode, PedalNodeKind, PedalStage, PreampModel, ToneModel,
};

#[derive(Debug)]
pub struct PedalInstance {
    program: Arc<PedalProgram>,
    sample_rate_hz: f32,
    node_states: Vec<NodeState>,
    node_values: Vec<f32>,
}

impl PedalInstance {
    #[must_use]
    pub fn new(program: Arc<PedalProgram>, sample_rate_hz: f32) -> Self {
        let sample_rate_hz = sanitize_sample_rate(sample_rate_hz);
        let node_states = program
            .graph()
            .nodes()
            .iter()
            .map(|node| NodeState::for_node(node, sample_rate_hz))
            .collect::<Vec<_>>();
        let node_values = vec![0.0_f32; program.graph().nodes().len()];
        Self {
            program,
            sample_rate_hz,
            node_states,
            node_values,
        }
    }

    pub fn reset(&mut self) {
        for (state, node) in self
            .node_states
            .iter_mut()
            .zip(self.program.graph().nodes().iter())
        {
            *state = NodeState::for_node(node, self.sample_rate_hz);
        }
        self.node_values.fill(0.0);
    }

    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let graph = self.program.graph();
        if graph.is_bypass() {
            return sanitize_audio(input);
        }

        let nodes = graph.nodes();
        for (index, node) in nodes.iter().enumerate() {
            let value = evaluate_node(
                &mut self.node_states,
                &self.node_values,
                self.sample_rate_hz,
                index,
                node,
                input,
            );
            self.node_values[index] = value;
        }

        resolve(&self.node_values, graph.output(), input)
    }

    pub fn process_buffer(&mut self, input: &[f32], output: &mut [f32]) {
        for (index, out) in output.iter_mut().enumerate() {
            let input_sample = input.get(index).copied().unwrap_or_default();
            *out = self.process_sample(input_sample);
        }
    }
}

fn evaluate_node(
    node_states: &mut [NodeState],
    node_values: &[f32],
    sample_rate_hz: f32,
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
            let value = f32::from_bits(*offset_bits) + phase.sin() * f32::from_bits(*depth_bits);
            let increment =
                core::f32::consts::TAU * f32::from_bits(*rate_hz_bits) / sample_rate_hz.max(1.0);
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
            let attack = smoothing_coeff(f32::from_bits(*attack_ms_bits), sample_rate_hz);
            let release = smoothing_coeff(f32::from_bits(*release_ms_bits), sample_rate_hz);
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
        PedalNodeKind::Stage(stage) => process_stage(node_states, node_values, index, stage, input),
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
            let feedback_signal = if let Some(cutoff_hz) = tone_hz_bits {
                low_pass
                    .as_mut()
                    .expect("feedback tone filter state should exist")
                    .process_with_cutoff(delayed, f32::from_bits(*cutoff_hz))
            } else {
                delayed
            };
            buffer[*write_index] = sanitize_audio(signal + (feedback_signal * amount));
            *write_index += 1;
            if *write_index >= (*delay_samples).max(1) {
                *write_index = 0;
            }
            sanitize_audio(signal + (feedback_signal * amount))
        }
    }
}

fn process_stage(
    node_states: &mut [NodeState],
    node_values: &[f32],
    index: usize,
    stage: &PedalStage,
    input: f32,
) -> f32 {
    match stage {
        PedalStage::Buffer { input: source } => resolve(node_values, *source, input),
        PedalStage::Gain {
            input: source,
            amount,
        }
        | PedalStage::Level {
            input: source,
            amount,
        } => sanitize_audio(
            resolve(node_values, *source, input) * resolve(node_values, *amount, input),
        ),
        PedalStage::Preamp {
            input: source,
            gain,
            model,
        } => preamp_sample(
            resolve(node_values, *source, input),
            resolve(node_values, *gain, input),
            *model,
        ),
        PedalStage::Clip {
            input: source,
            drive,
            model,
        } => clip_sample(
            resolve(node_values, *source, input),
            resolve(node_values, *drive, input),
            *model,
        ),
        PedalStage::Tone {
            input: source,
            cutoff_hz,
            resonance,
            model,
        } => {
            let signal = resolve(node_values, *source, input);
            let cutoff = resolve(node_values, *cutoff_hz, input);
            let resonance = resolve(node_values, *resonance, input);
            let NodeState::Tone {
                low_pass,
                high_pass,
            } = &mut node_states[index]
            else {
                return signal;
            };
            let low = low_pass.process_with_cutoff(signal, cutoff);
            let high = high_pass.process_with_cutoff(signal, cutoff.max(60.0));
            let mid = signal - low - high;
            match model {
                ToneModel::Neutral => sanitize_audio(low + mid + high),
                ToneModel::MidHump => {
                    sanitize_audio(low.mul_add(0.55, mid * (1.0 + resonance * 0.9)) + high * 0.12)
                }
                ToneModel::ScoopedStack => {
                    sanitize_audio(low.mul_add(0.75, mid * (0.25 - resonance * 0.15)) + high * 0.85)
                }
            }
        }
        PedalStage::Filter {
            input: source,
            kind,
            cutoff_hz,
            resonance,
        } => {
            let signal = resolve(node_values, *source, input);
            let cutoff = resolve(node_values, *cutoff_hz, input);
            let resonance = resolve(node_values, *resonance, input).clamp(0.0, 0.95);
            match kind {
                FilterMode::LowPass => {
                    let NodeState::LowPass(low_pass) = &mut node_states[index] else {
                        return signal;
                    };
                    let resonant_input =
                        sanitize_audio(signal - (low_pass.state * resonance * 0.9));
                    low_pass.process_with_cutoff(resonant_input, cutoff)
                }
                FilterMode::HighPass => {
                    let NodeState::HighPass(high_pass) = &mut node_states[index] else {
                        return signal;
                    };
                    let resonant_input =
                        sanitize_audio(signal - (high_pass.prev_output * resonance * 0.75));
                    high_pass.process_with_cutoff(resonant_input, cutoff)
                }
            }
        }
        PedalStage::Eq {
            input: source,
            low,
            mid,
            high,
        } => {
            let signal = resolve(node_values, *source, input);
            let low_gain = resolve(node_values, *low, input);
            let mid_gain = resolve(node_values, *mid, input);
            let high_gain = resolve(node_values, *high, input);
            let NodeState::Eq {
                low_pass,
                high_pass,
            } = &mut node_states[index]
            else {
                return signal;
            };
            let low_band = low_pass.process_with_cutoff(signal, 220.0);
            let high_band = high_pass.process_with_cutoff(signal, 3_200.0);
            let mid_band = signal - low_band - high_band;
            sanitize_audio((low_band * low_gain) + (mid_band * mid_gain) + (high_band * high_gain))
        }
        PedalStage::Sag {
            input: source,
            amount,
        } => {
            let signal = resolve(node_values, *source, input);
            let amount = resolve(node_values, *amount, input).clamp(0.0, 1.0);
            let NodeState::Sag { envelope } = &mut node_states[index] else {
                return signal;
            };
            *envelope += (signal.abs() - *envelope) * 0.01;
            let reduction = 1.0 - (*envelope * amount * 0.35).clamp(0.0, 0.8);
            sanitize_audio(signal * reduction)
        }
        PedalStage::Bias {
            input: source,
            amount,
        } => sanitize_audio(
            resolve(node_values, *source, input) + resolve(node_values, *amount, input),
        ),
    }
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
            PedalNodeKind::Stage(PedalStage::Tone { .. }) => Self::Tone {
                low_pass: LowPassState::new(sample_rate_hz),
                high_pass: HighPassState::new(sample_rate_hz),
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
        PreampModel::JfetClean => ((input * (1.0 + drive * 0.45)).tanh()) * 0.92,
        PreampModel::OpampTight => ((input * (1.0 + drive * 0.8)).tanh()) * 0.78,
    }
}

fn clip_sample(input: f32, drive: f32, model: ClipModel) -> f32 {
    let drive = sanitize_non_negative(drive);
    match model {
        ClipModel::SiliconHard => (input * (1.0 + drive * 1.6)).tanh(),
        ClipModel::GermaniumSoft => {
            let shaped = input * (1.0 + drive * 0.9);
            ((shaped * 1.6).atan() / core::f32::consts::FRAC_PI_2) * 0.95
        }
        ClipModel::RedLed => {
            let shaped = input * (1.0 + drive * 1.1);
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

fn sanitize_audio(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

fn sanitize_non_negative(value: f32) -> f32 {
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
