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
pub enum PreampModel {
    JfetClean,
    OpampTight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipModel {
    SiliconHard,
    GermaniumSoft,
    RedLed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToneModel {
    Neutral,
    MidHump,
    ScoopedStack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterMode {
    LowPass,
    HighPass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalStage {
    Buffer {
        input: NodeRef,
    },
    Preamp {
        input: NodeRef,
        gain: NodeRef,
        model: PreampModel,
    },
    Gain {
        input: NodeRef,
        amount: NodeRef,
    },
    Clip {
        input: NodeRef,
        drive: NodeRef,
        model: ClipModel,
    },
    Tone {
        input: NodeRef,
        cutoff_hz: NodeRef,
        resonance: NodeRef,
        model: ToneModel,
    },
    Filter {
        input: NodeRef,
        kind: FilterMode,
        cutoff_hz: NodeRef,
        resonance: NodeRef,
    },
    Eq {
        input: NodeRef,
        low: NodeRef,
        mid: NodeRef,
        high: NodeRef,
    },
    Level {
        input: NodeRef,
        amount: NodeRef,
    },
    Sag {
        input: NodeRef,
        amount: NodeRef,
    },
    Bias {
        input: NodeRef,
        amount: NodeRef,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalNodeKind {
    Constant {
        value_bits: u32,
    },
    Lfo {
        rate_hz_bits: u32,
        depth_bits: u32,
        offset_bits: u32,
    },
    EnvFollow {
        input: NodeRef,
        attack_ms_bits: u32,
        release_ms_bits: u32,
    },
    Add {
        left: NodeRef,
        right: NodeRef,
    },
    Mul {
        left: NodeRef,
        right: NodeRef,
    },
    Stage(PedalStage),
    Mix {
        inputs: Vec<NodeRef>,
    },
    Feedback {
        input: NodeRef,
        amount: NodeRef,
        delay_samples: usize,
        tone_hz_bits: Option<u32>,
    },
}

impl PedalNodeKind {
    #[must_use]
    pub const fn constant_value(&self) -> Option<f32> {
        match self {
            Self::Constant { value_bits } => Some(f32::from_bits(*value_bits)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalNode {
    signal_kind: SignalKind,
    kind: PedalNodeKind,
}

impl PedalNode {
    #[must_use]
    pub const fn new(signal_kind: SignalKind, kind: PedalNodeKind) -> Self {
        Self { signal_kind, kind }
    }

    #[must_use]
    pub const fn constant(value: f32) -> Self {
        Self::new(
            SignalKind::Control,
            PedalNodeKind::Constant {
                value_bits: value.to_bits(),
            },
        )
    }

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

    #[must_use]
    pub const fn stage(stage: PedalStage) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Stage(stage))
    }

    #[must_use]
    pub const fn add(signal_kind: SignalKind, left: NodeRef, right: NodeRef) -> Self {
        Self::new(signal_kind, PedalNodeKind::Add { left, right })
    }

    #[must_use]
    pub const fn mul(left: NodeRef, right: NodeRef) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mul { left, right })
    }

    #[must_use]
    pub const fn mix(inputs: Vec<NodeRef>) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mix { inputs })
    }

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

    #[must_use]
    pub const fn signal_kind(&self) -> SignalKind {
        self.signal_kind
    }

    #[must_use]
    pub const fn kind(&self) -> &PedalNodeKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PedalGraphProgram {
    nodes: Vec<PedalNode>,
    output: NodeRef,
}

impl PedalGraphProgram {
    #[must_use]
    pub const fn new(nodes: Vec<PedalNode>, output: NodeRef) -> Self {
        Self { nodes, output }
    }

    #[must_use]
    pub fn nodes(&self) -> &[PedalNode] {
        &self.nodes
    }

    #[must_use]
    pub const fn output(&self) -> NodeRef {
        self.output
    }

    #[allow(clippy::incompatible_msrv)]
    #[must_use]
    #[allow(clippy::incompatible_msrv)]
    pub const fn is_bypass(&self) -> bool {
        self.nodes.is_empty() && matches!(self.output, NodeRef::Input)
    }
}
