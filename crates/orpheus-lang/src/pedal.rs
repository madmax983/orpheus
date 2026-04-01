use core::fmt::{self, Display, Formatter};

/// The coarse signal domain understood by the pedal DSL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignalKind {
    Audio,
    Control,
}

impl Display for SignalKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio => formatter.write_str("Audio"),
            Self::Control => formatter.write_str("Control"),
        }
    }
}

/// A source-level pedal graph wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalGraph {
    source: String,
}

impl PedalGraph {
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn format_source(&self) -> String {
        self.source.clone()
    }
}

/// A validated pedal plan, ready for later lowering or rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPedalPlan {
    signal_kind: SignalKind,
    summary: String,
}

impl ValidatedPedalPlan {
    #[must_use]
    pub fn new(signal_kind: SignalKind, summary: impl Into<String>) -> Self {
        Self {
            signal_kind,
            summary: summary.into(),
        }
    }

    #[must_use]
    pub fn signal_kind(&self) -> &SignalKind {
        &self.signal_kind
    }

    #[must_use]
    pub fn explain(&self) -> String {
        format!("signal_kind={}, plan={}", self.signal_kind, self.summary)
    }
}

/// The language-side value for a pedal graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PedalValue {
    graph: PedalGraph,
    plan: ValidatedPedalPlan,
}

impl PedalValue {
    #[must_use]
    pub fn new(graph: PedalGraph, plan: ValidatedPedalPlan) -> Self {
        Self { graph, plan }
    }

    #[must_use]
    pub fn graph(&self) -> &PedalGraph {
        &self.graph
    }

    #[must_use]
    pub fn plan(&self) -> &ValidatedPedalPlan {
        &self.plan
    }

    #[must_use]
    pub fn format_source(&self) -> String {
        self.graph.format_source()
    }

    #[must_use]
    pub fn explain(&self) -> String {
        self.plan.explain()
    }
}
