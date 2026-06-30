//! Declarative program definitions for virtual analog pedals.
//!
//! A pedal program is an immutable graph of [`PedalNode`]s. It represents
//! the static topology of an effect, which is evaluated dynamically by a
//! `PedalInstance` at runtime.
//!
//! Programs consist of:
//! - Constants and Control signals (LFOs, Envelopes).
//! - Math operations (Add, Multiply) to mix or scale signals.
//! - Virtual analog DSP stages ([`PedalStage`]) like distortion clipping, filtering, and preamp gain.
//!
//! # Examples
//!
//! Creating a simple clean boost pedal that multiplies the input by a constant factor:
//!
//! ```rust
//! use orpheus_dsp::{PedalNode, PedalGraphProgram, NodeRef, SignalKind};
//!
//! let gain_node = PedalNode::constant(2.0);
//! let multiply_node = PedalNode::mul(NodeRef::Input, NodeRef::node(0));
//!
//! let program = PedalGraphProgram::new(vec![gain_node, multiply_node], NodeRef::node(1));
//! ```

#![allow(
    clippy::suboptimal_flops,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_map,
    clippy::redundant_closure_for_method_calls,
    clippy::default_constructed_unit_structs
)]
/// Represents the rate at which a signal is evaluated within a pedal graph.
///
/// Nodes producing or consuming `Audio` rate signals are evaluated every single
/// sample frame. Nodes producing or consuming `Control` rate signals (such as
/// LFOs or Envelopes) are evaluated at a lower sub-sampled rate to save CPU.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalKind {
    /// Audio-rate signals evaluated at every frame.
    Audio,
    /// Control-rate signals evaluated at block boundaries.
    Control,
}

/// A reference to the output of another node in the pedal graph.
///
/// Because the graph is represented as a flat `Vec` of `PedalNode`s, a `NodeRef`
/// is essentially an index into that list, with a special case for the overall
/// pedal's audio input.
///
/// Nodes can only reference previous nodes in the list (i.e. `index < current_node_index`)
/// to prevent cycles. The only exception is the [`PedalNodeKind::Feedback`] node,
/// which delays a forward reference by a buffer length.
///
/// # Examples
///
/// ```rust
/// use orpheus_dsp::NodeRef;
///
/// // Refers to the incoming dry signal
/// let input_ref = NodeRef::Input;
///
/// // Refers to the output of the 0th node in the program's node list
/// let node0_ref = NodeRef::node(0);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NodeRef {
    /// Refers to the mono audio input passed into the overall pedal.
    #[default]
    Input,
    /// Refers to the output value of the node at the specified index in the program.
    Node(usize),
}

