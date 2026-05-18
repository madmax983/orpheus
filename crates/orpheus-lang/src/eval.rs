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
use std::sync::Arc;

use orpheus_pattern::{Event, PatternNode, Rational, TimeSpan};

use crate::ReplMode;
use crate::ast::{Expr, Module, Stmt, binding_expr_self_references};
use crate::builtins::{builtin_value, is_sample_identifier, stack_values};
use crate::parser::parse_module;
use crate::pedal::compile_graph;
use crate::pitch::parse_named_pitch_literal;
use crate::value::{
    FunctionValue, NumberPatternValue, SampleEvent, SamplePatternValue, UserFn, Value,
};

/// Runtime evaluation error for bootstrap Orpheus modules.
///
/// `EvalError` occurs when an expression fails to evaluate at runtime.
/// In Orpheus, evaluation errors often stem from invalid arithmetic on rational
/// time domains (like dividing by zero), out-of-bounds parameters, or attempting
/// to use an unsupported operation on a pattern. Orpheus patterns operate in an
/// exact, bounded rational time domain, so overflows during shifts or scaling
/// can result in an `EvalError`.
///
/// **Recovery:** Since `EvalError` wraps various specific errors (like `ParseError` or `TypeError`),
/// you should match on its variants or display its `Display` implementation to locate the exact syntax issue or runtime flaw.
pub use crate::error::EvalError;

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
///
/// **Recovery:** Catch the error and print its message to the user. Errors are
/// designed to be human-readable and pinpoint syntax or runtime issues (like missing variables).
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
///
/// **Recovery:** Catch the error and display it in the REPL. The internal `bindings` environment
/// remains untouched and can be reused for subsequent evaluations without corruption.
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
    depth: std::cell::Cell<usize>,
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

    fn merge(mut self, other: Self) -> Result<Self, EvalError> {
        self.append_unsorted(other)?;
        self.sort();
        Ok(self)
    }

    fn append_unsorted(&mut self, mut other: Self) -> Result<(), EvalError> {
        match (self, &mut other) {
            (Self::Sample(left), Self::Sample(right)) => {
                left.append(right);
                Ok(())
            }
            (Self::Number(left), Self::Number(right)) => {
                left.append(right);
                Ok(())
            }
            (Self::Sample(_), Self::Number(_)) | (Self::Number(_), Self::Sample(_)) => Err(
                EvalError::new("explicit-time items must all resolve to the same pattern kind"),
            ),
        }
    }

    fn append_unsorted_shifted(&mut self, base: &Self, offset: &Rational) -> Result<(), EvalError> {
        match (self, base) {
            (Self::Sample(combined), Self::Sample(base_events)) => {
                Self::append_shifted_events(combined, base_events, offset)
            }
            (Self::Number(combined), Self::Number(base_events)) => {
                Self::append_shifted_events(combined, base_events, offset)
            }
            (Self::Sample(_), Self::Number(_)) | (Self::Number(_), Self::Sample(_)) => Err(
                EvalError::new("explicit-time items must all resolve to the same pattern kind"),
            ),
        }
    }

    fn append_shifted_events<T: Clone>(
        combined: &mut Vec<Event<T>>,
        base_events: &[Event<T>],
        offset: &Rational,
    ) -> Result<(), EvalError> {
        // ⚡ Bolt: Pre-allocate capacity to eliminate redundant heap allocations when appending events.
        combined.reserve(base_events.len());
        for event in base_events {
            let shifted_part = shift_span(&event.part, offset)?;
            let shifted_whole = if let Some(whole) = &event.whole {
                Some(shift_span(whole, offset)?)
            } else {
                None
            };

            // ⚡ Bolt: Avoid redundant allocations when shifting events.
            // By instantiating a new Event directly and only cloning `event.value`,
            // we eliminate unnecessary cloning of `TimeSpan` fields (`part` and `whole`)
            // that are immediately overwritten anyway.
            combined.push(Event {
                whole: shifted_whole,
                part: shifted_part,
                value: event.value.clone(),
            });
        }
        Ok(())
    }

    fn sort(&mut self) {
        match self {
            Self::Sample(events) => sort_events(events),
            Self::Number(events) => sort_events(events),
        }
    }

    fn shift(&mut self, offset: &Rational) -> Result<(), EvalError> {
        match self {
            Self::Sample(events) => shift_events(events, offset),
            Self::Number(events) => shift_events(events, offset),
        }
    }

    fn empty_with_capacity_matching(&self, multiplier: usize) -> Result<Self, EvalError> {
        let current_len = match self {
            Self::Sample(events) => events.len(),
            Self::Number(events) => events.len(),
        };

        let capacity = current_len
            .checked_mul(multiplier)
            .ok_or_else(|| EvalError::new("section pattern capacity overflowed"))?;

        if capacity > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }

        match self {
            Self::Sample(_) => Ok(Self::Sample(Vec::with_capacity(capacity))),
            Self::Number(_) => Ok(Self::Number(Vec::with_capacity(capacity))),
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
            depth: std::cell::Cell::new(0),
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
        statements
            .iter()
            .try_fold(None, |_, statement| match statement {
                Stmt::Binding {
                    name, params, expr, ..
                } => {
                    if !params.is_empty() && binding_expr_self_references(name, params, expr) {
                        return Err(EvalError::new(format!(
                            "parameterized binding `{name}` cannot contain a self-reference in v1"
                        )));
                    }
                    let value = if params.is_empty() {
                        self.eval_expr(expr)?
                    } else {
                        Value::Function(FunctionValue::User(Arc::new(UserFn {
                            mode: self.mode,
                            remaining_params: params.clone(),
                            body: expr.clone(),
                            captured_bindings: self.bindings.clone(),
                            expr_site_salts: self.expr_site_salts.clone(),
                            depth: self.depth.get(),
                        })))
                    };
                    self.bindings.insert(name.clone(), value.clone());
                    Ok(Some((name.clone(), value)))
                }
            })
    }

    fn eval_expr(&self, expr: &Expr) -> Result<Value, EvalError> {
        self.eval_expr_in_meter(expr, None)
    }

    fn eval_expr_in_meter(
        &self,
        expr: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        if self.depth.get() > 200 {
            return Err(EvalError::new("evaluation recursion limit exceeded"));
        }
        self.depth.set(self.depth.get() + 1);
        let result = self.eval_expr_in_meter_impl(expr, meter);
        self.depth.set(self.depth.get() - 1);
        result
    }
    fn eval_expr_in_meter_impl(
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
            Expr::String(value) => Ok(Value::String(value.clone().into())),
            Expr::Graph { bindings, result } => compile_graph(bindings, result).map(Value::Pedal),
            Expr::Binary { .. } => Err(EvalError::new(
                "binary pedal expressions are parsed but not yet executable in evaluation",
            )),
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
        // ⚡ Bolt: Removed intermediate allocation and `.collect()` by pushing directly into pre-allocated `Vec`
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
        if let Expr::Call { callee, args } = rhs {
            self.eval_call_with_args(rhs, callee, args, Some(lhs_value), meter)
        } else {
            self.apply_value(
                self.eval_expr_in_meter(rhs, meter)?,
                vec![lhs_value],
                self.expr_site_salt(rhs),
            )
        }
    }

    fn eval_call(
        &self,
        call_expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        self.eval_call_with_args(call_expr, callee, args, None, meter)
    }

    fn eval_call_with_args(
        &self,
        call_expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        piped_arg: Option<Value>,
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        let callee_value = self.eval_expr_in_meter(callee, meter)?;
        let mut evaluated_args = Vec::with_capacity(args.len() + usize::from(piped_arg.is_some()));
        for arg in args {
            evaluated_args.push(self.eval_expr_in_meter(arg, meter)?);
        }
        if let Some(piped) = piped_arg {
            evaluated_args.push(piped);
        }
        self.apply_value(callee_value, evaluated_args, self.expr_site_salt(call_expr))
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
            } => self.eval_nested_meter(beats, unit, pattern, meter),
            Expr::SeqSections(sections) => self.eval_seq_sections_events(sections, meter),
            Expr::Section { .. } => Err(EvalError::new(
                "`section(...)` can only appear inside `seq_sections(...)`",
            )),
            _ => Self::value_to_explicit(self.eval_expr_in_meter(expr, meter)?),
        }
    }

    fn eval_nested_meter(
        &self,
        beats: &Expr,
        unit: &Expr,
        pattern: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<ExplicitValue, EvalError> {
        let nested_meter = self.eval_meter_context(beats, unit, meter)?;
        self.eval_explicit_expr(pattern, Some(&nested_meter))
    }

    fn collect_explicit_items(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<ExplicitValue, EvalError> {
        let Some((first, rest)) = items.split_first() else {
            return Err(EvalError::new("`stream` requires at least one item"));
        };

        rest.iter()
            .try_fold(self.eval_explicit_expr(first, meter)?, |acc, item| {
                acc.merge(self.eval_explicit_expr(item, meter)?)
            })
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

        let initial_offset = 0_i128;
        let initial_combined = self.eval_section_events(first, meter, initial_offset)?;
        let initial_length = self.eval_section_length(first, meter)?;
        let next_offset = initial_offset
            .checked_add(initial_length)
            .ok_or_else(|| EvalError::new("section cycle offset overflowed"))?;

        let (combined, _) = rest.iter().try_fold(
            (initial_combined, next_offset),
            |(acc_combined, acc_offset), section| -> Result<(ExplicitValue, i128), EvalError> {
                let section_events = self.eval_section_events(section, meter, acc_offset)?;
                let section_length = self.eval_section_length(section, meter)?;

                let next_combined = acc_combined.merge(section_events)?;
                let next_offset = acc_offset
                    .checked_add(section_length)
                    .ok_or_else(|| EvalError::new("section cycle offset overflowed"))?;

                Ok((next_combined, next_offset))
            },
        )?;

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
        if repeat_count == 0 {
            return Err(EvalError::new("section cycle count must be positive"));
        }

        Self::repeat_explicit_value(base, repeat_count, cycle_offset)
    }

    fn repeat_explicit_value(
        base: ExplicitValue,
        repeat_count: i128,
        cycle_offset: i128,
    ) -> Result<ExplicitValue, EvalError> {
        let repeat_count_usize = usize::try_from(repeat_count)
            .map_err(|_| EvalError::new("section cycle count exceeded evaluator limits"))?;

        let mut combined = base.empty_with_capacity_matching(repeat_count_usize)?;

        for repeat in 0..repeat_count {
            let offset = rational_from_parts(
                cycle_offset
                    .checked_add(repeat)
                    .ok_or_else(|| EvalError::new("section cycle offset overflowed"))?,
                1,
            )?;
            if repeat == repeat_count - 1 {
                // ⚡ Bolt: Eliminate redundant allocation on the last section cycle repeat.
                let mut final_repeated = base;
                final_repeated.shift(&offset)?;
                combined.append_unsorted(final_repeated)?;
                break;
            }
            combined.append_unsorted_shifted(&base, &offset)?;
        }

        combined.sort();
        Ok(combined)
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
            Expr::Beat(value) => self.eval_beat_expr(value, meter),
            _ => extract_constant_number_rational(
                self.eval_expr_in_meter(expr, meter)?,
                "time expression",
            ),
        }
    }

    fn eval_beat_expr(
        &self,
        value: &Expr,
        meter: Option<&MeterContext>,
    ) -> Result<Rational, EvalError> {
        let meter = meter
            .ok_or_else(|| EvalError::new("`beat(...)` requires an enclosing `meter(...)`"))?;
        let beat_index = self.eval_time_expr(value, meter.into())?;
        let beat_length = rational_from_parts(1, meter.beats_per_cycle)?;
        Ok(beat_index.checked_mul(&beat_length)?)
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

        if value.is_nan() {
            return Err(EvalError::new(format!("{context} requires a valid number")));
        }

        #[allow(clippy::cast_precision_loss)]
        let max_val = i128::MAX as f64;
        #[allow(clippy::cast_precision_loss)]
        let min_val = i128::MIN as f64;
        if value > max_val || value < min_val {
            return Err(EvalError::new(format!(
                "{context} exceeded the supported range"
            )));
        }

        #[allow(clippy::cast_possible_truncation)]
        let parsed = value.round() as i128;

        if parsed <= 0 {
            return Err(EvalError::new(format!(
                "{context} must be a positive integer"
            )));
        }

        Ok(parsed)
    }

    fn value_to_explicit(value: Value) -> Result<ExplicitValue, EvalError> {
        match value {
            Value::SamplePattern(pattern) => {
                Ok(ExplicitValue::Sample(pattern.try_query(&TimeSpan::unit())?))
            }
            Value::NumberPattern(pattern) => {
                Ok(ExplicitValue::Number(pattern.try_query(&TimeSpan::unit())?))
            }
            _ => Err(EvalError::new(format!(
                "{}s cannot be materialized into explicit-time event streams",
                value.kind_name()
            ))),
        }
    }

    fn apply_value(
        &self,
        callee: Value,
        args: Vec<Value>,
        site_salt: Option<u64>,
    ) -> Result<Value, EvalError> {
        let Value::Function(mut function) = callee else {
            return Err(EvalError::new(format!(
                "cannot call a {}",
                callee.kind_name()
            )));
        };

        if let FunctionValue::Builtin(builtin_func) = function {
            let salted_func = Self::apply_site_salt_to_builtin(builtin_func, site_salt);
            function = FunctionValue::Builtin(salted_func);
        } else if let FunctionValue::User(ref mut user_fn) = function {
            std::sync::Arc::make_mut(user_fn).depth = self.depth.get();
        }

        apply_function_value(function, args)
    }

    const fn apply_site_salt_to_builtin(
        function: crate::value::BuiltinFn,
        site_salt: Option<u64>,
    ) -> crate::value::BuiltinFn {
        if let Some(salt) = site_salt
            && function.site_salt.is_none()
        {
            return function.with_site_salt(salt);
        }
        function
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

        if let Some(semitones) = parse_named_pitch_literal(name)? {
            return Ok(Value::NumberPattern(NumberPatternValue::constant(
                f64::from(semitones),
            )));
        }

        match self.mode {
            ReplMode::Loose => Err(EvalError::new(format!(
                "unresolved identifier `{name}` in loose mode; placeholder playback is not implemented in Task 5"
            ))),
            ReplMode::Strict => Err(EvalError::new(format!("unresolved identifier `{name}`"))),
        }
    }

    fn unsupported_pattern_item_error(items: &[Expr], context: &str) -> Option<EvalError> {
        items
            .iter()
            .find_map(|item| Self::check_unsupported_pattern_item(item, context))
    }

    fn check_unsupported_pattern_item(item: &Expr, context: &str) -> Option<EvalError> {
        match item {
            Expr::Call { callee, args } => Self::check_unsupported_call(callee, args, context),
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
            Expr::Group(group_items) => Self::unsupported_pattern_item_error(group_items, context),
            _ => None,
        }
    }

    fn check_unsupported_call(callee: &Expr, args: &[Expr], context: &str) -> Option<EvalError> {
        let name = if let Expr::Ident(name) = callee {
            name.as_str()
        } else {
            "call"
        };

        if name == "sample" && args.len() == 1 {
            None
        } else {
            Some(EvalError::new(format!(
                "function call `{name}` cannot appear inside a pattern {context} in Task 5; apply transforms with the pipe operator `|>` or call `{name}(..., pattern)` directly"
            )))
        }
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
            Expr::Group(items) => self.collect_sample_group(items, meter),
            _ => Ok(None),
        }
    }

    fn collect_sample_group(
        &self,
        items: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Option<PatternNode<SampleEvent>>, EvalError> {
        let Some(nodes) = self.collect_sample_nodes(items, meter)? else {
            return Ok(None);
        };
        Ok(Some(PatternNode::group(nodes)))
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
            Expr::Ident(name) => Ok(parse_named_pitch_literal(name)?
                .map(|semitones| PatternNode::atom(f64::from(semitones)))),
            Expr::Rest => Ok(Some(PatternNode::rest())),
            Expr::Group(items) => self.collect_number_group(items),
            _ => Ok(None),
        }
    }

    fn collect_number_group(&self, items: &[Expr]) -> Result<Option<PatternNode<f64>>, EvalError> {
        let Some(nodes) = self.collect_number_nodes(items)? else {
            return Ok(None);
        };
        Ok(Some(PatternNode::group(nodes)))
    }
}

