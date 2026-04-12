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

/// Virtual analog models for the preamp stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreampModel {
    /// A clean, high-headroom JFET preamp model.
    JfetClean,
    /// A tight, aggressive op-amp preamp model.
    OpampTight,
}

/// Clipping models for distortion and overdrive stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipModel {
    /// Hard clipping using silicon diodes.
    SiliconHard,
    /// Soft clipping using germanium diodes.
    GermaniumSoft,
    /// Asymmetrical clipping using red LEDs.
    RedLed,
}

/// EQ shapes for the tone stack stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToneModel {
    /// A flat, neutral tone response.
    Neutral,
    /// A mid-frequency boost, classic for overdrives.
    MidHump,
    /// A scooped mid-frequency response, classic for high gain.
    ScoopedStack,
}

/// Operating modes for the state-variable filter stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterMode {
    /// A low-pass filter mode.
    LowPass,
    /// A high-pass filter mode.
    HighPass,
}

/// Represents a complex virtual analog DSP processing stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalStage {
    /// A unity-gain buffer that prevents signal loading.
    Buffer {
        /// The signal to buffer.
        input: NodeRef,
    },
    /// A preamp stage adding gain and coloration.
    Preamp {
        /// The input signal.
        input: NodeRef,
        /// The gain amount control.
        gain: NodeRef,
        /// The analog model to simulate.
        model: PreampModel,
    },
    /// A simple linear gain multiplier stage.
    Gain {
        /// The input signal.
        input: NodeRef,
        /// The gain multiplier amount.
        amount: NodeRef,
    },
    /// A non-linear clipping distortion stage.
    Clip {
        /// The input signal.
        input: NodeRef,
        /// The drive amount control.
        drive: NodeRef,
        /// The diode clipping model.
        model: ClipModel,
    },
    /// A tone stack EQ stage.
    Tone {
        /// The input signal.
        input: NodeRef,
        /// The cutoff frequency in Hertz.
        cutoff_hz: NodeRef,
        /// The resonance or Q-factor.
        resonance: NodeRef,
        /// The tone stack model.
        model: ToneModel,
    },
    /// A resonant state-variable filter stage.
    Filter {
        /// The input signal.
        input: NodeRef,
        /// The filter operating mode.
        kind: FilterMode,
        /// The cutoff frequency in Hertz.
        cutoff_hz: NodeRef,
        /// The resonance amount.
        resonance: NodeRef,
    },
    /// A 3-band parametric EQ stage.
    Eq {
        /// The input signal.
        input: NodeRef,
        /// The low band gain control.
        low: NodeRef,
        /// The mid band gain control.
        mid: NodeRef,
        /// The high band gain control.
        high: NodeRef,
    },
    /// A final output level control stage.
    Level {
        /// The input signal.
        input: NodeRef,
        /// The output level multiplier.
        amount: NodeRef,
    },
    /// A power supply sag simulation stage.
    Sag {
        /// The input signal.
        input: NodeRef,
        /// The amount of voltage sag to simulate.
        amount: NodeRef,
    },
    /// A DC bias offset stage.
    Bias {
        /// The input signal.
        input: NodeRef,
        /// The DC bias offset amount.
        amount: NodeRef,
    },
}

/// The operation performed by a node in the pedal graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalNodeKind {
    /// A constant numeric value, usually for a control parameter.
    Constant {
        /// The raw bit representation of the f32 value.
        value_bits: u32,
    },
    /// A low-frequency oscillator.
    Lfo {
        /// The rate in Hertz as raw bits.
        rate_hz_bits: u32,
        /// The modulation depth as raw bits.
        depth_bits: u32,
        /// The DC offset as raw bits.
        offset_bits: u32,
    },
    /// An envelope follower that extracts the amplitude contour of a signal.
    EnvFollow {
        /// The audio signal to track.
        input: NodeRef,
        /// The attack time in milliseconds as raw bits.
        attack_ms_bits: u32,
        /// The release time in milliseconds as raw bits.
        release_ms_bits: u32,
    },
    /// Adds two signals together.
    Add {
        /// The left operand.
        left: NodeRef,
        /// The right operand.
        right: NodeRef,
    },
    /// Multiplies two signals together (ring modulation or VCA).
    Mul {
        /// The left operand.
        left: NodeRef,
        /// The right operand.
        right: NodeRef,
    },
    /// A complex analog DSP stage.
    Stage(PedalStage),
    /// Mixes multiple signals together evenly.
    Mix {
        /// The list of input signals to mix.
        inputs: Vec<NodeRef>,
    },
    /// A feedback path that delays a signal by a certain number of samples.
    Feedback {
        /// The signal to feedback.
        input: NodeRef,
        /// The feedback multiplier amount.
        amount: NodeRef,
        /// The delay time in samples.
        delay_samples: usize,
        /// An optional low-pass filter cutoff in Hertz (as raw bits) in the feedback path.
        tone_hz_bits: Option<u32>,
    },
}

