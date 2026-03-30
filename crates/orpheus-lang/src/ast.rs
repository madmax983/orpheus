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
    Pipe {
        /// The left-hand side pattern being piped.
        lhs: Box<Self>,
        /// The right-hand side function receiving the pattern.
        rhs: Box<Self>,
    },
    /// Function application created by `callee(...)`.
    Call {
        /// The function being called.
        callee: Box<Self>,
        /// The arguments passed to the function.
        args: Vec<Self>,
    },
    /// Explicit placement created by `at(time, pattern)`.
    At {
        /// The start time for the pattern.
        start: Box<Self>,
        /// The pattern to place at the specific time.
        pattern: Box<Self>,
    },
    /// Meter annotation created by `meter(n, d, pattern)`.
    Meter {
        /// The number of beats in a measure.
        beats: Box<Self>,
        /// The unit of the beat (e.g. 4 for quarter note).
        unit: Box<Self>,
        /// The pattern to annotate with the meter.
        pattern: Box<Self>,
    },
    /// Beat-relative numeric literal created by `beat(...)`.
    Beat(Box<Self>),
    /// One section in a song structure created by `section(pattern, cycles)`.
    Section {
        /// The pattern to loop for the section.
        pattern: Box<Self>,
        /// The number of cycles to run the pattern.
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
    Binding {
        /// The name being bound.
        name: String,
        /// The expression being bound to the name.
        expr: Expr,
    },
}
