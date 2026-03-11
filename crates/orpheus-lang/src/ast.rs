//! Abstract syntax tree nodes for the Phase 1 Orpheus parser.

/// A parsed Orpheus module.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// Top-level statements in source order.
    pub statements: Vec<Stmt>,
}

/// Phase 1 expression forms.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// Sequential composition created by juxtaposition.
    Seq(Vec<Self>),
    /// Parallel composition created by `stack(...)`.
    Stack(Vec<Self>),
    /// Explicit-time event stream composition created by `stream(...)`.
    Stream(Vec<Self>),
    /// Pipe application created by `lhs |> rhs`.
    Pipe { lhs: Box<Self>, rhs: Box<Self> },
    /// Function application created by `callee(...)`.
    Call { callee: Box<Self>, args: Vec<Self> },
    /// Explicit placement created by `at(time, pattern)`.
    At {
        start: Box<Self>,
        pattern: Box<Self>,
    },
    /// Meter annotation created by `meter(n, d, pattern)`.
    Meter {
        beats: Box<Self>,
        unit: Box<Self>,
        pattern: Box<Self>,
    },
    /// Beat-relative numeric literal created by `beat(...)`.
    Beat(Box<Self>),
    /// One section in a song structure created by `section(pattern, cycles)`.
    Section {
        pattern: Box<Self>,
        cycles: Box<Self>,
    },
    /// Sequential section composition created by `seq_sections(...)`.
    SeqSections(Vec<Self>),
    /// Parenthesized pattern group.
    Group(Vec<Self>),
    /// A bare identifier.
    Ident(String),
    /// A rest marker.
    Rest,
    /// A numeric literal.
    Number(f64),
    /// A string literal.
    String(String),
}

/// Phase 1 statements.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// A top-level binding statement.
    Binding { name: String, expr: Expr },
}