impl PedalNodeKind {
    /// Returns the constant float value if this node is a `Constant`.
    #[must_use]
    pub const fn constant_value(&self) -> Option<f32> {
        match self {
            Self::Constant { value_bits } => Some(f32::from_bits(*value_bits)),
            _ => None,
        }
    }
}

/// A single operation or stage within a pedal graph program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalNode {
    signal_kind: SignalKind,
    kind: PedalNodeKind,
}

impl PedalNode {
    /// Creates a new pedal node with the given signal kind and operation.
    #[must_use]
    pub const fn new(signal_kind: SignalKind, kind: PedalNodeKind) -> Self {
        Self { signal_kind, kind }
    }

    /// Creates a constant value node operating at control rate.
    #[must_use]
    pub const fn constant(value: f32) -> Self {
        Self::new(
            SignalKind::Control,
            PedalNodeKind::Constant {
                value_bits: value.to_bits(),
            },
        )
    }

    /// Creates an LFO node operating at control rate.
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

    /// Creates an envelope follower node operating at control rate.
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

    /// Creates an analog stage node operating at audio rate.
    #[must_use]
    pub const fn stage(stage: PedalStage) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Stage(stage))
    }

    /// Creates an addition node mixing two signals.
    #[must_use]
    pub const fn add(signal_kind: SignalKind, left: NodeRef, right: NodeRef) -> Self {
        Self::new(signal_kind, PedalNodeKind::Add { left, right })
    }

    /// Creates a multiplication node scaling two signals at audio rate.
    #[must_use]
    pub const fn mul(left: NodeRef, right: NodeRef) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mul { left, right })
    }

    /// Creates a mixing node summing multiple signals at audio rate.
    #[must_use]
    pub const fn mix(inputs: Vec<NodeRef>) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mix { inputs })
    }

    /// Creates a feedback node that delays a signal.
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

    /// Returns the signal rate (Audio or Control) this node operates at.
    #[must_use]
    pub const fn signal_kind(&self) -> SignalKind {
        self.signal_kind
    }

    /// Returns the inner operation kind of this node.
    #[must_use]
    pub const fn kind(&self) -> &PedalNodeKind {
        &self.kind
    }
}

/// A complete, immutable program defining a virtual analog pedal effect.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PedalGraphProgram {
    nodes: Vec<PedalNode>,
    output: NodeRef,
}

impl PedalGraphProgram {
    /// Creates a new pedal graph program from a list of nodes and an output reference.
    #[must_use]
    pub const fn new(nodes: Vec<PedalNode>, output: NodeRef) -> Self {
        Self { nodes, output }
    }

    /// Returns the topologically sorted list of nodes in the program.
    #[must_use]
    pub fn nodes(&self) -> &[PedalNode] {
        &self.nodes
    }

    /// Returns the reference to the node that produces the final audio output.
    #[must_use]
    pub const fn output(&self) -> NodeRef {
        self.output
    }

    /// Returns `true` if this program simply passes the input directly to the output.
    #[allow(clippy::incompatible_msrv)]
    #[must_use]
    #[allow(clippy::incompatible_msrv)]
    pub const fn is_bypass(&self) -> bool {
        self.nodes.is_empty() && matches!(self.output, NodeRef::Input)
    }
}
