use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use orpheus_pattern::PatternNode;

use crate::ReplMode;
use crate::ast::{Expr, Module, Stmt};
use crate::builtins::{builtin_value, is_sample_identifier, stack_values};
use crate::diagnostics::ParseError;
use crate::parser::parse_module;
use crate::value::{NumberPatternValue, SampleEvent, SamplePatternValue, Value};

/// Runtime evaluation error for bootstrap Orpheus modules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalError {
    message: Box<str>,
}

impl EvalError {
    pub(crate) fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for EvalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EvalError {}

impl From<ParseError> for EvalError {
    fn from(error: ParseError) -> Self {
        Self::new(error.to_string())
    }
}

/// Evaluates bootstrap Orpheus source into runtime values.
///
/// # Errors
///
/// Returns [`EvalError`] when parsing fails or when evaluation encounters an
/// unsupported expression or builtin application.
pub fn eval_module(source: &str, mode: ReplMode) -> Result<BTreeMap<String, Value>, EvalError> {
    let parsed = parse_module(source)?;
    Evaluator::new(mode).eval_module(&parsed)
}

struct Evaluator {
    mode: ReplMode,
    bindings: BTreeMap<String, Value>,
}

impl Evaluator {
    const fn new(mode: ReplMode) -> Self {
        Self {
            mode,
            bindings: BTreeMap::new(),
        }
    }

    fn eval_module(mut self, module: &Module) -> Result<BTreeMap<String, Value>, EvalError> {
        for statement in &module.statements {
            match statement {
                Stmt::Binding { name, expr } => {
                    let value = self.eval_expr(expr)?;
                    self.bindings.insert(name.clone(), value);
                }
            }
        }

        Ok(self.bindings)
    }

    fn eval_expr(&self, expr: &Expr) -> Result<Value, EvalError> {
        match expr {
            Expr::Seq(items) => self.eval_sequence(items),
            Expr::Stack(layers) => self.eval_stack(layers),
            Expr::Pipe { lhs, rhs } => self.eval_pipe(lhs, rhs),
            Expr::Call { callee, args } => self.eval_call(callee, args),
            Expr::Group(items) => self.eval_group(items),
            Expr::Ident(name) => self.eval_ident(name),
            Expr::Rest => Err(EvalError::new(
                "rest markers can only appear inside pattern sequences",
            )),
            Expr::Number(value) => Ok(Value::NumberPattern(NumberPatternValue::constant(*value))),
        }
    }