impl NodeRef {
    /// Convenience function for creating a `NodeRef::Node(index)`.
    #[must_use]
    pub const fn node(index: usize) -> Self {
        Self::Node(index)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The physical amplifier model simulation characteristic.
pub enum PreampModel {
    /// A transparent, high-headroom Field Effect Transistor (FET) stage.
    JfetClean,
    /// A stiff, focused operational amplifier stage with aggressive bite.
    OpampTight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The diode clipping network used for distortion generation.
pub enum ClipModel {
    /// Hard, bright, and aggressive clipping characteristic of silicon diodes.
    SiliconHard,
    /// Warm, asymmetrical, and responsive clipping characteristic of germanium diodes.
    GermaniumSoft,
    /// Open, loud, and crunchy clipping characteristic of Light Emitting Diodes (LEDs).
    RedLed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The equalization topology curve.
pub enum ToneModel {
    /// A flat, transparent EQ response.
    Neutral,
    /// An equalization curve that boosts midrange frequencies, typical of classic overdrives.
    MidHump,
    /// An equalization curve that cuts midrange and boosts lows/highs, typical of high-gain amplifiers.
    ScoopedStack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The frequency passband behavior.
pub enum FilterMode {
    /// Attenuates frequencies above the cutoff.
    LowPass,
    /// Attenuates frequencies below the cutoff.
    HighPass,
}

/// A discrete signal processing module inside a pedal graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalStage {
    /// A transparent buffer that passes the signal through.
    Buffer {
        /// The signal to buffer.
        input: NodeRef,
    },
    /// An input pre-amplifier stage.
    Preamp {
        /// The audio signal to amplify.
        input: NodeRef,
        /// The control-rate gain scalar.
        gain: NodeRef,
        /// The analog characteristic model to use.
        model: PreampModel,
    },
    /// A linear gain multiplier.
    Gain {
        /// The audio signal to amplify.
        input: NodeRef,
        /// The control-rate gain scalar.
        amount: NodeRef,
    },
    /// Non-linear diode clipping for distortion.
    Clip {
        /// The audio signal to clip.
        input: NodeRef,
        /// The control-rate drive scalar.
        drive: NodeRef,
        /// The analog characteristic model to use.
        model: ClipModel,
    },
    /// A specialized equalization filter.
    Tone {
        /// The audio signal to equalize.
        input: NodeRef,
        /// The control-rate center frequency in Hertz.
        cutoff_hz: NodeRef,
        /// The control-rate resonance or Q factor.
        resonance: NodeRef,
        /// The topological EQ model to use.
        model: ToneModel,
    },
    /// A general-purpose filter.
    Filter {
        /// The audio signal to filter.
        input: NodeRef,
        /// The passband mode (`LowPass` or `HighPass`).
        kind: FilterMode,
        /// The control-rate cutoff frequency in Hertz.
        cutoff_hz: NodeRef,
        /// The control-rate resonance or Q factor.
        resonance: NodeRef,
    },
    /// A 3-band parametric equalizer.
    Eq {
        /// The audio signal to equalize.
        input: NodeRef,
        /// The control-rate low-shelf gain scalar.
        low: NodeRef,
        /// The control-rate mid-band gain scalar.
        mid: NodeRef,
        /// The control-rate high-shelf gain scalar.
        high: NodeRef,
    },
    /// An output volume control.
    Level {
        /// The audio signal to scale.
        input: NodeRef,
        /// The control-rate volume scalar.
        amount: NodeRef,
    },
    /// Voltage sag simulation under load.
    Sag {
        /// The audio signal to dynamically compress.
        input: NodeRef,
        /// The control-rate sensitivity.
        amount: NodeRef,
    },
    /// DC offset injection.
    Bias {
        /// The audio signal to offset.
        input: NodeRef,
        /// The control-rate DC offset amount.
        amount: NodeRef,
    },
}

/// The underlying operational logic of a `PedalNode`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalNodeKind {
    /// A static floating-point value.
    Constant {
        /// The raw IEEE 754 bits of the `f32` constant.
        value_bits: u32,
    },
    /// A Low Frequency Oscillator.
    Lfo {
        /// The raw IEEE 754 bits of the `f32` rate in Hertz.
        rate_hz_bits: u32,
        /// The raw IEEE 754 bits of the `f32` amplitude depth.
        depth_bits: u32,
        /// The raw IEEE 754 bits of the `f32` DC offset.
        offset_bits: u32,
    },
    /// An Envelope Follower.
    EnvFollow {
        /// The audio signal to track.
        input: NodeRef,
        /// The raw IEEE 754 bits of the `f32` attack time in milliseconds.
        attack_ms_bits: u32,
        /// The raw IEEE 754 bits of the `f32` release time in milliseconds.
        release_ms_bits: u32,
    },
    /// Sums two signals.
    Add {
        /// The left operand.
        left: NodeRef,
        /// The right operand.
        right: NodeRef,
    },
    /// Multiplies two signals.
    Mul {
        /// The left operand.
        left: NodeRef,
        /// The right operand.
        right: NodeRef,
    },
    /// Evaluates a DSP stage block.
    Stage(PedalStage),
    /// Sums multiple inputs.
    Mix {
        /// The list of input signals to sum.
        inputs: Vec<NodeRef>,
    },
    /// Delay line with feedback.
    Feedback {
        /// The audio signal to delay.
        input: NodeRef,
        /// The control-rate feedback multiplier.
        amount: NodeRef,
        /// The static delay time in sample frames.
        delay_samples: usize,
        /// The raw IEEE 754 bits of the `f32` low-pass filter cutoff frequency for the feedback loop, if any.
        tone_hz_bits: Option<u32>,
    },
}

impl PedalNodeKind {
    #[doc(hidden)]
    #[must_use]
    pub const fn constant_value(&self) -> Option<f32> {
        match self {
            Self::Constant { value_bits } => Some(f32::from_bits(*value_bits)),
            _ => None,
        }
    }
}

/// A discrete node inside a `PedalGraphProgram`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalNode {
    signal_kind: SignalKind,
    kind: PedalNodeKind,
}

impl PedalNode {
    /// Construct a general purpose node.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_dsp::{PedalNode, SignalKind, PedalNodeKind};
    ///
    /// let constant_node = PedalNode::new(
    ///     SignalKind::Control,
    ///     PedalNodeKind::Constant { value_bits: 2.0_f32.to_bits() }
    /// );
    /// ```
    #[must_use]
    pub const fn new(signal_kind: SignalKind, kind: PedalNodeKind) -> Self {
        Self { signal_kind, kind }
    }

    /// Construct a control-rate constant value.
    #[must_use]
    pub const fn constant(value: f32) -> Self {
        Self::new(
            SignalKind::Control,
            PedalNodeKind::Constant {
                value_bits: value.to_bits(),
            },
        )
    }

    /// Construct a low-frequency oscillator.
    #[must_use]
    pub const fn lfo(rate_hz: f32, depth: f32, offset: f32) -> Self {
        Self::new(
            SignalKind::Control,
            PedalNodeKind::Lfo {
                rate_hz_bits: rate_hz.to_bits(),
                depth_bits: depth.to_bits(),
                offset_bits: offset.to_bits(),
            },
        )
    }

    /// Construct an envelope tracker.
    #[must_use]
    pub const fn env_follow(input: NodeRef, attack_ms: f32, release_ms: f32) -> Self {
        Self::new(
            SignalKind::Control,
            PedalNodeKind::EnvFollow {
                input,
                attack_ms_bits: attack_ms.to_bits(),
                release_ms_bits: release_ms.to_bits(),
            },
        )
    }

    /// Construct an audio processing stage.
    #[must_use]
    pub const fn stage(stage: PedalStage) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Stage(stage))
    }

