#![allow(clippy::match_same_arms)]
//! Abstract syntax tree nodes for the Phase 1 Orpheus parser.
//!
//! This module defines the structure of the AST emitted by the parser before
//! any type inference or evaluation has taken place. The primary entrypoint
//! is the [`Module`] struct, which contains a collection of [`Stmt`] nodes.

/// A parsed Orpheus module.
///
/// # Examples
///
/// ```
/// use orpheus_lang::parse_module;
///
/// let module = parse_module("f = bd").unwrap();
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// Top-level statements in source order.
    pub statements: Vec<Stmt>,
}

/// Phase 1 expression forms.
///
/// # Examples
///
/// ```
/// use orpheus_lang::Expr;
///
/// let expr = Expr::Ident("bd".to_string());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// Sequential composition created by juxtaposition.
    Seq(Vec<Self>),
    /// Parallel composition created by `stack(...)`.
    Stack(Vec<Self>),
    /// Explicit-time event stream composition created by `stream(...)`.
    Stream(Vec<Self>),
    /// A let-bound graph block used by the pedal DSL.
    Graph {
        /// Intermediary bindings assigned in the block.
        bindings: Vec<GraphBinding>,
        /// Final yield expression.
        result: Box<Self>,
    },
    /// A named instrument definition created by `voice { ... }`.
    Voice {
        /// Intermediary bindings assigned in the block.
        bindings: Vec<GraphBinding>,
        /// The final mono voice signal expression.
        result: Box<Self>,
    },
    /// Pipe application created by `lhs |> rhs`.
    Pipe {
        /// The left-hand side expression to be piped.
        lhs: Box<Self>,
        /// The right-hand side function receiving the pipe.
        rhs: Box<Self>,
    },
    /// Binary arithmetic used by graph-local control expressions.
    Binary {
        /// The left-hand side operand.
        lhs: Box<Self>,
        /// The operator between the operands.
        op: BinaryOp,
        /// The right-hand side operand.
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
        /// The explicit time offset.
        start: Box<Self>,
        /// The pattern to schedule at the offset.
        pattern: Box<Self>,
    },
    /// Meter annotation created by `meter(n, d, pattern)`.
    Meter {
        /// The number of beats per measure.
        beats: Box<Self>,
        /// The duration of a single beat.
        unit: Box<Self>,
        /// The pattern to apply the meter to.
        pattern: Box<Self>,
    },
    /// Beat-relative numeric literal created by `beat(...)`.
    Beat(Box<Self>),
    /// One section in a song structure created by `section(pattern, cycles)`.
    Section {
        /// The pattern representing the musical section.
        pattern: Box<Self>,
        /// The duration of the section in cycles.
        cycles: Box<Self>,
    },
    /// Sequential section composition created by `seq_sections(...)`.
    SeqSections(Vec<Self>),
    /// Parenthesized pattern group.
    Group(Vec<Self>),
    /// Per-cycle alternation created by `<a b c>`; plays one element per cycle.
    Alternation(Vec<Self>),
    /// A sequence element with a tight postfix step operator, e.g. `bd*2`,
    /// `bd/2`, or `bd?`. Replication (`bd!3`) is expanded into separate steps
    /// by the parser, so `StepOp::Replicate` never survives in step position.
    Modified {
        /// The element the operator applies to.
        inner: Box<Self>,
        /// The postfix operator.
        op: StepOp,
    },
    /// Polymeter created by `{a b, c d e}%n`; each subsequence wraps its own
    /// steps while playing `steps` of them per cycle (the first subsequence's
    /// length when no `%n` suffix is given).
    Polymeter {
        /// The comma-separated subsequences.
        groups: Vec<Vec<Self>>,
        /// Optional explicit steps-per-cycle override from `%n`.
        steps: Option<i64>,
    },
    /// A bare identifier.
    Ident(String),
    /// A rest marker.
    Rest,
    /// A numeric literal.
    Number(f64),
    /// A string literal.
    String(String),
}

/// A single let-bound signal inside a pedal graph.
///
/// # Examples
///
/// ```
/// use orpheus_lang::Expr;
/// use orpheus_lang::GraphBinding;
///
/// let binding = GraphBinding {
///     name: "x".to_string(),
///     expr: Expr::Number(1.0),
/// };
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct GraphBinding {
    /// The local signal name.
    pub name: String,
    /// The bound expression.
    pub expr: Expr,
}

