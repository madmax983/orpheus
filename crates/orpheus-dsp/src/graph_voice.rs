//! Graph-backed engine voices (ADR 0004 follow-up, ADR 0009).
//!
//! This module wires the Faust-style graph combinator system into the render
//! engine's voice lifecycle. A [`GraphVoiceProgram`] describes a playable
//! stereo voice as a compiled graph [`Processor`] with the fixed interface
//!
//! ```text
//! inputs:  [gate, freq_hz, gain, pan]   (params as signals, ADR 0004)
//! outputs: [left, right]
//! ```
//!
//! Allocation discipline: programs are compiled into a fixed pool of
//! [`GraphVoice`]s at engine construction time (off the audio thread) and
//! warmed with [`GraphVoice::prepare`] so lazy-but-once scratch growth also
//! happens off-thread. Triggering and rendering a pooled voice performs no
//! heap allocation; finished voices are reset in place and reused instead of
//! being dropped on the audio thread.

use thiserror::Error;

use crate::SampleTrigger;
use crate::graph::{
    Node, Processor, Seq, adsr, ar, bind, constant, gain_node, ladder_filter, noise, pan, par,
    passthrough, pulse, saw, seq, sine, soft_sat, sum, tri, wire,
};
use crate::routing::TrackId;

/// Pooled voices per program: how many simultaneous notes one graph program
/// can sound before further triggers are dropped.
const GRAPH_VOICE_POLYPHONY: usize = 8;

/// Output trim applied to graph voices, matching the analog-voice headroom
/// convention in `voice.rs`.
const GRAPH_OUTPUT_TRIM: f32 = 0.35;

/// A named, buildable graph voice program.
///
/// The builder function compiles a graph with the fixed voice interface:
/// 4 inputs (gate, freq\_hz, gain, pan) and 2 outputs (left, right).
#[derive(Clone, Copy, Debug)]
pub struct GraphVoiceProgram {
    token: &'static str,
    release_seconds: f32,
    build: fn(f32) -> Processor,
}

impl GraphVoiceProgram {
    /// The pattern-token this program is selected by (e.g. `"gsine"`).
    #[must_use]
    pub const fn token(&self) -> &'static str {
        self.token
    }

    /// How long the voice keeps sounding after its gate falls, in seconds.
    #[must_use]
    pub const fn release_seconds(&self) -> f32 {
        self.release_seconds
    }

    /// Compiles the program into an unprepared [`GraphVoice`].
    ///
    /// Construction may allocate; call it off the audio thread and follow up
    /// with [`GraphVoice::prepare`] before real-time use.
    #[must_use]
    pub fn build_voice(&self, sample_rate_hz: f32) -> GraphVoice {
        GraphVoice {
            processor: (self.build)(sample_rate_hz),
        }
    }

    /// The release tail length in frames at `sample_rate_hz`.
    fn release_frames(&self, sample_rate_hz: f32) -> u32 {
        release_seconds_to_frames(self.release_seconds, sample_rate_hz)
    }
}

/// The built-in graph voice programs available to the engine.
///
/// Currently one program:
///
/// - `gsine` — the gated sine voice from the graph integration suite:
///   sine carrier multiplied by a gate-driven ADSR envelope, scaled by the
///   trigger gain, and placed in the stereo field by an equal-power panner.
#[must_use]
pub const fn builtin_graph_voice_programs() -> &'static [GraphVoiceProgram] {
    const PROGRAMS: [GraphVoiceProgram; 1] = [GraphVoiceProgram {
        token: "gsine",
        release_seconds: GSINE_RELEASE_SECONDS,
        build: build_gsine,
    }];
    &PROGRAMS
}

const GSINE_ATTACK_SECONDS: f32 = 0.001;
const GSINE_DECAY_SECONDS: f32 = 0.01;
const GSINE_SUSTAIN_LEVEL: f32 = 0.7;
const GSINE_RELEASE_SECONDS: f32 = 0.02;