/// Applies a list of arguments to a runtime function value.
///
/// This handles both built-in primitive transformations (like `fast`) and
/// custom user-defined closures. If the number of arguments provided is
/// less than the function's arity, it returns a new, curried `FunctionValue`
/// with the provided arguments bound.
///
/// # Parameters
/// - `function`: The [`FunctionValue`] (built-in or user-defined) to invoke.
/// - `args`: The list of evaluated [`Value`]s to pass as arguments.
///
/// # Errors
///
/// Returns an [`EvalError`] if the function application results in a runtime
/// error (e.g., mismatched types during builtin execution) or if the arity
/// of user-defined functions is violated during execution.
///
/// # Examples
///
/// ```
/// use orpheus_lang::Value;
/// use orpheus_lang::{apply_function_value, builtin_value};
///
/// let fast_func = builtin_value("fast").unwrap();
/// let bd = builtin_value("bd").unwrap();
///
/// if let Value::Function(func) = fast_func {
///     // `fast` takes 2 arguments: a rate and a pattern.
///     // Here we simulate applying a single argument to a curried function
///     let curried = apply_function_value(func, vec![bd]).unwrap();
///     assert!(matches!(curried, Value::Function(_)));
/// }
/// ```
pub fn apply_function_value(function: FunctionValue, args: Vec<Value>) -> Result<Value, EvalError> {
    match function {
        FunctionValue::Builtin(function) => function.apply(args),
        FunctionValue::User(function) => apply_user_function(function, args),
    }
}