/// Arithmetic operators supported by the pedal graph surface.
///
/// # Examples
///
/// ```
/// use orpheus_lang::BinaryOp;
///
/// let op = BinaryOp::Add;
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Multiplication.
    Mul,
    /// Assignment-like named argument syntax.
    Assign,
}

/// Tight postfix mini-notation operators on sequence elements.
///
/// # Examples
///
/// ```
/// use orpheus_lang::StepOp;
///
/// let op = StepOp::Fast(2.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepOp {
    /// `a*n`: speeds the element up by `n` within its own slot (`bd*2`
    /// repeats it twice, `bd*1.5` plays it at 3/2 speed).
    ///
    /// The raw factor is kept as parsed; inside `graph { ... }` blocks the
    /// parser rewrites it back into pedal-DSL multiplication, elsewhere it
    /// must convert to a positive rational whose numerator and denominator
    /// are each at most 1024 (the shared `fast`/`slow` factor rules).
    Fast(f64),
    /// `a/n`: slows the element so it takes `n` cycles to complete; decimal
    /// factors convert to exact rationals under the same bounds as `a*n`.
    Slow(f64),
    /// `a!n`: replicates the element as `n` separate sequence steps.
    Replicate(i64),
    /// `a?` / `a?p`: randomly removes the element's events with drop
    /// probability `p` (0.5 for bare `?`).
    Degrade(f64),
}

impl Expr {
    fn references_ident(&self, target: &str) -> bool {
        self.references_ident_with_shadow(target, false)
    }

    fn references_ident_with_shadow(&self, target: &str, shadowed: bool) -> bool {
        match self {
            Self::Seq(items)
            | Self::Stack(items)
            | Self::Stream(items)
            | Self::SeqSections(items)
            | Self::Group(items)
            | Self::Alternation(items) => Self::references_ident_in_list(items, target, shadowed),
            Self::Graph { bindings, result } | Self::Voice { bindings, result } => {
                Self::references_ident_in_graph(bindings, result, target, shadowed)
            }
            Self::Pipe { lhs, rhs } | Self::Binary { lhs, rhs, .. } => {
                lhs.references_ident_with_shadow(target, shadowed)
                    || rhs.references_ident_with_shadow(target, shadowed)
            }
            Self::Call { callee, args } => {
                callee.references_ident_with_shadow(target, shadowed)
                    || Self::references_ident_in_list(args, target, shadowed)
            }
            Self::At { start, pattern } => {
                start.references_ident_with_shadow(target, shadowed)
                    || pattern.references_ident_with_shadow(target, shadowed)
            }
            Self::Meter {
                beats,
                unit,
                pattern,
            } => {
                beats.references_ident_with_shadow(target, shadowed)
                    || unit.references_ident_with_shadow(target, shadowed)
                    || pattern.references_ident_with_shadow(target, shadowed)
            }
            Self::Beat(value) => value.references_ident_with_shadow(target, shadowed),
            Self::Modified { inner, .. } => inner.references_ident_with_shadow(target, shadowed),
            Self::Polymeter { groups, .. } => groups
                .iter()
                .any(|group| Self::references_ident_in_list(group, target, shadowed)),
            Self::Section { pattern, cycles } => {
                pattern.references_ident_with_shadow(target, shadowed)
                    || cycles.references_ident_with_shadow(target, shadowed)
            }
            Self::Ident(name) => !shadowed && name == target,
            Self::Rest | Self::Number(_) | Self::String(_) => false,
        }
    }

    fn references_ident_in_list(items: &[Self], target: &str, shadowed: bool) -> bool {
        items
            .iter()
            .any(|item| item.references_ident_with_shadow(target, shadowed))
    }

    fn references_ident_in_graph(
        bindings: &[GraphBinding],
        result: &Self,
        target: &str,
        mut shadowed: bool,
    ) -> bool {
        for binding in bindings {
            if binding.expr.references_ident_with_shadow(target, shadowed) {
                return true;
            }
            if binding.name == target {
                shadowed = true;
            }
        }
        result.references_ident_with_shadow(target, shadowed)
    }
}

/// Phase 1 statements.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// A top-level binding statement.
    Binding {
        /// The name of the identifier being bound.
        name: String,
        /// Optional parameter names for user-defined functions.
        params: Vec<String>,
        /// The right-hand side expression.
        expr: Expr,
    },
}

