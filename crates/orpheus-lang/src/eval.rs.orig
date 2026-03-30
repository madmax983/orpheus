//! Evaluation engine for Orpheus source code.
//!
//! This module is responsible for translating parsed Phase 1 abstract syntax
//! trees (ASTs) into concrete runtime `Value`s. It manages the environment
//! (bindings), evaluates sequence and structural patterns into `PatternRuntime`s,
//! and handles rendering those patterns into offline audio or exporting them.
//!
//! # Examples
//!
//! The entry point for evaluation is `eval_module`, which processes source
//! code and returns a set of bound values:
//!
//! ```
//! use orpheus_lang::{ReplMode, eval_module};
//!
//! let source = "song = fast(2, bd sn)";
//! let bindings = eval_module(source, ReplMode::Loose).unwrap();
//!
//! assert!(bindings.contains_key("song"));
//! ```

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use orpheus_pattern::{Event, PatternNode, Rational, TimeSpan};

use crate::ReplMode;
use crate::ast::{Expr, Module, Stmt};
use crate::builtins::{builtin_value, is_sample_identifier, stack_values};
use crate::diagnostics::ParseError;
use crate::parser::parse_module;
use crate::value::{NumberPatternValue, SampleEvent, SamplePatternValue, Value};

/// Runtime evaluation error for bootstrap Orpheus modules.
///
/// `EvalError` occurs when an expression fails to evaluate at runtime.
/// In Orpheus, evaluation errors often stem from invalid arithmetic on rational
/// time domains (like dividing by zero), out-of-bounds parameters, or attempting
/// to use an unsupported operation on a pattern. Orpheus patterns operate in an
/// exact, bounded rational time domain, so overflows during shifts or scaling
/// can result in an `EvalError`.
///
/// # Examples
///
/// An `EvalError` provides an error message indicating what went wrong:
///
/// ```
/// use orpheus_lang::EvalError;
///
/// let err = EvalError::new("decimal literal exceeded the supported range");
/// assert_eq!(err.to_string(), "decimal literal exceeded the supported range");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalError {
    message: Box<str>,
}

impl EvalError {
    /// Creates a new `EvalError` with the given message.
    ///
    /// The message explains what went wrong during runtime evaluation.
    ///
    /// Common causes for `EvalError` include:
    /// - Out-of-bounds numeric parameters.
    /// - Arithmetic overflow during explicit time-shifts.
    /// - Applying functions to invalid types.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::EvalError;
    ///
    /// let err = EvalError::new("division by zero");
    /// assert_eq!(err.to_string(), "division by zero");
    /// ```
    pub fn new(message: impl Into<Box<str>>) -> Self {
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
/// Given a string of Orpheus source code, this parses the text into an AST,
/// executes it, and returns the resulting environment of named `Value`s.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
///
/// let source = "x = fast(2, bd sn)";
/// let env = eval_module(source, ReplMode::Loose).unwrap();
/// let val = env.get("x").unwrap();
///
/// assert!(val.as_sample_pattern().is_some());
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] when parsing fails or when evaluation encounters an
/// unsupported expression or builtin application.
pub fn eval_module(source: &str, mode: ReplMode) -> Result<BTreeMap<String, Value>, EvalError> {
    let parsed = parse_module(source)?;
    Evaluator::new(mode, &parsed).eval_module(&parsed)
}

/// Evaluates a source module directly into an existing set of bindings.
///
/// This is used heavily by the REPL to maintain state across multiple
/// sequential inputs. It optionally returns the last evaluated statement's
/// binding name and value, which is useful for printing the result of an assignment.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use orpheus_lang::{ReplMode, eval_into_bindings};
///
/// let mut env = BTreeMap::new();
/// eval_into_bindings("a = 1", ReplMode::Loose, &mut env).unwrap();
/// eval_into_bindings("b = a", ReplMode::Loose, &mut env).unwrap();
///
/// assert!(env.contains_key("b"));
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if parsing fails, or if evaluation encounters a runtime error.
pub fn eval_into_bindings(
    source: &str,
    mode: ReplMode,
    bindings: &mut BTreeMap<String, Value>,
) -> Result<Option<(String, Value)>, EvalError> {
    let parsed = parse_module(source)?;
    // ⚡ Bolt: Use `std::mem::take` instead of `bindings.clone()` to move the BTreeMap into the evaluator.
    // This avoids a full heap allocation and deep copy of the environment on every REPL statement.
    let mut evaluator = Evaluator::with_bindings(mode, std::mem::take(bindings), &parsed);
    let result = evaluator.eval_statements(&parsed.statements);
    *bindings = evaluator.bindings;
    result
}

struct Evaluator {
    mode: ReplMode,
    bindings: BTreeMap<String, Value>,
    expr_site_salts: BTreeMap<usize, u64>,
}

#[derive(Clone, Copy, Debug)]
struct MeterContext {
    beats_per_cycle: i128,
}

#[derive(Clone, Debug)]
enum ExplicitValue {
    Sample(Vec<Event<SampleEvent>>),
    Number(Vec<Event<f64>>),
}

impl ExplicitValue {
    fn into_value(self) -> Value {
        match self {
            Self::Sample(events) => Value::SamplePattern(SamplePatternValue::from_events(events)),
            Self::Number(events) => Value::NumberPattern(NumberPatternValue::from_events(events)),
        }
    }

