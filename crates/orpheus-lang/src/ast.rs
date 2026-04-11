//! Abstract syntax tree nodes for the Phase 1 Orpheus parser.

/// A parsed Orpheus module.
///
/// Represents the root of the abstract syntax tree after parsing.
/// A module is a sequence of top-level binding statements.
///
/// # Examples
///
/// ```
/// use orpheus_lang::parse_module;
///
/// let source = "drums = bd sn\nfx = fast(2, drums)";
/// let module = parse_module(source).unwrap();
/// assert_eq!(module.statements.len(), 2);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// Top-level statements in source order.
    pub statements: Vec<Stmt>,
}

/// Phase 1 expression forms.
///
/// Captures the raw syntactic structure of Orpheus patterns before type inference
/// or evaluation. This includes primitives like literals and identifiers, as well
/// as composite structures like sequences, pipes, and function calls.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{parse_module, Stmt, Expr};
///
/// let module = parse_module("x = 1").unwrap();
/// if let Stmt::Binding { expr, .. } = &module.statements[0] {
///     assert_eq!(*expr, Expr::Number(1.0));
/// }
/// ```
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
        /// The left-hand side expression to be piped.
        lhs: Box<Self>,
        /// The right-hand side function receiving the pipe.
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
    /// A bare identifier.
    Ident(String),
    /// A rest marker.
    Rest,
    /// A numeric literal.
    Number(f64),
    /// A string literal.
    String(String),
}

impl Expr {
    fn references_ident(&self, target: &str) -> bool {
        match self {
            Self::Seq(items)
            | Self::Stack(items)
            | Self::Stream(items)
            | Self::SeqSections(items)
            | Self::Group(items) => items.iter().any(|item| item.references_ident(target)),
            Self::Pipe { lhs, rhs } => lhs.references_ident(target) || rhs.references_ident(target),
            Self::Call { callee, args } => {
                callee.references_ident(target)
                    || args.iter().any(|arg| arg.references_ident(target))
            }
            Self::At { start, pattern } => {
                start.references_ident(target) || pattern.references_ident(target)
            }
            Self::Meter {
                beats,
                unit,
                pattern,
            } => {
                beats.references_ident(target)
                    || unit.references_ident(target)
                    || pattern.references_ident(target)
            }
            Self::Beat(value) => value.references_ident(target),
            Self::Section { pattern, cycles } => {
                pattern.references_ident(target) || cycles.references_ident(target)
            }
            Self::Ident(name) => name == target,
            Self::Rest | Self::Number(_) | Self::String(_) => false,
        }
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
}
