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

/// Analog-modeled preamp circuits simulating the first gain stage of an amplifier.
///
/// Orpheus provides these static models to add harmonic saturation and drive to a signal
/// before hitting heavier clipping diodes or tonestacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreampModel {
    /// A clean JFET-style preamp with soft saturation and low odd-order harmonics.
    JfetClean,
    /// A tight, op-amp based high gain preamp, responding sharply to transients.
    OpampTight,
}

/// Analog clipping diode models simulating the primary distortion characteristics.
///
/// These mathematically approximate the physical threshold and curvature of various diodes
/// used in classic distortion, fuzz, and overdrive pedals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipModel {
    /// Hard-clipping silicon diodes (like a Boss DS-1), providing a harsh, compressed square wave.
    SiliconHard,
    /// Soft-clipping germanium diodes, offering a warmer, fuzzier tone.
    GermaniumSoft,
    /// High-headroom LED clipping (like a `ProCo` Rat or Turbo RAT mod), offering crunchy dynamics.
    RedLed,
}

/// Passive tonestack models representing fixed EQ curves.
///
/// Instead of a generic digital low-pass filter, these models mimic the specific R-C network
/// topologies of famous guitar pedal circuits, profoundly shaping the spectral balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToneModel {
    /// Flat, neutral frequency response bypassing any coloration.
    Neutral,
    /// Classic mid-hump response pushing 700-800Hz forward to cut through a mix.
    MidHump,
    /// A Big Muff-style mid-scooped response emphasizing extreme highs and booming lows.
    ScoopedStack,
}

/// Resonant filter topologies used to dynamically sculpt frequencies via cutoff modulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterMode {
    /// Allows low frequencies to pass while attenuating high frequencies above the cutoff.
    LowPass,
    /// Allows high frequencies to pass while attenuating low frequencies below the cutoff.
    HighPass,
}

/// A granular digital signal processing unit simulating a single stage in an analog circuit.
///
/// To optimize for real-time constraints on the audio thread, Orpheus doesn't evaluate a full
/// AST tree directly. Instead, `PedalStage` definitions represent a static, flat, and highly
/// optimized sequence of buffers, non-linear saturations, and filters.
///
/// These stages are meant to be assembled sequentially inside a `PedalNodeKind::Stage`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalStage {
    /// An isolation buffer passing a signal cleanly without coloration or loading effects.
    Buffer {
        /// The upstream DSP node providing the source signal.
        input: NodeRef,
    },
    /// An amplifier input stage designed to add warmth and subtle harmonics.
    Preamp {
        /// The upstream audio source.
        input: NodeRef,
        /// A control-rate node dictating the input sensitivity.
        gain: NodeRef,
        /// The specific circuit topology emulation (`JFET`, `OpAmp`, etc).
        model: PreampModel,
    },
    /// A pristine digital multiplier increasing or decreasing amplitude without distortion.
    Gain {
        /// The audio source to be scaled.
        input: NodeRef,
        /// A control-rate node dictating the linear multiplication factor.
        amount: NodeRef,
    },
    /// A harsh, non-linear waveform shaper driven past its voltage rails.
    Clip {
        /// The audio source to push into the diodes.
        input: NodeRef,
        /// A control node defining the intensity of the saturation.
        drive: NodeRef,
        /// The specific diode configuration (Silicon, Germanium, LED).
        model: ClipModel,
    },
    /// A fixed, passive EQ network mimicking iconic analog tonestacks.
    Tone {
        /// The audio source.
        input: NodeRef,
        /// A control node modulating the center frequency of the tonestack.
        cutoff_hz: NodeRef,
        /// A control node for the width or Q factor of the band.
        resonance: NodeRef,
        /// The circuit topology (e.g., scooped Muff vs. humped TS).
        model: ToneModel,
    },
    /// A variable-state resonant filter actively shaping the spectral content.
    Filter {
        /// The audio source.
        input: NodeRef,
        /// The topology of the filter (`LowPass`, `HighPass`).
        kind: FilterMode,
        /// A control node modulating the corner frequency.
        cutoff_hz: NodeRef,
        /// A control node defining the peak resonance near the cutoff point.
        resonance: NodeRef,
    },
    /// An active 3-band parametric equalizer.
    Eq {
        /// The audio source.
        input: NodeRef,
        /// A control node for low-shelf gain.
        low: NodeRef,
        /// A control node for mid-band bell gain.
        mid: NodeRef,
        /// A control node for high-shelf gain.
        high: NodeRef,
    },
    /// The final master volume attenuator before the pedal outputs.
    Level {
        /// The fully processed audio source.
        input: NodeRef,
        /// A control node for the final output scalar.
        amount: NodeRef,
    },
    /// A simulation of power supply voltage drops during heavy transient loads.
    Sag {
        /// The audio source.
        input: NodeRef,
        /// A control node dictating the severity of the voltage drop envelope.
        amount: NodeRef,
    },
    /// A constant DC voltage offset applied directly to the audio wave.
    Bias {
        /// The audio source.
        input: NodeRef,
        /// A control node dictating the positive or negative offset shift.
        amount: NodeRef,
    },
}