/// Builds the `gsine` voice graph: `[gate, freq, gain, pan] -> [L, R]`.
fn build_gsine(sample_rate_hz: f32) -> Processor {
    let envelope = bind(
        adsr(sample_rate_hz),
        &[
            (1, GSINE_ATTACK_SECONDS),
            (2, GSINE_DECAY_SECONDS),
            (3, GSINE_SUSTAIN_LEVEL),
            (4, GSINE_RELEASE_SECONDS),
        ],
    )
    .unwrap_or_else(|error| panic!("gsine envelope bindings must be valid: {error}"));

    // [gate, freq, gain, pan] -> [freq, gate, gain, pan]
    let reorder = wire(&[1, 0, 2, 3]);
    // -> [osc, level, gain, pan]
    let sources = par(sine(sample_rate_hz), par(envelope, passthrough(2)));
    // -> [osc * level, gain, pan]
    let enveloped = par(gain_node(), passthrough(2));
    // -> [audio * gain, pan]
    let levelled = par(gain_node(), passthrough(1));
    // -> [left, right]
    let stereo = pan();

    let graph = seq(
        reorder,
        seq(
            sources,
            seq(
                enveloped,
                seq(levelled, stereo)
                    .unwrap_or_else(|error| panic!("gsine pan stage must compose: {error}")),
            )
            .unwrap_or_else(|error| panic!("gsine gain stage must compose: {error}")),
        )
        .unwrap_or_else(|error| panic!("gsine envelope stage must compose: {error}")),
    )
    .unwrap_or_else(|error| panic!("gsine input reorder must compose: {error}"));

    debug_assert_eq!(graph.inputs(), 4);
    debug_assert_eq!(graph.outputs(), 2);
    Processor::new(graph)
}

/// A reference to a signal available to a [`VoiceNodeSpec`].
///
/// Specs form a DAG over a flat node list: a node may read the per-note gate,
/// the per-note frequency, or the output of any node defined before it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceSignalRef {
    /// The per-note gate (1 while the pattern event span holds, then 0).
    Gate,
    /// The per-note frequency in Hertz.
    Freq,
    /// The output of the node at this index in the spec's node list.
    Node(u32),
}

/// One node in a declarative [`GraphVoiceSpec`].
///
/// Each variant maps onto one existing graph-module primitive; the spec is a
/// data-only wiring of that vocabulary, so it stays `Clone`/`PartialEq` and
/// can travel through [`crate::EngineCommand`]s.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceNodeSpec {
    /// A constant control value.
    Constant {
        /// The constant value produced every frame.
        value: f32,
    },
    /// A sine oscillator driven by `freq`.
    Sine {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
    },
    /// A band-limited saw oscillator driven by `freq`.
    Saw {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
    },
    /// A triangle oscillator driven by `freq`.
    Tri {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
    },
    /// A band-limited pulse oscillator driven by `freq` and `width`.
    Pulse {
        /// The frequency signal in Hertz.
        freq: VoiceSignalRef,
        /// The pulse width (duty cycle) signal in \[0, 1\].
        width: VoiceSignalRef,
    },
    /// Deterministic white noise.
    Noise {
        /// The PRNG seed; equal seeds produce identical noise.
        seed: u32,
    },
    /// A gate-driven ADSR envelope with fixed segment parameters.
    Adsr {
        /// The gate signal opening and closing the envelope.
        gate: VoiceSignalRef,
        /// Attack time in seconds.
        attack_s: f32,
        /// Decay time in seconds.
        decay_s: f32,
        /// Sustain level in \[0, 1\].
        sustain: f32,
        /// Release time in seconds.
        release_s: f32,
    },
    /// A gate-driven attack/release envelope with fixed segment parameters.
    Ar {
        /// The gate signal opening and closing the envelope.
        gate: VoiceSignalRef,
        /// Attack time in seconds.
        attack_s: f32,
        /// Release time in seconds.
        release_s: f32,
    },
    /// A 4-stage ladder low-pass filter.
    Lowpass {
        /// The audio signal to filter.
        input: VoiceSignalRef,
        /// The cutoff frequency signal in Hertz.
        cutoff_hz: VoiceSignalRef,
        /// The resonance signal.
        resonance: VoiceSignalRef,
    },
    /// Soft saturation.
    Drive {
        /// The audio signal to saturate.
        input: VoiceSignalRef,
        /// The drive amount signal.
        amount: VoiceSignalRef,
    },
    /// Multiplies two signals (ring mod, envelope application, gain).
    Mul {
        /// The left operand.
        left: VoiceSignalRef,
        /// The right operand.
        right: VoiceSignalRef,
    },
    /// Sums two signals.
    Add {
        /// The left operand.
        left: VoiceSignalRef,
        /// The right operand.
        right: VoiceSignalRef,
    },
}