    fn merge(self, other: Self) -> Result<Self, EvalError> {
        match (self, other) {
            (Self::Sample(mut left), Self::Sample(mut right)) => {
                left.append(&mut right);
                sort_events(&mut left);
                Ok(Self::Sample(left))
            }
            (Self::Number(mut left), Self::Number(mut right)) => {
                left.append(&mut right);
                sort_events(&mut left);
                Ok(Self::Number(left))
            }
            (Self::Sample(_), Self::Number(_)) | (Self::Number(_), Self::Sample(_)) => Err(
                EvalError::new("explicit-time items must all resolve to the same pattern kind"),
            ),
        }
    }

    fn shift(&mut self, offset: &Rational) -> Result<(), EvalError> {
        match self {
            Self::Sample(events) => shift_events(events, offset),
            Self::Number(events) => shift_events(events, offset),
        }
    }
}

impl Evaluator {
    fn new(mode: ReplMode, module: &Module) -> Self {
        Self::with_bindings(mode, BTreeMap::new(), module)
    }

    fn with_bindings(mode: ReplMode, bindings: BTreeMap<String, Value>, module: &Module) -> Self {
        Self {
            mode,
            bindings,
            expr_site_salts: collect_expr_site_salts(module),
        }
    }

    fn eval_module(mut self, module: &Module) -> Result<BTreeMap<String, Value>, EvalError> {
        self.eval_statements(&module.statements)?;
        Ok(self.bindings)
    }

    fn eval_statements(
        &mut self,
        statements: &[Stmt],
    ) -> Result<Option<(String, Value)>, EvalError> {
        let mut last_binding = None;

        for statement in statements {
            match statement {
                Stmt::Binding { name, expr } => {
                    let value = self.eval_expr(expr)?;
                    self.bindings.insert(name.clone(), value.clone());
                    last_binding = Some((name.clone(), value));
                }
            }
        }

        Ok(last_binding)
    }

    fn eval_expr(&self, expr: &Expr) -> Result<Value, EvalError> {
        self.eval_expr_in_meter(expr, None)
    }

    fn eval_expr_in_meter(
        &self,
        expr: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        match expr {
            Expr::Seq(items) => self.eval_sequence(items, meter),
            Expr::Stack(layers) => self.eval_stack(layers, meter),
            Expr::Stream(items) => self.eval_stream(items, meter),
            Expr::Pipe { lhs, rhs } => self.eval_pipe(lhs, rhs, meter),
            Expr::Call { callee, args } => self.eval_call(expr, callee, args, meter),
            Expr::At { start, pattern } => self.eval_at(start, pattern, meter),
            Expr::Meter {
                beats,
                unit,
                pattern,
            } => self.eval_meter(beats, unit, pattern, meter),
            Expr::Beat(_) => Err(EvalError::new(
                "`beat(...)` can only appear inside `at(...)` within an enclosing `meter(...)`",
            )),
            Expr::Section { .. } => Err(EvalError::new(
                "`section(...)` can only appear inside `seq_sections(...)`",
            )),
            Expr::SeqSections(sections) => self.eval_seq_sections(sections, meter),
            Expr::Group(items) => self.eval_group(items, meter),
            Expr::Ident(name) => self.eval_ident(name),
            Expr::Rest => Err(EvalError::new(
                "rest markers can only appear inside pattern sequences",
            )),
            Expr::Number(value) => Ok(Value::NumberPattern(NumberPatternValue::constant(*value))),
            Expr::String(value) => Ok(Value::String(value.clone())),
        }
    }

    fn eval_sequence(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        self.eval_structural_pattern(
            items,
            meter,
            "sequence",
            SamplePatternValue::from_nodes,
            NumberPatternValue::from_nodes,
        )
    }