/// Defines the fundamental primitive operations permissible in a pedal evaluation sequence.
///
/// Because a `PedalGraphProgram` is a flat vector of evaluation steps executed every sample,
/// `PedalNodeKind` provides the atomic instructions that `PedalInstance` iterates over.
/// They map 1:1 with internal DSP state components (like phase accumulators for LFOs,
/// or ring buffers for Feedback delays).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PedalNodeKind {
    /// A static floating-point value injected into the graph. Used to fix parameters.
    Constant {
        /// The raw IEEE 754 float bits representing the constant, bypassing `f32::Eq` issues.
        value_bits: u32,
    },
    /// A Low Frequency Oscillator generating periodic control curves.
    Lfo {
        /// The cycle rate of the LFO in Hertz.
        rate_hz_bits: u32,
        /// The amplitude depth from peak-to-peak.
        depth_bits: u32,
        /// The DC offset centering the waveform.
        offset_bits: u32,
    },
    /// An envelope follower smoothing the rectified amplitude of an incoming audio signal.
    EnvFollow {
        /// The upstream audio source being tracked.
        input: NodeRef,
        /// How fast the follower reacts to a sudden volume increase (ms).
        attack_ms_bits: u32,
        /// How slow the follower decays back to zero after a transient (ms).
        release_ms_bits: u32,
    },
    /// A summation operation combining two signals.
    Add {
        /// The left-hand operand.
        left: NodeRef,
        /// The right-hand operand.
        right: NodeRef,
    },
    /// A multiplication operation scaling one signal by another.
    Mul {
        /// The left-hand operand.
        left: NodeRef,
        /// The right-hand operand.
        right: NodeRef,
    },
    /// Encapsulates a complex digital simulation of a physical circuit stage.
    Stage(PedalStage),
    /// A summing mixer averaging multiple input lines into a single channel.
    Mix {
        /// The collection of reference pointers pointing to upstream signals.
        inputs: Vec<NodeRef>,
    },
    /// A delay feedback loop creating echoes or comb filtering.
    Feedback {
        /// The source audio injected into the delay line.
        input: NodeRef,
        /// A control node determining how much delayed signal is fed back (decay).
        amount: NodeRef,
        /// The raw buffer length of the delay line.
        delay_samples: usize,
        /// An optional low-pass filter cutoff applied inside the feedback loop to simulate bucket-brigade degradation.
        tone_hz_bits: Option<u32>,
    },
}

impl PedalNodeKind {
    /// Attempts to extract the floating point scalar if this node is statically a `Constant`.
    ///
    /// The runtime DSP graph uses this to aggressively pre-calculate limits on tail frames
    /// or optimize out math operations if one of the operands never changes.
    #[must_use]
    pub const fn constant_value(&self) -> Option<f32> {
        match self {
            Self::Constant { value_bits } => Some(f32::from_bits(*value_bits)),
            _ => None,
        }
    }
}