impl VoiceNodeSpec {
    /// The signal references this node reads, in primitive input order.
    fn input_refs(&self) -> Vec<VoiceSignalRef> {
        match self {
            Self::Constant { .. } | Self::Noise { .. } => Vec::new(),
            Self::Sine { freq } | Self::Saw { freq } | Self::Tri { freq } => vec![*freq],
            Self::Pulse { freq, width } => vec![*freq, *width],
            Self::Adsr { gate, .. } | Self::Ar { gate, .. } => vec![*gate],
            Self::Lowpass {
                input,
                cutoff_hz,
                resonance,
            } => vec![*input, *cutoff_hz, *resonance],
            Self::Drive { input, amount } => vec![*input, *amount],
            Self::Mul { left, right } | Self::Add { left, right } => vec![*left, *right],
        }
    }

    /// Whether every fixed parameter is finite (and non-negative where the
    /// primitive expects a duration or level).
    fn parameters_are_valid(&self) -> bool {
        match self {
            Self::Constant { value } => value.is_finite(),
            Self::Adsr {
                attack_s,
                decay_s,
                sustain,
                release_s,
                ..
            } => [*attack_s, *decay_s, *sustain, *release_s]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0),
            Self::Ar {
                attack_s,
                release_s,
                ..
            } => [*attack_s, *release_s]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0),
            Self::Sine { .. }
            | Self::Saw { .. }
            | Self::Tri { .. }
            | Self::Pulse { .. }
            | Self::Noise { .. }
            | Self::Lowpass { .. }
            | Self::Drive { .. }
            | Self::Mul { .. }
            | Self::Add { .. } => true,
        }
    }
}

/// Validation errors for [`GraphVoiceSpec::new`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GraphVoiceSpecError {
    /// The pattern token was empty or contained whitespace.
    #[error("voice program token must be a non-empty single word")]
    InvalidToken,
    /// The spec contained no nodes.
    #[error("voice program body must contain at least one node")]
    EmptyBody,
    /// A node referenced itself or a later node.
    #[error("voice node {node} references node {reference}, which is not defined before it")]
    ForwardReference {
        /// The index of the offending node.
        node: usize,
        /// The out-of-range reference.
        reference: usize,
    },
    /// The output referenced a node index outside the node list.
    #[error("voice program output references node {reference}, which does not exist")]
    OutputOutOfRange {
        /// The out-of-range reference.
        reference: usize,
    },
    /// A node carried a non-finite (or negative duration/level) parameter.
    #[error("voice node {node} has a non-finite or negative parameter")]
    InvalidParameter {
        /// The index of the offending node.
        node: usize,
    },
    /// The release tail was not finite and non-negative.
    #[error("voice program release must be finite and non-negative")]
    InvalidRelease,
}

/// A declarative, user-definable graph voice program.
///
/// Where [`GraphVoiceProgram`] describes a built-in voice through a static
/// builder function, a `GraphVoiceSpec` is pure data: a flat DAG of
/// [`VoiceNodeSpec`]s over the existing graph vocabulary, validated at
/// construction so compilation cannot fail. Compilation lowers each node onto
/// its graph-module primitive and wires the DAG with `seq`/`par`/`wire`
/// combinators into the fixed voice interface
/// `[gate, freq_hz, gain, pan] -> [left, right]` (the per-trigger gain and
/// equal-power pan stages are appended automatically).
///
/// Construction and compilation may allocate and must happen off the audio
/// thread; the compiled [`GraphVoice`] follows the pooled discipline of
/// ADR 0009.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphVoiceSpec {
    token: Box<str>,
    release_seconds: f32,
    nodes: Vec<VoiceNodeSpec>,
    output: VoiceSignalRef,
}