/// Applies a user-defined function to the provided arguments, executing the body if fully applied.
///
/// If the function is partially applied, this returns a new curried function with the provided arguments captured in its environment.
fn apply_user_function(
    mut function: std::sync::Arc<UserFn>,
    args: Vec<Value>,
) -> Result<Value, EvalError> {
    if function.depth > 200 {
        return Err(EvalError::new("evaluation recursion limit exceeded"));
    }
    let remaining = function.remaining_params.len();
    let applied = args.len();
    if applied > remaining {
        return Err(EvalError::new(format!(
            "function expected {remaining} argument(s), got {applied}"
        )));
    }

    let user_fn = std::sync::Arc::make_mut(&mut function);

    // ⚡ Bolt: Drain parameters directly to avoid cloning strings when binding arguments
    for (param, arg) in user_fn.remaining_params.drain(..applied).zip(args) {
        user_fn.captured_bindings.insert(param, arg);
    }

    if !user_fn.remaining_params.is_empty() {
        return Ok(Value::Function(FunctionValue::User(function)));
    }

    let owned_user_fn = std::sync::Arc::unwrap_or_clone(function);

    let evaluator = Evaluator {
        mode: owned_user_fn.mode,
        bindings: owned_user_fn.captured_bindings,
        expr_site_salts: owned_user_fn.expr_site_salts,
        depth: std::cell::Cell::new(owned_user_fn.depth + 1),
    };
    evaluator.eval_expr(&owned_user_fn.body)
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
        Expr::Pipe { lhs, rhs } => record_pipe_salts(lhs, rhs, seed, salts),
        Expr::Call { callee, args } => record_call_salts(callee, args, seed, salts),
        Expr::At { start, pattern } => record_at_salts(start, pattern, seed, salts),
        Expr::Meter {
            beats,
            unit,
            pattern,
        } => record_meter_salts(beats, unit, pattern, seed, salts),
        Expr::Beat(value) => {
            record_expr_site_salts(value, derive_site_seed(seed, ROLE_BEAT_VALUE, 0), salts);
        }
        Expr::Section { pattern, cycles } => record_section_salts(pattern, cycles, seed, salts),
        Expr::SeqSections(sections) => {
            record_expr_list(sections, seed, ROLE_SEQ_SECTION_ITEM, salts);
        }
        Expr::Group(items) => record_expr_list(items, seed, ROLE_GROUP_ITEM, salts),
        Expr::Graph { bindings, result } => {
            for (index, binding) in bindings.iter().enumerate() {
                record_expr_site_salts(
                    &binding.expr,
                    derive_site_seed(seed, ROLE_GROUP_ITEM, index as u64),
                    salts,
                );
            }
            record_expr_site_salts(result, derive_site_seed(seed, ROLE_GROUP_ITEM, 0), salts);
        }
        Expr::Binary { lhs, rhs, .. } => {
            record_expr_site_salts(lhs, derive_site_seed(seed, ROLE_GROUP_ITEM, 0), salts);
            record_expr_site_salts(rhs, derive_site_seed(seed, ROLE_GROUP_ITEM, 1), salts);
        }
        Expr::Ident(_) | Expr::Rest | Expr::Number(_) | Expr::String(_) => {}
    }
}