/// Checks if a binding expression references its own binding name,
/// indicating a recursive definition.
///
/// This is used during type inference and evaluation to detect cyclic
/// dependencies or infinite loops, unless it's a valid local shadowing.
///
/// # Parameters
/// - `name`: The name of the binding being defined.
/// - `params`: The list of parameters bound by the function (to check for shadowing).
/// - `expr`: The body expression of the binding.
///
/// # Returns
/// `true` if the identifier is referenced in the expression body and is not shadowed
/// by a parameter. `false` otherwise.
///
#[doc(hidden)]
pub fn binding_expr_self_references(name: &str, params: &[String], expr: &Expr) -> bool {
    !params.iter().any(|param| param == name) && expr.references_ident(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_references_ident_seq() {
        let expr = Expr::Seq(vec![Expr::Ident("foo".to_string()), Expr::Number(42.0)]);
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_stack() {
        let expr = Expr::Stack(vec![
            Expr::String("bar".to_string()),
            Expr::Ident("foo".to_string()),
        ]);
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("baz"));
    }

    #[test]
    fn test_references_ident_stream() {
        let expr = Expr::Stream(vec![Expr::Ident("foo".to_string())]);
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_seq_sections() {
        let expr = Expr::SeqSections(vec![Expr::Ident("foo".to_string())]);
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_group() {
        let expr = Expr::Group(vec![Expr::Ident("foo".to_string())]);
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_at() {
        let expr = Expr::At {
            start: Box::new(Expr::Ident("foo".to_string())),
            pattern: Box::new(Expr::Number(1.0)),
        };
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));

        let expr2 = Expr::At {
            start: Box::new(Expr::Number(1.0)),
            pattern: Box::new(Expr::Ident("foo".to_string())),
        };
        assert!(expr2.references_ident("foo"));
        assert!(!expr2.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_meter() {
        let expr = Expr::Meter {
            beats: Box::new(Expr::Ident("foo".to_string())),
            unit: Box::new(Expr::Number(4.0)),
            pattern: Box::new(Expr::Number(1.0)),
        };
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));

        let expr2 = Expr::Meter {
            beats: Box::new(Expr::Number(4.0)),
            unit: Box::new(Expr::Ident("foo".to_string())),
            pattern: Box::new(Expr::Number(1.0)),
        };
        assert!(expr2.references_ident("foo"));
        assert!(!expr2.references_ident("bar"));

        let expr3 = Expr::Meter {
            beats: Box::new(Expr::Number(4.0)),
            unit: Box::new(Expr::Number(4.0)),
            pattern: Box::new(Expr::Ident("foo".to_string())),
        };
        assert!(expr3.references_ident("foo"));
        assert!(!expr3.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_beat() {
        let expr = Expr::Beat(Box::new(Expr::Ident("foo".to_string())));
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));
    }

    #[test]
    fn test_references_ident_section() {
        let expr = Expr::Section {
            pattern: Box::new(Expr::Ident("foo".to_string())),
            cycles: Box::new(Expr::Number(4.0)),
        };
        assert!(expr.references_ident("foo"));
        assert!(!expr.references_ident("bar"));

        let expr2 = Expr::Section {
            pattern: Box::new(Expr::Number(4.0)),
            cycles: Box::new(Expr::Ident("foo".to_string())),
        };
        assert!(expr2.references_ident("foo"));
        assert!(!expr2.references_ident("bar"));
    }

    #[test]
    fn test_binding_expr_self_references_when_not_shadowed() {
        let params = vec![];
        let expr = Expr::Ident("foo".to_string());
        assert!(binding_expr_self_references("foo", &params, &expr));
    }

    #[test]
    fn test_binding_expr_self_references_when_shadowed_by_parameter() {
        let params = vec!["foo".to_string()];
        let expr = Expr::Ident("foo".to_string());
        assert!(!binding_expr_self_references("foo", &params, &expr));
    }

    #[test]
    fn test_binding_expr_self_references_with_graph_shadowing() {
        let shadowing_result = Expr::Graph {
            bindings: vec![GraphBinding {
                name: "foo".to_string(),
                expr: Expr::Ident("bar".to_string()),
            }],
            result: Box::new(Expr::Ident("foo".to_string())),
        };
        assert!(!binding_expr_self_references("foo", &[], &shadowing_result));

        let shadowing_rhs = Expr::Graph {
            bindings: vec![GraphBinding {
                name: "foo".to_string(),
                expr: Expr::Ident("foo".to_string()),
            }],
            result: Box::new(Expr::Number(0.0)),
        };
        assert!(binding_expr_self_references("foo", &[], &shadowing_rhs));
    }
}