impl GraphVoiceSpec {
    /// Validates and builds a voice spec.
    ///
    /// # Errors
    ///
    /// Returns a [`GraphVoiceSpecError`] when the token is not a single word,
    /// the body is empty, a node references itself or a later node, the
    /// output reference is out of range, or a parameter is invalid.
    pub fn new(
        token: impl Into<Box<str>>,
        release_seconds: f32,
        nodes: Vec<VoiceNodeSpec>,
        output: VoiceSignalRef,
    ) -> Result<Self, GraphVoiceSpecError> {
        let token = token.into();
        if token.is_empty() || token.chars().any(char::is_whitespace) {
            return Err(GraphVoiceSpecError::InvalidToken);
        }
        if nodes.is_empty() {
            return Err(GraphVoiceSpecError::EmptyBody);
        }
        if !(release_seconds.is_finite() && release_seconds >= 0.0) {
            return Err(GraphVoiceSpecError::InvalidRelease);
        }

        for (index, node) in nodes.iter().enumerate() {
            if !node.parameters_are_valid() {
                return Err(GraphVoiceSpecError::InvalidParameter { node: index });
            }
            for reference in node.input_refs() {
                if let VoiceSignalRef::Node(target) = reference
                    && target as usize >= index
                {
                    return Err(GraphVoiceSpecError::ForwardReference {
                        node: index,
                        reference: target as usize,
                    });
                }
            }
        }

        if let VoiceSignalRef::Node(target) = output
            && target as usize >= nodes.len()
        {
            return Err(GraphVoiceSpecError::OutputOutOfRange {
                reference: target as usize,
            });
        }

        Ok(Self {
            token,
            release_seconds,
            nodes,
            output,
        })
    }

    /// The pattern-token this program is selected by.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// How long the voice keeps sounding after its gate falls, in seconds.
    #[must_use]
    pub const fn release_seconds(&self) -> f32 {
        self.release_seconds
    }

    /// Compiles the spec into an unprepared [`GraphVoice`].
    ///
    /// Construction may allocate; call it off the audio thread and follow up
    /// with [`GraphVoice::prepare`] before real-time use.
    #[must_use]
    pub fn build_voice(&self, sample_rate_hz: f32) -> GraphVoice {
        GraphVoice {
            processor: self.build_processor(sample_rate_hz),
        }
    }

    /// The release tail length in frames at `sample_rate_hz`.
    fn release_frames(&self, sample_rate_hz: f32) -> u32 {
        release_seconds_to_frames(self.release_seconds, sample_rate_hz)
    }

    /// Lowers the spec DAG onto graph combinators.
    ///
    /// The graph threads a growing signal bus through one stage per node.
    /// Before stage `k` the bus is `[out_{k-1}, .., out_0, gate, freq, gain,
    /// pan]`; the stage wires the node's inputs to the front (a `wire` node
    /// may duplicate bus channels), runs the node in parallel with a
    /// passthrough of the whole bus, and thereby prepends its output. A final
    /// selector feeds `[audio, gain, pan]` through the shared gain and
    /// equal-power pan stages.
    fn build_processor(&self, sample_rate_hz: f32) -> Processor {
        let node_count = self.nodes.len();
        let mut stages = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| Self::build_stage(index, node, sample_rate_hz));
        let first = stages
            .next()
            .unwrap_or_else(|| unreachable!("validated specs have at least one node"));
        let dag = stages.fold(first, |acc, stage| {
            seq(acc, stage)
                .unwrap_or_else(|error| panic!("voice spec stages must compose: {error}"))
        });

