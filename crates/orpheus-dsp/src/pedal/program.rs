#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalKind {
    Audio,
    Control,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NodeRef {
    #[default]
    Input,
    Node(usize),
}

impl NodeRef {
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

    #[must_use]
    #[allow(clippy::incompatible_msrv)]
    pub const fn is_bypass(&self) -> bool {
        self.nodes.is_empty() && matches!(self.output, NodeRef::Input)
    }
}