fn record_pipe_salts(lhs: &Expr, rhs: &Expr, seed: u64, salts: &mut BTreeMap<usize, u64>) {
    record_expr_site_salts(lhs, derive_site_seed(seed, ROLE_PIPE_LHS, 0), salts);
    record_expr_site_salts(rhs, derive_site_seed(seed, ROLE_PIPE_RHS, 0), salts);
}

fn record_call_salts(callee: &Expr, args: &[Expr], seed: u64, salts: &mut BTreeMap<usize, u64>) {
    record_expr_site_salts(callee, derive_site_seed(seed, ROLE_CALL_CALLEE, 0), salts);
    record_expr_list(args, seed, ROLE_CALL_ARG, salts);
}

fn record_at_salts(start: &Expr, pattern: &Expr, seed: u64, salts: &mut BTreeMap<usize, u64>) {
    record_expr_site_salts(start, derive_site_seed(seed, ROLE_AT_START, 0), salts);
    record_expr_site_salts(pattern, derive_site_seed(seed, ROLE_AT_PATTERN, 0), salts);
}

fn record_meter_salts(
    beats: &Expr,
    unit: &Expr,
    pattern: &Expr,
    seed: u64,
    salts: &mut BTreeMap<usize, u64>,
) {
    record_expr_site_salts(beats, derive_site_seed(seed, ROLE_METER_BEATS, 0), salts);
    record_expr_site_salts(unit, derive_site_seed(seed, ROLE_METER_UNIT, 0), salts);
    record_expr_site_salts(
        pattern,
        derive_site_seed(seed, ROLE_METER_PATTERN, 0),
        salts,
    );
}