    fn eval_group(&self, items: &[Expr], meter: Option<&MeterContext>) -> Result<Value, EvalError> {
        self.eval_structural_pattern(
            items,
            meter,
            "group",
            SamplePatternValue::from_group,
            NumberPatternValue::from_group,
        )
    }

    fn eval_structural_pattern(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
        context: &str,
        from_sample_nodes: impl FnOnce(Vec<PatternNode<SampleEvent>>) -> SamplePatternValue,
        from_number_nodes: impl FnOnce(Vec<PatternNode<f64>>) -> NumberPatternValue,
    ) -> Result<Value, EvalError> {
        if let Some(error) = Self::unsupported_pattern_item_error(items, context) {
            return Err(error);
        }

        if let Some(nodes) = self.collect_sample_nodes(items, meter)? {
            return Ok(Value::SamplePattern(from_sample_nodes(nodes)));
        }

        if let Some(nodes) = self.collect_number_nodes(items)? {
            return Ok(Value::NumberPattern(from_number_nodes(nodes)));
        }

        for item in items {
            let _ = self.eval_expr_in_meter(item, meter)?;
        }

        Err(EvalError::new(format!(
            "{context} items must all resolve to the same structural pattern kind"
        )))
    }

    fn eval_stack(
        &self,
        layers: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let mut values = Vec::with_capacity(layers.len());
        for layer in layers {
            values.push(self.eval_expr_in_meter(layer, meter)?);
        }

        stack_values(values)
    }

    fn eval_stream(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let explicit = self.collect_explicit_items(items, meter)?;
        Ok(explicit.into_value())
    }

    fn eval_pipe(
        &self,
        lhs: &Expr,
        rhs: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let lhs_value = self.eval_expr_in_meter(lhs, meter)?;
        match rhs {
            Expr::Call { callee, args } => {
                self.eval_call_with_args(rhs, callee, args, vec![lhs_value], meter)
            }
            _ => Self::apply_value(
                self.eval_expr_in_meter(rhs, meter)?,
                vec![lhs_value],
                self.expr_site_salt(rhs),
            ),
        }
    }

    fn eval_call(
        &self,
        call_expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        self.eval_call_with_args(call_expr, callee, args, Vec::new(), meter)
    }

    fn eval_call_with_args(
        &self,
        call_expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        piped_args: Vec<Value>,
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let callee_value = self.eval_expr_in_meter(callee, meter)?;
        let mut evaluated_args = Vec::with_capacity(args.len() + piped_args.len());
        for arg in args {
            evaluated_args.push(self.eval_expr_in_meter(arg, meter)?);
        }
        evaluated_args.extend(piped_args);
        Self::apply_value(callee_value, evaluated_args, self.expr_site_salt(call_expr))
    }

    fn eval_at(
        &self,
        start: &Expr,
        pattern: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let explicit = self.eval_at_events(start, pattern, meter)?;
        Ok(explicit.into_value())
    }

    fn eval_meter(
        &self,
        beats: &Expr,
        unit: &Expr,
        pattern: &Expr,
        outer_meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let meter = self.eval_meter_context(beats, unit, outer_meter)?;
        self.eval_expr_in_meter(pattern, Some(&meter))
    }

    fn eval_seq_sections(
        &self,
        sections: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let explicit = self.eval_seq_sections_events(sections, meter)?;
        Ok(explicit.into_value())
    }