/// A fully defined graph operation carrying both its logic and domain metadata.
///
/// `PedalNode` pairs a DSP primitive (`PedalNodeKind`) with a `SignalKind` guaranteeing
/// whether the output updates once per audio sample or only at block boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalNode {
    signal_kind: SignalKind,
    kind: PedalNodeKind,
}

impl PedalNode {
    /// Binds an execution rate domain to a primitive instruction step.
    #[must_use]
    pub const fn new(signal_kind: SignalKind, kind: PedalNodeKind) -> Self {
        Self { signal_kind, kind }
    }

    /// Embeds a static number. Because constants don't move, they operate at `Control` rate.
    #[must_use]
    pub const fn constant(value: f32) -> Self {
        Self::new(
            SignalKind::Control,
            PedalNodeKind::Constant {
                value_bits: value.to_bits(),
            },
        )
    }

    /// Constructs a low-frequency oscillator running at `Control` rate to sweep parameters over time.
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

    /// Sets up an envelope follower reacting to transients, generating a `Control` rate curve.
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

    /// Defines a virtual analog transformation block forcing execution to the `Audio` frame rate.
    #[must_use]
    pub const fn stage(stage: PedalStage) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Stage(stage))
    }

    /// Combines two signals additively. The resulting signal inherits the faster execution rate of its inputs.
    #[must_use]
    pub const fn add(signal_kind: SignalKind, left: NodeRef, right: NodeRef) -> Self {
        Self::new(signal_kind, PedalNodeKind::Add { left, right })
    }

    /// Scales one signal against another. Forces an `Audio` rate execution because AM modulation can introduce audible artifacts if subsampled.
    #[must_use]
    pub const fn mul(left: NodeRef, right: NodeRef) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mul { left, right })
    }

    /// Averages a bank of `Audio` signals together safely.
    #[must_use]
    pub const fn mix(inputs: Vec<NodeRef>) -> Self {
        Self::new(SignalKind::Audio, PedalNodeKind::Mix { inputs })
    }

    /// Instructs the DSP engine to allocate a stateful delay buffer loop for this `Audio` stream.
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

    /// Interrogates whether the node's output should be queried once per frame or once per sub-block.
    #[must_use]
    pub const fn signal_kind(&self) -> SignalKind {
        self.signal_kind
    }

    /// Exposes the internal parameters directing the DSP math operations.
    #[must_use]
    pub const fn kind(&self) -> &PedalNodeKind {
        &self.kind
    }
}

/// A fully compiled, immutable template describing a virtual analog routing topology.
///
/// `PedalGraphProgram` serves as the compiled bytecode format generated by the language
/// parser. The audio thread takes this flat vector of `PedalNode` elements and
/// sequentially executes them over a `node_values` scratch buffer every sample cycle.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PedalGraphProgram {
    nodes: Vec<PedalNode>,
    output: NodeRef,
}

impl PedalGraphProgram {
    /// Finalizes the graph creation, providing the flattened execution list and identifying which index is sent to the speakers.
    #[must_use]
    pub const fn new(nodes: Vec<PedalNode>, output: NodeRef) -> Self {
        Self { nodes, output }
    }

    /// Provides read-only access to the sequential list of instructions the `PedalInstance` will execute.
    #[must_use]
    pub fn nodes(&self) -> &[PedalNode] {
        &self.nodes
    }

    /// Points to the specific node in the flat array whose output defines the final wet signal.
    #[must_use]
    pub const fn output(&self) -> NodeRef {
        self.output
    }

    /// Fast-path check identifying empty effects that just route input directly to output, bypassing DSP evaluation entirely.
    #[allow(clippy::incompatible_msrv)]
    #[must_use]
    #[allow(clippy::incompatible_msrv)]
    pub const fn is_bypass(&self) -> bool {
        self.nodes.is_empty() && matches!(self.output, NodeRef::Input)
    }
}