        let output_channel = bus_channel(node_count, self.output);
        let gain_channel = channel_index(node_count + 2);
        let pan_channel = channel_index(node_count + 3);
        let select = wire(&[output_channel, gain_channel, pan_channel]);
        // [audio, gain, pan] -> [audio * gain, pan] -> [left, right]
        let levelled = par(gain_node(), passthrough(1));
        let tail = seq(
            select,
            seq(levelled, pan())
                .unwrap_or_else(|error| panic!("voice spec pan stage must compose: {error}")),
        )
        .unwrap_or_else(|error| panic!("voice spec output selector must compose: {error}"));

        let graph = seq(dag, tail)
            .unwrap_or_else(|error| panic!("voice spec output stage must compose: {error}"));
        debug_assert_eq!(graph.inputs(), 4);
        debug_assert_eq!(graph.outputs(), 2);
        Processor::new(graph)
    }

    /// Builds the stage for node `index`: input wiring followed by the node
    /// running in parallel with a passthrough of the whole bus.
    fn build_stage(index: usize, node: &VoiceNodeSpec, sample_rate_hz: f32) -> Seq {
        let bus_width = index + 4;
        let mut mapping: Vec<u32> = node
            .input_refs()
            .iter()
            .map(|reference| bus_channel(index, *reference))
            .collect();
        mapping.extend((0..bus_width).map(channel_index));
        let inputs = wire(&mapping);

        let bus = channel_index(bus_width);
        let staged = match node {
            VoiceNodeSpec::Constant { value } => par(constant(*value), passthrough(bus)),
            VoiceNodeSpec::Sine { .. } => par(sine(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Saw { .. } => par(saw(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Tri { .. } => par(tri(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Pulse { .. } => par(pulse(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Noise { seed } => par(noise(*seed), passthrough(bus)),
            VoiceNodeSpec::Adsr {
                attack_s,
                decay_s,
                sustain,
                release_s,
                ..
            } => {
                let envelope = bind(
                    adsr(sample_rate_hz),
                    &[
                        (1, *attack_s),
                        (2, *decay_s),
                        (3, *sustain),
                        (4, *release_s),
                    ],
                )
                .unwrap_or_else(|error| panic!("adsr bindings are valid by construction: {error}"));
                par(envelope, passthrough(bus))
            }
            VoiceNodeSpec::Ar {
                attack_s,
                release_s,
                ..
            } => {
                let envelope = bind(ar(sample_rate_hz), &[(1, *attack_s), (2, *release_s)])
                    .unwrap_or_else(|error| {
                        panic!("ar bindings are valid by construction: {error}")
                    });
                par(envelope, passthrough(bus))
            }
            VoiceNodeSpec::Lowpass { .. } => par(ladder_filter(sample_rate_hz), passthrough(bus)),
            VoiceNodeSpec::Drive { .. } => par(soft_sat(), passthrough(bus)),
            VoiceNodeSpec::Mul { .. } => par(gain_node(), passthrough(bus)),
            VoiceNodeSpec::Add { .. } => par(sum(2), passthrough(bus)),
        };

        seq(inputs, staged)
            .unwrap_or_else(|error| panic!("voice spec stage wiring must compose: {error}"))
    }
}

/// The bus channel carrying `reference` when `prepended` node outputs sit in
/// front of the fixed `[gate, freq, gain, pan]` tail.
fn bus_channel(prepended: usize, reference: VoiceSignalRef) -> u32 {
    match reference {
        VoiceSignalRef::Gate => channel_index(prepended),
        VoiceSignalRef::Freq => channel_index(prepended + 1),
        VoiceSignalRef::Node(index) => {
            // Outputs are prepended, so node j sits at prepended - 1 - j.
            channel_index(prepended - 1 - index as usize)
        }
    }
}

fn channel_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or_else(|_| panic!("voice spec bus width does not fit in u32"))
}

/// Converts a release tail in seconds to frames, clamped to at least one.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn release_seconds_to_frames(release_seconds: f32, sample_rate_hz: f32) -> u32 {
    let frames = (release_seconds * sample_rate_hz).ceil();
    if frames.is_finite() && frames > 0.0 {
        frames.min(u32::MAX as f32) as u32
    } else {
        1
    }
}

/// A compiled, per-note instance of a graph voice program.
///
/// The engine owns these in a fixed pool. All allocation happens in
/// [`GraphVoiceProgram::build_voice`] and the first block of
/// [`Self::prepare`]; after that, [`Self::process_frame`] and [`Self::reset`]
/// are allocation-free and lock-free.
#[derive(Debug)]
pub struct GraphVoice {
    processor: Processor,
}

impl GraphVoice {
    /// Warms the processor so lazy-but-once scratch buffers are grown off the
    /// audio thread, then resets all state.
    ///
    /// The engine renders one frame at a time, so a single one-frame block is
    /// sufficient to reach every scratch buffer in the graph.
    pub fn prepare(&mut self) {
        let _ = self.process_frame(0.0, 0.0, 0.0, 0.0);
        self.reset();
    }

    /// Renders one stereo frame with the given control values.
    ///
    /// Parameters flow as signals (ADR 0004): the gate, frequency, gain, and
    /// pan are one-frame input channels. This path must stay allocation-free.
    pub fn process_frame(&mut self, gate: f32, freq_hz: f32, gain: f32, pan: f32) -> (f32, f32) {
        let gate_buf = [gate];
        let freq_buf = [freq_hz];
        let gain_buf = [gain];
        let pan_buf = [pan];
        let inputs: [&[f32]; 4] = [&gate_buf, &freq_buf, &gain_buf, &pan_buf];
        let mut left = [0.0_f32];
        let mut right = [0.0_f32];
        {
            let mut outputs: [&mut [f32]; 2] = [&mut left, &mut right];
            self.processor.process(&inputs, &mut outputs, 1);
        }
        (left[0], right[0])
    }

    /// Resets all internal DSP state without releasing buffers.
    pub fn reset(&mut self) {
        self.processor.reset();
    }
}

/// One currently sounding note on a pooled graph voice.
#[derive(Clone, Copy, Debug)]
struct GraphVoiceNote {
    track_id: TrackId,
    gate_frames_remaining: u32,
    release_frames_remaining: u32,
    freq_hz: f32,
    gain: f32,
    pan: f32,
}

#[derive(Debug)]
struct GraphVoiceSlot {
    token: Box<str>,
    release_frames: u32,
    voice: GraphVoice,
    note: Option<GraphVoiceNote>,
}

/// A fixed pool of prepared graph voices owned by the engine core.
///
/// Built off the audio thread — at engine construction or on the language
/// thread before an [`crate::EngineCommand::ReplaceGraphVoicePrograms`] swap;
/// triggering and rendering never allocate.
#[derive(Debug)]
pub struct GraphVoiceBank {
    sample_rate_hz: f32,
    user_specs: Vec<GraphVoiceSpec>,
    slots: Vec<GraphVoiceSlot>,
}

/// Two banks are interchangeable when they were built for the same sample
/// rate from the same user programs (built-ins are constant).
impl PartialEq for GraphVoiceBank {
    fn eq(&self, other: &Self) -> bool {
        self.sample_rate_hz.to_bits() == other.sample_rate_hz.to_bits()
            && self.user_specs == other.user_specs
    }
}

/// Cloning rebuilds and re-prepares the pool from the retained specs. It
/// allocates: never clone a bank on the audio thread.
impl Clone for GraphVoiceBank {
    fn clone(&self) -> Self {
        Self::with_user_programs(self.sample_rate_hz, self.user_specs.clone())
    }
}

impl GraphVoiceBank {
    /// Builds and prepares the pool for every built-in program.
    #[must_use]
    pub fn with_builtin_programs(sample_rate_hz: f32) -> Self {
        Self::with_user_programs(sample_rate_hz, Vec::new())
    }

    /// Builds and prepares pools for the built-in programs plus every user
    /// spec. A user spec whose token collides with a built-in shadows it.
    ///
    /// Construction compiles and warms every pooled voice, so it allocates;
    /// call it off the audio thread and hand the finished bank to the engine
    /// (via [`crate::EngineCommand::ReplaceGraphVoicePrograms`]).
    #[must_use]
    pub fn with_user_programs(sample_rate_hz: f32, user_specs: Vec<GraphVoiceSpec>) -> Self {
        let mut slots = Vec::new();
        for program in builtin_graph_voice_programs() {
            if user_specs
                .iter()
                .any(|spec| spec.token() == program.token())
            {
                continue;
            }
            let release_frames = program.release_frames(sample_rate_hz);
            for _ in 0..GRAPH_VOICE_POLYPHONY {
                let mut voice = program.build_voice(sample_rate_hz);
                voice.prepare();
                slots.push(GraphVoiceSlot {
                    token: program.token().into(),
                    release_frames,
                    voice,
                    note: None,
                });
            }
        }
        for spec in &user_specs {
            let release_frames = spec.release_frames(sample_rate_hz);
            for _ in 0..GRAPH_VOICE_POLYPHONY {
                let mut voice = spec.build_voice(sample_rate_hz);
                voice.prepare();
                slots.push(GraphVoiceSlot {
                    token: spec.token().into(),
                    release_frames,
                    voice,
                    note: None,
                });
            }
        }
        Self {
            sample_rate_hz,
            user_specs,
            slots,
        }
    }

    /// The user-defined program specs this bank was built from.
    #[must_use]
    pub fn user_specs(&self) -> &[GraphVoiceSpec] {
        &self.user_specs
    }

    /// Whether `token` names a pooled graph voice program.
    #[must_use]
    pub fn has_program(&self, token: &str) -> bool {
        self.slots.iter().any(|slot| &*slot.token == token)
    }

    /// Starts a note on an idle pooled voice for `token`.
    ///
    /// Returns `false` (dropping the trigger) when the token names no program
    /// or every pooled voice for it is already sounding. Never allocates.
    pub fn trigger(
        &mut self,
        token: &str,
        track_id: TrackId,
        gate_frames: u32,
        freq_hz: f32,
        gain: f32,
        pan: f32,
    ) -> bool {
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| &*slot.token == token && slot.note.is_none())
        else {
            return false;
        };

        slot.note = Some(GraphVoiceNote {
            track_id,
            gate_frames_remaining: gate_frames.max(1),
            release_frames_remaining: slot.release_frames,
            freq_hz,
            gain,
            pan,
        });
        true
    }

    /// Renders one frame of every sounding voice into the per-track mix.
    ///
    /// Finished voices are reset in place and returned to the pool; nothing
    /// is allocated or dropped on this path.
    pub fn render_frame(&mut self, track_mix: &mut [(f32, f32)]) {
        for slot in &mut self.slots {
            let Some(note) = slot.note.as_mut() else {
                continue;
            };

            let gate = if note.gate_frames_remaining > 0 {
                1.0
            } else {
                0.0
            };
            let (left, right) = slot
                .voice
                .process_frame(gate, note.freq_hz, note.gain, note.pan);

            if let Ok(track_index) = usize::try_from(note.track_id.get())
                && let Some((mix_left, mix_right)) = track_mix.get_mut(track_index)
            {
                *mix_left += left;
                *mix_right += right;
            }

            if note.gate_frames_remaining > 0 {
                note.gate_frames_remaining -= 1;
            } else if note.release_frames_remaining > 0 {
                note.release_frames_remaining -= 1;
            } else {
                slot.voice.reset();
                slot.note = None;
            }
        }
    }

    /// Silences every sounding voice and returns it to the pool.
    pub fn stop_all(&mut self) {
        for slot in &mut self.slots {
            if slot.note.take().is_some() {
                slot.voice.reset();
            }
        }
    }
}

/// Derives the graph voice control values from a scheduled trigger.
///
/// Frequency follows the analog-voice convention: the engine reference
/// frequency scaled by the trigger's playback rate. Gain applies the shared
/// output trim; pan is clamped to the stereo field.
#[allow(clippy::cast_possible_truncation)]
pub fn graph_note_params(trigger: &SampleTrigger, base_hz: f32) -> (f32, f32, f32) {
    let rate = if trigger.rate().is_finite() {
        trigger.rate().abs() as f32
    } else {
        1.0
    };
    let freq_hz = (base_hz * rate).max(0.0);

    let gain = if trigger.gain().is_finite() {
        (trigger.gain() as f32).max(0.0)
    } else {
        1.0
    } * GRAPH_OUTPUT_TRIM;

    let pan = if trigger.pan().is_finite() {
        (trigger.pan() as f32).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    (freq_hz, gain, pan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn track(id: u32) -> TrackId {
        TrackId::new(id)
    }

    #[test]
    fn builtin_programs_expose_gsine() {
        let programs = builtin_graph_voice_programs();
        assert!(programs.iter().any(|program| program.token() == "gsine"));
    }

    #[test]
    fn gsine_voice_produces_audio_while_gated_and_decays_after_release() {
        let program = &builtin_graph_voice_programs()[0];
        let mut voice = program.build_voice(SR);
        voice.prepare();

        let mut gated_energy = 0.0_f32;
        for _ in 0..2_400 {
            let (left, right) = voice.process_frame(1.0, 220.0, 0.5, 0.0);
            gated_energy += left.abs() + right.abs();
        }
        assert!(gated_energy > 1.0, "gated voice should be audible");

        // Run the release out (0.02 s = 960 frames), then check silence.
        for _ in 0..2_400 {
            let _ = voice.process_frame(0.0, 220.0, 0.5, 0.0);
        }
        let (left, right) = voice.process_frame(0.0, 220.0, 0.5, 0.0);
        assert!(left.abs() < 1e-4 && right.abs() < 1e-4);
    }

    #[test]
    fn bank_trigger_respects_polyphony_and_unknown_tokens() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.has_program("gsine"));
        assert!(!bank.has_program("bd"));
        assert!(!bank.trigger("bd", track(0), 10, 220.0, 0.5, 0.0));

        for _ in 0..GRAPH_VOICE_POLYPHONY {
            assert!(bank.trigger("gsine", track(0), 10, 220.0, 0.5, 0.0));
        }
        assert!(
            !bank.trigger("gsine", track(0), 10, 220.0, 0.5, 0.0),
            "the pool is exhausted, the trigger must be dropped"
        );
    }

    #[test]
    fn bank_returns_finished_voices_to_the_pool() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.trigger("gsine", track(0), 2, 220.0, 0.5, 0.0));

        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        // 2 gate frames + release frames + the final reclaim frame.
        let program = &builtin_graph_voice_programs()[0];
        let lifetime = 2 + program.release_frames(SR) + 1;
        for _ in 0..lifetime {
            bank.render_frame(&mut mix);
        }

        for _ in 0..GRAPH_VOICE_POLYPHONY {
            assert!(bank.trigger("gsine", track(0), 2, 220.0, 0.5, 0.0));
        }
    }

    #[test]
    fn render_frame_ignores_out_of_range_tracks() {
        let mut bank = GraphVoiceBank::with_builtin_programs(SR);
        assert!(bank.trigger("gsine", track(7), 4, 220.0, 0.5, 0.0));
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        bank.render_frame(&mut mix);
        assert_eq!(mix[0], (0.0, 0.0));
    }

    #[test]
    fn graph_note_params_follow_trigger_fields() {
        let trigger = SampleTrigger::named("gsine")
            .with_rate(2.0)
            .with_gain(1.0)
            .with_pan(-0.5);
        let (freq_hz, gain, pan) = graph_note_params(&trigger, 220.0);
        assert!((freq_hz - 440.0).abs() < 1e-3);
        assert!((gain - GRAPH_OUTPUT_TRIM).abs() < 1e-6);
        assert!((pan - -0.5).abs() < 1e-6);
    }
}