    fn eval_explicit_expr(
        &self,
        expr: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<ExplicitValue, EvalError> {
        match expr {
            Expr::At { start, pattern } => self.eval_at_events(start, pattern, meter),
            Expr::Stream(items) => self.collect_explicit_items(items, meter),
            Expr::Meter {
                beats,
                unit,
                pattern,
            } => {
                let nested_meter = self.eval_meter_context(beats, unit, meter)?;
                self.eval_explicit_expr(pattern, Some(&nested_meter))
            }
            Expr::SeqSections(sections) => self.eval_seq_sections_events(sections, meter),
            Expr::Section { .. } => Err(EvalError::new(
                "`section(...)` can only appear inside `seq_sections(...)`",
            )),
            _ => Self::value_to_explicit(self.eval_expr_in_meter(expr, meter)?),
        }
    }

    fn collect_explicit_items(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<ExplicitValue, EvalError> {
        let Some((first, rest)) = items.split_first() else {
            return Err(EvalError::new("`stream` requires at least one item"));
        };

        let mut combined = self.eval_explicit_expr(first, meter)?;
        for item in rest {
            combined = combined.merge(self.eval_explicit_expr(item, meter)?)?;
        }

        Ok(combined)
    }

    fn eval_at_events(
        &self,
        start: &Expr,
        pattern: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<ExplicitValue, EvalError> {
        let offset = self.eval_time_expr(start, meter)?;
        let mut explicit = Self::value_to_explicit(self.eval_expr_in_meter(pattern, meter)?)?;
        explicit.shift(&offset)?;
        Ok(explicit)
    }

    fn eval_seq_sections_events(
        &self,
        sections: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<ExplicitValue, EvalError> {
        let Some((first, rest)) = sections.split_first() else {
            return Err(EvalError::new(
                "`seq_sections` requires at least one section",
            ));
        };

        let mut cycle_offset = 0_i128;
        let mut combined = self.eval_section_events(first, meter, cycle_offset)?;
        cycle_offset = cycle_offset
            .checked_add(self.eval_section_length(first, meter)?)
            .ok_or_else(|| EvalError::new("section cycle offset overflowed"))?;

        for section in rest {
            combined = combined.merge(self.eval_section_events(section, meter, cycle_offset)?)?;
            cycle_offset = cycle_offset
                .checked_add(self.eval_section_length(section, meter)?)
                .ok_or_else(|| EvalError::new("section cycle offset overflowed"))?;
        }

        Ok(combined)
    }

    fn eval_section_events(
        &self,
        section: &Expr,
        meter: Option<&MeterContext>,
        cycle_offset: i128,
    ) -> Result<ExplicitValue, EvalError> {
        let Expr::Section { pattern, cycles } = section else {
            return Err(EvalError::new(
                "`seq_sections` only accepts `section(pattern, cycles)` items",
            ));
        };

        let repeat_count = self.eval_positive_integer(cycles, meter, "section cycle count")?;
        if repeat_count > 1024 {
            return Err(EvalError::new(
                "section cycle count exceeded the maximum allowed bound of 1024",
            ));
        }

        let base = Self::value_to_explicit(self.eval_expr_in_meter(pattern, meter)?)?;
        let mut combined: Option<ExplicitValue> = None;

        for repeat in 0..repeat_count {
            let offset = rational_from_parts(
                cycle_offset
                    .checked_add(repeat)
                    .ok_or_else(|| EvalError::new("section cycle offset overflowed"))?,
                1,
            )?;
            let mut repeated = base.clone();
            repeated.shift(&offset)?;
            combined = Some(match combined {
                Some(existing) => existing.merge(repeated)?,
                None => repeated,
            });
        }

        combined.ok_or_else(|| EvalError::new("section cycle count must be positive"))
    }

    fn eval_section_length(
        &self,
        section: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<i128, EvalError> {
        let Expr::Section { cycles, .. } = section else {
            return Err(EvalError::new(
                "`seq_sections` only accepts `section(pattern, cycles)` items",
            ));
        };

        let count = self.eval_positive_integer(cycles, meter, "section cycle count")?;
        if count > 1024 {
            return Err(EvalError::new(
                "section cycle count exceeded the maximum allowed bound of 1024",
            ));
        }

        Ok(count)
    }

    fn eval_meter_context(
        &self,
        beats: &Expr,
        unit: &Expr,
        outer_meter: Option<&MeterContext>,
    ) -> Result<MeterContext, EvalError> {
        let beats_per_cycle = self.eval_positive_integer(beats, outer_meter, "meter beat count")?;
        let _beat_unit = self.eval_positive_integer(unit, outer_meter, "meter beat unit")?;
        Ok(MeterContext { beats_per_cycle })
    }

    fn eval_time_expr(
        &self,
        expr: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Rational, EvalError> {
        match expr {
            Expr::Number(value) => f64_to_rational(*value, "time expression"),
            Expr::Beat(value) => {
                let meter = meter.ok_or_else(|| {
                    EvalError::new("`beat(...)` requires an enclosing `meter(...)`")
                })?;
                let beat_index = self.eval_time_expr(value, meter.into())?;
                let beat_length = rational_from_parts(1, meter.beats_per_cycle)?;
                beat_index
                    .checked_mul(&beat_length)
                    .map_err(|error| EvalError::new(error.to_string()))
            }
            _ => extract_constant_number_rational(
                self.eval_expr_in_meter(expr, meter)?,
                "time expression",
            ),
        }
    }

    fn eval_positive_integer(
        &self,
        expr: &Expr,
        meter: Option<&MeterContext>,
        context: &str,
    ) -> Result<i128, EvalError> {
        let value = extract_constant_number_value(self.eval_expr_in_meter(expr, meter)?, context)?;
        if !value.is_finite() || value <= 0.0 || value.fract().abs() > f64::EPSILON {
            return Err(EvalError::new(format!(
                "{context} must be a positive integer"
            )));
        }

        format!("{value:.0}")
            .parse::<i128>()
            .map_err(|_| EvalError::new(format!("{context} exceeded the supported range")))
    }

    fn value_to_explicit(value: Value) -> Result<ExplicitValue, EvalError> {
        match value {
            Value::SamplePattern(pattern) => {
                Ok(ExplicitValue::Sample(pattern.try_query(&TimeSpan::unit())?))
            }
            Value::NumberPattern(pattern) => {
                Ok(ExplicitValue::Number(pattern.try_query(&TimeSpan::unit())?))
            }
            Value::Function(_) => Err(EvalError::new(
                "functions cannot be materialized into explicit-time event streams",
            )),
            Value::String(_) => Err(EvalError::new(
                "strings cannot be materialized into explicit-time event streams",
            )),
        }
    }

    fn apply_value(
        callee: Value,
        args: Vec<Value>,
        site_salt: Option<u64>,
    ) -> Result<Value, EvalError> {
        match callee {
            Value::Function(function) => {
                let function = match site_salt {
                    Some(site_salt) if function.site_salt.is_none() => {
                        function.with_site_salt(site_salt)
                    }
                    Some(_) | None => function,
                };
                function.apply(args)
            }
            Value::SamplePattern(_) | Value::NumberPattern(_) | Value::String(_) => Err(
                EvalError::new(format!("cannot call a {}", callee.kind_name())),
            ),
        }
    }

    fn expr_site_salt(&self, expr: &Expr) -> Option<u64> {
        self.expr_site_salts.get(&expr_key(expr)).copied()
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
                "unresolved identifier `{name}` in loose mode; placeholder playback is not implemented in Task 5"
            ))),
            ReplMode::Strict => Err(EvalError::new(format!("unresolved identifier `{name}`"))),
        }
    }

    fn unsupported_pattern_item_error(items: &[Expr], context: &str) -> Option<EvalError> {
        items.iter().find_map(|item| match item {
            Expr::Call { callee, args } => {
                let name = match callee.as_ref() {
                    Expr::Ident(name) => name.as_str(),
                    _ => "call",
                };
                if name == "sample" && args.len() == 1 {
                    None
                } else {
                    Some(EvalError::new(format!(
                        "function call `{name}` cannot appear inside a pattern {context} in Task 5; apply transforms with the pipe operator `|>` or call `{name}(..., pattern)` directly"
                    )))
                }
            }
            Expr::Ident(name) if matches!(builtin_value(name), Some(Value::Function(_))) => {
                Some(EvalError::new(format!(
                    "function `{name}` cannot appear inside a pattern {context} in Task 5; apply transforms with the pipe operator `|>` or call `{name}(..., pattern)` directly"
                )))
            }
            Expr::Stream(_)
            | Expr::At { .. }
            | Expr::Meter { .. }
            | Expr::Beat(_)
            | Expr::Section { .. }
            | Expr::SeqSections(_) => Some(EvalError::new(format!(
                "explicit-time forms cannot appear inside a pattern {context}; use `stream(...)` or lift the form outside the {context}"
            ))),
            Expr::Group(group_items) => {
                Self::unsupported_pattern_item_error(group_items, context)
            }
            _ => None,
        })
    }

    fn collect_sample_nodes(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Option<Vec<PatternNode<SampleEvent>>>, EvalError> {
        let mut nodes = Vec::with_capacity(items.len());
        for item in items {
            let Some(node) = self.try_sample_node(item, meter)? else {
                return Ok(None);
            };
            nodes.push(node);
        }

        Ok(Some(nodes))
    }

    fn try_sample_node(
        &self,
        expr: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Option<PatternNode<SampleEvent>>, EvalError> {
        match expr {
            Expr::Ident(name) if is_sample_identifier(name) => {
                Ok(Some(PatternNode::atom(SampleEvent::named(name))))
            }
            Expr::Call { callee, args } if matches!(callee.as_ref(), Expr::Ident(name) if name == "sample") =>
            {
                let [arg] = args.as_slice() else {
                    return Err(EvalError::new("`sample` requires exactly one argument"));
                };
                let sample = extract_string_value(
                    self.eval_expr_in_meter(arg, meter)?,
                    "`sample` requires a string argument",
                )?;
                Ok(Some(PatternNode::atom(SampleEvent::named(&sample))))
            }
            Expr::Rest => Ok(Some(PatternNode::rest())),
            Expr::Group(items) => {
                let Some(nodes) = self.collect_sample_nodes(items, meter)? else {
                    return Ok(None);
                };
                Ok(Some(PatternNode::group(nodes)))
            }
            _ => Ok(None),
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
            _ => Ok(None),
        }
    }
}

const SITE_SEED_ROOT: u64 = 0xC6A4_A793_5BD1_E995;
const ROLE_STATEMENT: u64 = 0x01;
const ROLE_SEQ_ITEM: u64 = 0x02;
const ROLE_STACK_LAYER: u64 = 0x03;
const ROLE_STREAM_ITEM: u64 = 0x04;
const ROLE_PIPE_LHS: u64 = 0x05;
const ROLE_PIPE_RHS: u64 = 0x06;
const ROLE_CALL_CALLEE: u64 = 0x07;
const ROLE_CALL_ARG: u64 = 0x08;
const ROLE_AT_START: u64 = 0x09;
const ROLE_AT_PATTERN: u64 = 0x0A;
const ROLE_METER_BEATS: u64 = 0x0B;
const ROLE_METER_UNIT: u64 = 0x0C;
const ROLE_METER_PATTERN: u64 = 0x0D;
const ROLE_BEAT_VALUE: u64 = 0x0E;
const ROLE_SECTION_PATTERN: u64 = 0x0F;
const ROLE_SECTION_CYCLES: u64 = 0x10;
const ROLE_SEQ_SECTION_ITEM: u64 = 0x11;
const ROLE_GROUP_ITEM: u64 = 0x12;

fn collect_expr_site_salts(module: &Module) -> BTreeMap<usize, u64> {
    let mut salts = BTreeMap::new();
    for (index, statement) in module.statements.iter().enumerate() {
        match statement {
            Stmt::Binding { expr, .. } => {
                let seed = derive_site_seed(SITE_SEED_ROOT, ROLE_STATEMENT, index as u64);
                record_expr_site_salts(expr, seed, &mut salts);
            }
        }
    }
    salts
}

fn record_expr_site_salts(expr: &Expr, seed: u64, salts: &mut BTreeMap<usize, u64>) {
    salts.insert(expr_key(expr), seed);
    match expr {
        Expr::Seq(items) => record_expr_list(items, seed, ROLE_SEQ_ITEM, salts),
        Expr::Stack(layers) => record_expr_list(layers, seed, ROLE_STACK_LAYER, salts),
        Expr::Stream(items) => record_expr_list(items, seed, ROLE_STREAM_ITEM, salts),
        Expr::Pipe { lhs, rhs } => {
            record_expr_site_salts(lhs, derive_site_seed(seed, ROLE_PIPE_LHS, 0), salts);
            record_expr_site_salts(rhs, derive_site_seed(seed, ROLE_PIPE_RHS, 0), salts);
        }
        Expr::Call { callee, args } => {
            record_expr_site_salts(callee, derive_site_seed(seed, ROLE_CALL_CALLEE, 0), salts);
            record_expr_list(args, seed, ROLE_CALL_ARG, salts);
        }
        Expr::At { start, pattern } => {
            record_expr_site_salts(start, derive_site_seed(seed, ROLE_AT_START, 0), salts);
            record_expr_site_salts(pattern, derive_site_seed(seed, ROLE_AT_PATTERN, 0), salts);
        }
        Expr::Meter {
            beats,
            unit,
            pattern,
        } => {
            record_expr_site_salts(beats, derive_site_seed(seed, ROLE_METER_BEATS, 0), salts);
            record_expr_site_salts(unit, derive_site_seed(seed, ROLE_METER_UNIT, 0), salts);
            record_expr_site_salts(
                pattern,
                derive_site_seed(seed, ROLE_METER_PATTERN, 0),
                salts,
            );
        }
        Expr::Beat(value) => {
            record_expr_site_salts(value, derive_site_seed(seed, ROLE_BEAT_VALUE, 0), salts);
        }
        Expr::Section { pattern, cycles } => {
            record_expr_site_salts(
                pattern,
                derive_site_seed(seed, ROLE_SECTION_PATTERN, 0),
                salts,
            );
            record_expr_site_salts(
                cycles,
                derive_site_seed(seed, ROLE_SECTION_CYCLES, 0),
                salts,
            );
        }
        Expr::SeqSections(sections) => {
            record_expr_list(sections, seed, ROLE_SEQ_SECTION_ITEM, salts);
        }
        Expr::Group(items) => record_expr_list(items, seed, ROLE_GROUP_ITEM, salts),
        Expr::Ident(_) | Expr::Rest | Expr::Number(_) | Expr::String(_) => {}
    }
}

fn record_expr_list(items: &[Expr], seed: u64, role: u64, salts: &mut BTreeMap<usize, u64>) {
    for (index, item) in items.iter().enumerate() {
        record_expr_site_salts(item, derive_site_seed(seed, role, index as u64), salts);
    }
}

const fn derive_site_seed(base: u64, role: u64, ordinal: u64) -> u64 {
    let mut state = base
        ^ role.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ ordinal.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = state.wrapping_add(0x94D0_49BB_1331_11EB);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}

fn expr_key(expr: &Expr) -> usize {
    std::ptr::from_ref(expr) as usize
}

fn extract_constant_number_value(value: Value, context: &str) -> Result<f64, EvalError> {
    match value {
        Value::NumberPattern(pattern) => pattern.constant_value(),
        Value::SamplePattern(_) | Value::Function(_) | Value::String(_) => Err(EvalError::new(
            format!("{context} must resolve to a constant number"),
        )),
    }
}

fn extract_string_value(value: Value, message: &str) -> Result<String, EvalError> {
    match value {
        Value::String(string) => Ok(string),
        Value::SamplePattern(_) | Value::NumberPattern(_) | Value::Function(_) => {
            Err(EvalError::new(message))
        }
    }
}

fn extract_constant_number_rational(value: Value, context: &str) -> Result<Rational, EvalError> {
    let constant = extract_constant_number_value(value, context)?;
    f64_to_rational(constant, context)
}

/// Converts a 64-bit floating point number into an exact rational number representation.
///
/// This avoids floating-point precision drift during continuous time evaluation.
///
/// # Examples
///
/// ```
/// use orpheus_lang::f64_to_rational;
///
/// let r = f64_to_rational(1.5, "test").unwrap();
/// assert_eq!(r.numerator(), 3);
/// assert_eq!(r.denominator(), 2);
/// ```
///
/// # Errors
///
/// Returns an [`EvalError`] if the float is not finite, uses scientific notation, or cannot be parsed.
pub fn f64_to_rational(value: f64, context: &str) -> Result<Rational, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::new(format!("{context} must be finite")));
    }

    let rendered = value.to_string();
    if rendered.contains('e') || rendered.contains('E') {
        return Err(EvalError::new(format!(
            "{context} must not use scientific notation in Task 12"
        )));
    }

    let (negative, digits) = rendered
        .strip_prefix('-')
        .map_or((false, rendered.as_str()), |rest| (true, rest));

    let (numerator, denominator) = if let Some((whole, fractional)) = digits.split_once('.') {
        let scale = checked_pow10(fractional.len())?;
        let combined = format!("{whole}{fractional}");
        let numerator = combined
            .parse::<i128>()
            .map_err(|_| EvalError::new(format!("{context} exceeded the supported range")))?;
        (numerator, scale)
    } else {
        let numerator = digits
            .parse::<i128>()
            .map_err(|_| EvalError::new(format!("{context} exceeded the supported range")))?;
        (numerator, 1_i128)
    };

    let signed_numerator = if negative { -numerator } else { numerator };
    rational_from_parts(signed_numerator, denominator)
}

fn checked_pow10(exponent: usize) -> Result<i128, EvalError> {
    let mut value = 1_i128;
    for _ in 0..exponent {
        value = value
            .checked_mul(10)
            .ok_or_else(|| EvalError::new("decimal literal exceeded the supported range"))?;
    }
    Ok(value)
}

fn shift_events<T>(events: &mut [Event<T>], offset: &Rational) -> Result<(), EvalError> {
    for event in &mut *events {
        event.part = shift_span(&event.part, offset)?;
        if let Some(whole) = event.whole.take() {
            event.whole = Some(shift_span(&whole, offset)?);
        }
    }

    sort_events(events);
    Ok(())
}

fn shift_span(span: &TimeSpan, offset: &Rational) -> Result<TimeSpan, EvalError> {
    build_span(
        rational_add(span.start(), offset)?,
        rational_add(span.end(), offset)?,
    )
}

fn sort_events<T>(events: &mut [Event<T>]) {
    events.sort_by(|left, right| {
        left.part
            .start()
            .cmp(right.part.start())
            .then(left.part.end().cmp(right.part.end()))
    });
}

/// Creates a `TimeSpan` spanning from cycle 0 to the specified `cycle_count`.
///
/// This specifies a half-open time interval `[0, cycle_count)`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::render_span;
///
/// let span = render_span(4).unwrap();
/// assert_eq!(span.start().numerator(), 0);
/// assert_eq!(span.end().numerator(), 4);
/// ```
///
/// # Errors
///
/// Returns an [`EvalError`] if constructing the underlying rational span fails,
/// which may occur if the `cycle_count` exceeds the representable range.
pub fn render_span(cycle_count: u64) -> Result<TimeSpan, EvalError> {
    build_span(
        Rational::zero(),
        rational_from_parts(i128::from(cycle_count), 1)?,
    )
}

fn build_span(start: Rational, end: Rational) -> Result<TimeSpan, EvalError> {
    TimeSpan::new(start, end).map_err(|error| EvalError::new(error.to_string()))
}

fn rational_add(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    left.checked_add(right)
        .map_err(|error| EvalError::new(error.to_string()))
}

fn rational_from_parts(numerator: i128, denominator: i128) -> Result<Rational, EvalError> {
    Rational::checked_from_parts(numerator, denominator)
        .map_err(|error| EvalError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{Evaluator, ReplMode, eval_module, parse_module, render_span};
    use crate::value::{SampleEvent, Value, sometimes_applies_on_cycle};
    use orpheus_pattern::{Event, Rational};

    fn sample_events_for_span(
        value: &Value,
        cycle_count: u64,
    ) -> Result<Vec<Event<SampleEvent>>, super::EvalError> {
        value
            .as_sample_pattern()
            .unwrap()
            .try_query(&render_span(cycle_count)?)
    }

    fn sample_names_in_cycle(events: &[Event<SampleEvent>], cycle: i128) -> Vec<String> {
        let cycle_start = Rational::checked_from_parts(cycle, 1).unwrap();
        let cycle_end = Rational::checked_from_parts(cycle + 1, 1).unwrap();
        events
            .iter()
            .filter(|event| event.part.start() >= &cycle_start && event.part.end() <= &cycle_end)
            .map(|event| event.value.sample().to_owned())
            .collect()
    }

    fn find_call_expr_site_salt(source: &str, binding_name: &str) -> u64 {
        let parsed = parse_module(source).unwrap();
        let evaluator = Evaluator::new(ReplMode::Loose, &parsed);
        let expr = parsed
            .statements
            .iter()
            .find_map(|statement| match statement {
                crate::Stmt::Binding { name, expr } if name == binding_name => Some(expr),
                crate::Stmt::Binding { .. } => None,
            })
            .unwrap();
        evaluator.expr_site_salt(expr).unwrap()
    }

    #[test]
    fn separate_sometimes_call_sites_are_salted_independently() {
        let source = "\
left = sometimes(rev, bd sn)
right = sometimes(fast(2), cp hh)";
        let left_salt = find_call_expr_site_salt(source, "left");
        let right_salt = find_call_expr_site_salt(source, "right");
        let cycle = (0_i128..64)
            .find(|cycle| {
                sometimes_applies_on_cycle(*cycle, left_salt)
                    != sometimes_applies_on_cycle(*cycle, right_salt)
            })
            .expect("expected separate call sites to diverge on some cycle");
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let span_cycles = u64::try_from(cycle + 1).unwrap();
        let left_events = sample_events_for_span(module.get("left").unwrap(), span_cycles).unwrap();
        let right_events =
            sample_events_for_span(module.get("right").unwrap(), span_cycles).unwrap();
        let left_names = sample_names_in_cycle(&left_events, cycle);
        let right_names = sample_names_in_cycle(&right_events, cycle);

        assert_ne!(left_salt, right_salt);
        assert_eq!(
            left_names,
            if sometimes_applies_on_cycle(cycle, left_salt) {
                vec!["sn".to_owned(), "bd".to_owned()]
            } else {
                vec!["bd".to_owned(), "sn".to_owned()]
            }
        );
        assert_eq!(
            right_names,
            if sometimes_applies_on_cycle(cycle, right_salt) {
                vec![
                    "cp".to_owned(),
                    "hh".to_owned(),
                    "cp".to_owned(),
                    "hh".to_owned(),
                ]
            } else {
                vec!["cp".to_owned(), "hh".to_owned()]
            }
        );
        assert_ne!(
            sometimes_applies_on_cycle(cycle, left_salt),
            sometimes_applies_on_cycle(cycle, right_salt)
        );
    }

    #[test]
    fn eval_error_formats_its_message() {
        let err = super::EvalError::new("syntax error");
        assert_eq!(err.to_string(), "syntax error");
    }

    #[test]
    fn render_error_formats_eval_error() {
        let err = crate::RenderError::Eval(super::EvalError::new("render failed"));
        assert_eq!(err.to_string(), "render failed");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn render_error_formats_audio_error() {
        let err = crate::RenderError::Audio(orpheus_dsp::OfflineRenderError::InvalidCycleCount);
        assert_eq!(
            err.to_string(),
            "offline rendering requires at least one cycle"
        );
        assert!(std::error::Error::source(&err).is_some());
    }
}
