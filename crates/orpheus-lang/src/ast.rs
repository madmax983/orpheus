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
    /// A let-bound graph block used by the pedal DSL.
    Graph {
        bindings: Vec<GraphBinding>,
        result: Box<Self>,
    },
    /// Pipe application created by `lhs |> rhs`.
    Pipe { lhs: Box<Self>, rhs: Box<Self> },
    /// Binary arithmetic used by graph-local control expressions.
    Binary {
        lhs: Box<Self>,
        op: BinaryOp,
        rhs: Box<Self>,
    },
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

/// A single let-bound signal inside a pedal graph.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphBinding {
    /// The local signal name.
    pub name: String,
    /// The bound expression.
    pub expr: Expr,
}

/// Arithmetic operators supported by the pedal graph surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Multiplication.
    Mul,
    /// Assignment-like named argument syntax.
    Assign,
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
            | Self::Group(items) => items
                .iter()
                .any(|item| item.references_ident_with_shadow(target, shadowed)),
            Self::Graph { bindings, result } => {
                let mut shadowed = shadowed;
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
            Self::Pipe { lhs, rhs } => {
                lhs.references_ident_with_shadow(target, shadowed)
                    || rhs.references_ident_with_shadow(target, shadowed)
            }
            Self::Binary { lhs, rhs, .. } => {
                lhs.references_ident_with_shadow(target, shadowed)
                    || rhs.references_ident_with_shadow(target, shadowed)
            }
            Self::Call { callee, args } => {
                callee.references_ident_with_shadow(target, shadowed)
                    || args
                        .iter()
                        .any(|arg| arg.references_ident_with_shadow(target, shadowed))
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
            Self::Section { pattern, cycles } => {
                pattern.references_ident_with_shadow(target, shadowed)
                    || cycles.references_ident_with_shadow(target, shadowed)
            }
            Self::Ident(name) => !shadowed && name == target,
            Self::Rest | Self::Number(_) | Self::String(_) => false,
        }
    }
}

/// Phase 1 statements.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// A top-level binding statement.
    Binding {
        name: String,
        params: Vec<String>,
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