    /// Construct a signal addition node.
    #[must_use]
    pub const fn add(signal_kind: SignalKind, left: NodeRef, right: NodeRef) -> Self {
        Self::new(signal_kind, PedalNodeKind::Add { left, right })
    }

    /// Construct a signal multiplication node.
    #[must_use]
    pub const fn mul(left: NodeRef, right: NodeRef) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mul { left, right })
    }

    /// Construct an audio-rate mixer merging multiple inputs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_dsp::{PedalNode, NodeRef};
    ///
    /// let mixer = PedalNode::mix(vec![NodeRef::node(0), NodeRef::node(1), NodeRef::node(2)]);
    /// ```
    #[must_use]
    pub const fn mix(inputs: Vec<NodeRef>) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mix { inputs })
    }

    /// Construct a feedback delay.
    #[must_use]
    pub fn feedback(
        input: NodeRef,
        amount: NodeRef,
        delay_samples: usize,
        tone_hz: Option<f32>,
    ) -> Self {
        Self::new(
            SignalKind::Audio,
            PedalNodeKind::Feedback {
                input,
                amount,
                delay_samples,
                tone_hz_bits: tone_hz.map(f32::to_bits),
            },
        )
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn signal_kind(&self) -> SignalKind {
        self.signal_kind
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn kind(&self) -> &PedalNodeKind {
        &self.kind
    }
}

/// The immutable compiled program for a pedal effect.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PedalGraphProgram {
    nodes: Vec<PedalNode>,
    output: NodeRef,
}

impl PedalGraphProgram {
    /// Create a pedal program.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_dsp::{PedalGraphProgram, PedalNode, NodeRef};
    ///
    /// let nodes = vec![PedalNode::constant(2.0), PedalNode::mul(NodeRef::Input, NodeRef::node(0))];
    /// let program = PedalGraphProgram::new(nodes, NodeRef::node(1));
    /// ```
    #[must_use]
    pub const fn new(nodes: Vec<PedalNode>, output: NodeRef) -> Self {
        Self { nodes, output }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn nodes(&self) -> &[PedalNode] {
        &self.nodes
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn output(&self) -> NodeRef {
        self.output
    }

    #[doc(hidden)]
    #[allow(clippy::incompatible_msrv)]
    #[must_use]
    #[allow(clippy::incompatible_msrv)]
    pub const fn is_bypass(&self) -> bool {
        self.nodes.is_empty() && matches!(self.output, NodeRef::Input)
    }
}

/// Immutable pedal metadata attached to triggers off the audio thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalProgram {
    source: Box<str>,
    explain: Box<str>,
    graph: PedalGraphProgram,
}

impl PedalProgram {
    /// Create a new `PedalProgram` linking the parsed source with its validation explanation.
    #[must_use]
    pub fn new(source: impl Into<Box<str>>, explain: impl Into<Box<str>>) -> Self {
        Self {
            source: source.into(),
            explain: explain.into(),
            graph: PedalGraphProgram::new(Vec::new(), NodeRef::Input),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[doc(hidden)]
    #[must_use]
    pub fn explain(&self) -> &str {
        &self.explain
    }

    /// Inject a constructed [`PedalGraphProgram`] representing the effect's internal routing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orpheus_dsp::{PedalProgram, PedalNode, NodeRef};
    ///
    /// let program = PedalProgram::new("input |> output", "bypass")
    ///     .with_graph(vec![], NodeRef::Input);
    /// ```
    #[must_use]
    pub fn with_graph(mut self, nodes: Vec<crate::pedal::PedalNode>, output: NodeRef) -> Self {
        self.graph = PedalGraphProgram::new(nodes, output);
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn graph(&self) -> &PedalGraphProgram {
        &self.graph
    }
}