    fn eval_sequence(&self, items: &[Expr]) -> Result<Value, EvalError> {
        if let Some(nodes) = self.collect_sample_nodes(items)? {
            return Ok(Value::SamplePattern(SamplePatternValue::from_nodes(nodes)));
        }

        if let Some(nodes) = self.collect_number_nodes(items)? {
            return Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)));
        }

        Err(EvalError::new(
            "sequence items must all resolve to the same structural pattern kind",
        ))
    }

    fn eval_group(&self, items: &[Expr]) -> Result<Value, EvalError> {
        if let Some(nodes) = self.collect_sample_nodes(items)? {
            return Ok(Value::SamplePattern(SamplePatternValue::from_group(nodes)));
        }

        if let Some(nodes) = self.collect_number_nodes(items)? {
            return Ok(Value::NumberPattern(NumberPatternValue::from_group(nodes)));
        }

        Err(EvalError::new(
            "group items must all resolve to the same structural pattern kind",
        ))
    }

    fn eval_stack(&self, layers: &[Expr]) -> Result<Value, EvalError> {
        let mut values = Vec::with_capacity(layers.len());
        for layer in layers {
            values.push(self.eval_expr(layer)?);
        }

        stack_values(values)
    }

    fn eval_pipe(&self, lhs: &Expr, rhs: &Expr) -> Result<Value, EvalError> {
        let lhs_value = self.eval_expr(lhs)?;
        match rhs {
            Expr::Call { callee, args } => self.eval_call_with_args(callee, args, vec![lhs_value]),
            _ => Self::apply_value(self.eval_expr(rhs)?, vec![lhs_value]),
        }
    }

    fn eval_call(&self, callee: &Expr, args: &[Expr]) -> Result<Value, EvalError> {
        self.eval_call_with_args(callee, args, Vec::new())
    }

    fn eval_call_with_args(
        &self,
        callee: &Expr,
        args: &[Expr],
        piped_args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let callee_value = self.eval_expr(callee)?;
        let mut evaluated_args = Vec::with_capacity(args.len() + piped_args.len());
        for arg in args {
            evaluated_args.push(self.eval_expr(arg)?);
        }
        evaluated_args.extend(piped_args);
        Self::apply_value(callee_value, evaluated_args)
    }

    fn apply_value(callee: Value, args: Vec<Value>) -> Result<Value, EvalError> {
        match callee {
            Value::Function(function) => function.apply(args),
            Value::SamplePattern(_) | Value::NumberPattern(_) => Err(EvalError::new(format!(
                "cannot call a {}",
                callee.kind_name()
            ))),
        }
    }

    fn eval_ident(&self, name: &str) -> Result<Value, EvalError> {
        if let Some(value) = self.bindings.get(name) {
            return Ok(value.clone());
        }

        if let Some(value) = builtin_value(name) {
            return Ok(value);
        }

        match self.mode {
            ReplMode::Loose => Err(EvalError::new(format!(
                "unresolved identifier `{name}`; loose-mode placeholders are not implemented yet"
            ))),
            ReplMode::Strict => Err(EvalError::new(format!("unresolved identifier `{name}`"))),
        }
    }

    fn collect_sample_nodes(
        &self,
        items: &[Expr],
    ) -> Result<Option<Vec<PatternNode<SampleEvent>>>, EvalError> {
        let mut nodes = Vec::with_capacity(items.len());
        for item in items {
            let Some(node) = self.try_sample_node(item)? else {
                return Ok(None);
            };
            nodes.push(node);
        }

        Ok(Some(nodes))
    }

    fn try_sample_node(&self, expr: &Expr) -> Result<Option<PatternNode<SampleEvent>>, EvalError> {
        match expr {
            Expr::Ident(name) if is_sample_identifier(name) => {
                Ok(Some(PatternNode::atom(SampleEvent::named(name))))
            }
            Expr::Rest => Ok(Some(PatternNode::rest())),
            Expr::Group(items) => {
                let Some(nodes) = self.collect_sample_nodes(items)? else {
                    return Ok(None);
                };
                Ok(Some(PatternNode::group(nodes)))
            }
            Expr::Seq(_)
            | Expr::Stack(_)
            | Expr::Pipe { .. }
            | Expr::Call { .. }
            | Expr::Ident(_)
            | Expr::Number(_) => Ok(None),
        }
    }

    fn collect_number_nodes(
        &self,
        items: &[Expr],
    ) -> Result<Option<Vec<PatternNode<f64>>>, EvalError> {
        let mut nodes = Vec::with_capacity(items.len());
        for item in items {
            let Some(node) = self.try_number_node(item)? else {
                return Ok(None);
            };
            nodes.push(node);
        }

        Ok(Some(nodes))
    }

    fn try_number_node(&self, expr: &Expr) -> Result<Option<PatternNode<f64>>, EvalError> {
        match expr {
            Expr::Number(value) => Ok(Some(PatternNode::atom(*value))),
            Expr::Rest => Ok(Some(PatternNode::rest())),
            Expr::Group(items) => {
                let Some(nodes) = self.collect_number_nodes(items)? else {
                    return Ok(None);
                };
                Ok(Some(PatternNode::group(nodes)))
            }
            Expr::Seq(_)
            | Expr::Stack(_)
            | Expr::Pipe { .. }
            | Expr::Call { .. }
            | Expr::Ident(_) => Ok(None),
        }
    }
}