fn record_section_salts(
    pattern: &Expr,
    cycles: &Expr,
    seed: u64,
    salts: &mut BTreeMap<usize, u64>,
) {
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
    if let Value::NumberPattern(pattern) = value {
        pattern.constant_value()
    } else {
        Err(EvalError::new(format!(
            "{context} must resolve to a constant number"
        )))
    }
}

fn extract_string_value(value: Value, message: &str) -> Result<String, EvalError> {
    if let Value::String(string) = value {
        Ok(string.to_string())
    } else {
        Err(EvalError::new(message))
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
        let combined = [whole, fractional].concat();
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

/// ⚡ Bolt: Use `sort_unstable_by` instead of `sort_by` to eliminate sorting allocation overhead,
/// as pattern events occurring at the exact same time have no inherent order to preserve.
fn sort_events<T>(events: &mut [Event<T>]) {
    events.sort_unstable_by(|left, right| {
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
    if cycle_count == 0 {
        return Err(EvalError::new("rendering requires at least one cycle"));
    }
    build_span(
        Rational::zero(),
        rational_from_parts(i128::from(cycle_count), 1)?,
    )
}

fn build_span(start: Rational, end: Rational) -> Result<TimeSpan, EvalError> {
    Ok(TimeSpan::new(start, end)?)
}

fn rational_add(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    Ok(left.checked_add(right)?)
}

fn rational_from_parts(numerator: i128, denominator: i128) -> Result<Rational, EvalError> {
    Ok(Rational::checked_from_parts(numerator, denominator)?)
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
                crate::Stmt::Binding { name, expr, .. } if name == binding_name => Some(expr),
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
    }

    #[test]
    fn render_error_formats_audio_error() {
        let err = crate::RenderError::Audio(orpheus_dsp::OfflineRenderError::InvalidCycleCount);
        assert_eq!(
            err.to_string(),
            "offline rendering requires at least one cycle"
        );
    }

    #[test]
    fn eval_error_from_parse_error() {
        let parse_err =
            crate::diagnostics::ParseError::new("parse error at line 0, col 0: test parse error");
        let err: super::EvalError = parse_err.into();
        assert_eq!(
            err.to_string(),
            "parse error at line 0, col 0: test parse error"
        );
    }

    #[test]
    fn eval_error_from_try_from_int_error() {
        let num_err: Result<u8, _> = 256u16.try_into();
        let err: super::EvalError = num_err.unwrap_err().into();
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn eval_error_from_io_error() {
        let not_found = std::io::Error::new(std::io::ErrorKind::NotFound, "oops");
        let err: super::EvalError = not_found.into();
        assert_eq!(err.to_string(), "file not found");

        let permission_denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "oops");
        let err2: super::EvalError = permission_denied.into();
        assert_eq!(err2.to_string(), "permission denied");

        let other_err = std::io::Error::other("custom error message");
        let err3: super::EvalError = other_err.into();
        assert_eq!(err3.to_string(), "custom error message");
    }

    #[test]
    fn eval_error_from_fmt_error() {
        let fmt_err = std::fmt::Error;
        let err: super::EvalError = fmt_err.into();
        assert_eq!(
            err.to_string(),
            "an error occurred when formatting an argument"
        );
    }

    #[test]
    fn eval_error_from_type_error() {
        let type_err = crate::diagnostics::TypeError::new("mock type error");
        let err: super::EvalError = type_err.into();
        assert_eq!(err.to_string(), "mock type error");
    }

    #[test]
    fn eval_error_from_load_error() {
        let load_err = crate::diagnostics::LoadError::new("mock load error");
        let err: super::EvalError = load_err.into();
        assert_eq!(err.to_string(), "mock load error");
    }

    #[test]
    fn eval_error_from_pattern_error() {
        use orpheus_pattern::PatternError;
        let pattern_err = PatternError::InvalidDenominator { denominator: 0 };
        let err: super::EvalError = pattern_err.into();
        assert_eq!(err.to_string(), "rational denominator cannot be zero");
    }

    #[test]
    fn eval_structural_pattern_mixed_types() {
        let result = eval_module("x = (bd 1)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "group items must all resolve to the same structural pattern kind"
        );
    }

    #[test]
    fn eval_structural_pattern_mixed_types_sequence() {
        let result = eval_module("x = bd 1", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "sequence items must all resolve to the same structural pattern kind"
        );
    }

    #[test]
    fn explicit_stream_mixed_types() {
        let result = eval_module("x = stream(at(0, bd), at(1, 1))", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "explicit-time items must all resolve to the same pattern kind"
        );
    }

    #[test]
    fn explicit_seq_sections_mixed_types() {
        let result = eval_module(
            "x = seq_sections(section(at(0, bd), 1), section(at(0, 1), 1))",
            ReplMode::Strict,
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "explicit-time items must all resolve to the same pattern kind"
        );
    }

    #[test]
    fn eval_apply_value_to_non_function() {
        let result = eval_module("x = 1 |> 2", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "cannot call a number pattern"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_error() {
        let result = eval_module("x = bd stream(at(0, sn))", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "explicit-time forms cannot appear inside a pattern sequence; use `stream(...)` or lift the form outside the sequence"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_group_error() {
        let result = eval_module("x = (bd stream(at(0, sn)))", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "explicit-time forms cannot appear inside a pattern group; use `stream(...)` or lift the form outside the group"
        );
    }

    #[test]
    fn eval_rest_marker_error() {
        let result = eval_module("x = ~", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "rest markers can only appear inside pattern sequences"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_error_unsupported_pattern_item_error_sample() {
        let result = eval_module("x = bd sample(\"bd\", sn)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "function call `sample` cannot appear inside a pattern sequence in Task 5; apply transforms with the pipe operator `|>` or call `sample(..., pattern)` directly"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_error_unsupported_pattern_item_error_function() {
        let result = eval_module("x = fast bd", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "function `fast` cannot appear inside a pattern sequence in Task 5; apply transforms with the pipe operator `|>` or call `fast(..., pattern)` directly"
        );
    }

    #[test]
    fn eval_explicit_expr_errors() {
        let result = eval_module("x = stream(section(1, 1))", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`section(...)` can only appear inside `seq_sections(...)`"
        );
    }

    #[test]
    fn eval_seq_sections_invalid_item() {
        let result = eval_module("x = seq_sections(1)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`seq_sections` only accepts `section(pattern, cycles)` items"
        );
    }

    #[test]
    fn eval_meter_without_beat() {
        let result = eval_module("x = beat(0)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`beat(...)` can only appear inside `at(...)` within an enclosing `meter(...)`"
        );
    }

    #[test]
    fn eval_section_standalone() {
        let result = eval_module("x = section(1, 1)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`section(...)` can only appear inside `seq_sections(...)`"
        );
    }

    #[test]
    fn explicit_stream_value_conversion() {
        let result = eval_module("x = stream(bd)", ReplMode::Strict);
        assert!(result.is_ok());
    }

    #[test]
    fn eval_meter_nested_beat_missing_meter() {
        let result = eval_module("x = at(beat(0), bd)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`beat(...)` requires an enclosing `meter(...)`"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_error_unsupported_pattern_item_error_group_nested() {
        let result = eval_module("x = (bd (stream(at(0, sn))))", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "explicit-time forms cannot appear inside a pattern group; use `stream(...)` or lift the form outside the group"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_error_unsupported_pattern_item_error_sample_group() {
        let result = eval_module("x = (bd sample(\"bd\", sn))", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "function call `sample` cannot appear inside a pattern group in Task 5; apply transforms with the pipe operator `|>` or call `sample(..., pattern)` directly"
        );
    }

    #[test]
    fn eval_explicit_to_implicit_error_unsupported_pattern_item_error_function_group() {
        let result = eval_module("x = (fast bd)", ReplMode::Strict);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "function `fast` cannot appear inside a pattern group in Task 5; apply transforms with the pipe operator `|>` or call `fast(..., pattern)` directly"
        );
    }

    #[test]
    fn eval_apply_user_function_over_application_error() {
        let result = eval_module("f x = x\nerr = f(1, 2)", ReplMode::Loose);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "function expected 1 argument(s), got 2"
        );
    }

    #[test]
    fn eval_apply_user_function_currying_success() {
        let result = eval_module("f x y = x y\npartial = f(1)", ReplMode::Loose).unwrap();
        let partial = result.get("partial").unwrap();
        assert!(matches!(
            partial,
            Value::Function(crate::value::FunctionValue::User(_))
        ));
    }

    #[test]
    fn eval_apply_user_function_too_many_args_returns_error() {
        let source = "f x = x\nresult = f(1, 2)";
        let result = eval_module(source, ReplMode::Loose);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("function expected 1 argument(s), got 2"));
    }

    #[test]
    fn eval_apply_builtin_function_too_many_args_returns_error() {
        let source = "result = fast(1, bd, sn)";
        let result = eval_module(source, ReplMode::Loose);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected 2 argument(s), got 3"));
    }

    #[test]
    fn eval_error_from_conversions() {
        let num_err: std::num::TryFromIntError = u8::try_from(256u16).unwrap_err();
        let eval_err: crate::error::EvalError = num_err.into();
        assert!(eval_err.to_string().contains("out of range"));

        let num_err: std::num::ParseIntError = "abc".parse::<i32>().unwrap_err();
        let eval_err: crate::error::EvalError = num_err.into();
        assert!(eval_err.to_string().contains("invalid digit"));

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let eval_err: crate::error::EvalError = io_err.into();
        assert_eq!(eval_err.to_string(), "file not found");

        let fmt_err = std::fmt::Error;
        let eval_err: crate::error::EvalError = fmt_err.into();
        assert_eq!(
            eval_err.to_string(),
            "an error occurred when formatting an argument"
        );

        let pat_err: orpheus_pattern::PatternError =
            orpheus_pattern::PatternError::InvalidDenominator { denominator: 0 };
        let eval_err: crate::error::EvalError = pat_err.into();
        assert_eq!(eval_err.to_string(), "rational denominator cannot be zero");
    }

    #[test]
    fn eval_error_from_parse_int_error() {
        let err: Result<i32, _> = "not_a_number".parse();
        let eval_err: super::EvalError = err.unwrap_err().into();
        assert!(eval_err.to_string().contains("invalid digit"));
    }

    #[test]
    fn eval_error_from_pitch_literal_error() {
        use crate::pitch::PitchLiteralError;
        let pitch_err = PitchLiteralError::new("invalid pitch literal".to_owned());
        let eval_err: super::EvalError = pitch_err.into();
        assert_eq!(eval_err.to_string(), "invalid pitch literal");
    }

    #[test]
    fn f64_to_rational_returns_error_on_infinite_float() {
        let err = super::f64_to_rational(f64::INFINITY, "test context").unwrap_err();
        assert!(err.to_string().contains("must be finite"));
        let err2 = super::f64_to_rational(f64::NEG_INFINITY, "test context").unwrap_err();
        assert!(err2.to_string().contains("must be finite"));
    }

    #[test]
    fn f64_to_rational_returns_error_on_nan_float() {
        let err = super::f64_to_rational(f64::NAN, "test context").unwrap_err();
        assert!(err.to_string().contains("must be finite"));
    }

    #[test]
    fn f64_to_rational_returns_error_on_exceeding_decimal_range_from_large_exponent() {
        // Rust's `to_string()` for `f64` expands large values (e.g. 1e100) out completely.
        // This causes `checked_pow10` or string length to exceed the supported precision range
        // or trigger an overflow, verifying the `checked_pow10` fallback error.
        let err = super::f64_to_rational(1.0e100, "test context").unwrap_err();
        assert!(err.to_string().contains("exceeded the supported range"));
    }

    #[test]
    fn f64_to_rational_handles_fractional_floats() {
        let r = super::f64_to_rational(0.123_456_789, "test").unwrap();
        assert_eq!(r.numerator(), 123_456_789);
        assert_eq!(r.denominator(), 1_000_000_000);
    }

    #[test]
    fn f64_to_rational_handles_negative_floats() {
        let r = super::f64_to_rational(-1.25, "test").unwrap();
        assert_eq!(r.numerator(), -5);
        assert_eq!(r.denominator(), 4);

        let r2 = super::f64_to_rational(-0.75, "test").unwrap();
        assert_eq!(r2.numerator(), -3);
        assert_eq!(r2.denominator(), 4);
    }

    #[test]
    fn f64_to_rational_handles_floats_without_fraction() {
        let r = super::f64_to_rational(42.0, "test").unwrap();
        assert_eq!(r.numerator(), 42);
        assert_eq!(r.denominator(), 1);

        let r2 = super::f64_to_rational(-7.0, "test").unwrap();
        assert_eq!(r2.numerator(), -7);
        assert_eq!(r2.denominator(), 1);
    }

    #[test]
    fn extract_string_value_handles_strings() {
        let val = crate::value::Value::String("hello".into());
        let res = super::extract_string_value(val, "error");
        assert_eq!(res.unwrap(), "hello");
    }

    #[test]
    fn extract_string_value_returns_error_on_non_string() {
        let val = crate::value::Value::NumberPattern(
            crate::value::NumberPatternValue::from_events(vec![]),
        );
        let res = super::extract_string_value(val, "expected string");
        assert_eq!(res.unwrap_err().to_string(), "expected string");
    }

    #[test]
    fn extract_constant_number_value_handles_number_pattern() {
        use orpheus_pattern::{Event, TimeSpan};
        let event = Event {
            whole: None,
            part: TimeSpan::unit(),
            value: 42.0,
        };
        let val = crate::value::Value::NumberPattern(
            crate::value::NumberPatternValue::from_events(vec![event]),
        );
        let res = super::extract_constant_number_value(val, "expected number");
        assert!((res.unwrap() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_constant_number_value_returns_error_on_non_number() {
        let val = crate::value::Value::String("hello".into());
        let res = super::extract_constant_number_value(val, "expected number");
        assert_eq!(
            res.unwrap_err().to_string(),
            "expected number must resolve to a constant number"
        );
    }

    #[test]
    fn apply_function_value_evaluates_user_function_correctly() {
        let module = eval_module("f x = x\nres = f(42.0)", ReplMode::Loose).unwrap();
        let val = module.get("res").unwrap().as_number_pattern().unwrap();
        assert!((val.try_query_unit().unwrap()[0].value - 42.0).abs() < f64::EPSILON);
    }
}

    #[test]
    fn render_span_returns_error_when_cycle_count_is_zero() {
        let err = super::render_span(0).unwrap_err();
        assert!(err.to_string().contains("rendering requires at least one cycle"));
    }
