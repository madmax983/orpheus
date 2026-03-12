use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::Path;

use orpheus_dsp::{OfflineRenderError, SampleBank, SampleTrigger, render_events_to_file_with_bank};
use orpheus_pattern::{Event, PatternNode, Rational, TimeSpan};

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

/// Error raised while rendering an Orpheus sample pattern to an audio file.
#[derive(Debug)]
pub enum RenderError {
    Eval(EvalError),
    Audio(OfflineRenderError),
}

impl Display for RenderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eval(error) => Display::fmt(error, formatter),
            Self::Audio(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Eval(error) => Some(error),
            Self::Audio(error) => Some(error),
        }
    }
}

impl From<EvalError> for RenderError {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

impl From<OfflineRenderError> for RenderError {
    fn from(error: OfflineRenderError) -> Self {
        Self::Audio(error)
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

pub fn eval_into_bindings(
    source: &str,
    mode: ReplMode,
    bindings: &mut BTreeMap<String, Value>,
) -> Result<Option<(String, Value)>, EvalError> {
    let parsed = parse_module(source)?;
    let mut evaluator = Evaluator::with_bindings(mode, bindings.clone());
    let last_binding = evaluator.eval_statements(&parsed.statements)?;
    *bindings = evaluator.bindings;
    Ok(last_binding)
}

/// Renders a sample pattern to a deterministic stereo audio file selected by
/// the target extension.
///
/// Supported extensions:
/// - `.wav`
/// - `.flac`
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target file.
pub fn render_sample_pattern_to_file(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), RenderError> {
    let sample_bank = SampleBank::load_builtin();
    render_sample_pattern_to_file_with_bank(pattern, path, cycle_count, &sample_bank)
}

/// Renders a sample pattern to a deterministic stereo audio file using the
/// supplied sample bank overrides.
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target file.
pub fn render_sample_pattern_to_file_with_bank(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
    sample_bank: &SampleBank,
) -> Result<(), RenderError> {
    if cycle_count == 0 {
        return Err(EvalError::new("rendering requires at least one cycle").into());
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let rendered_events = events
        .into_iter()
        .map(|event| Event {
            whole: event.whole,
            part: event.part,
            value: sample_trigger_from_event(&event.value),
        })
        .collect::<Vec<_>>();

    render_events_to_file_with_bank(path, &rendered_events, cycle_count, sample_bank)?;
    Ok(())
}

/// Renders a sample pattern to a deterministic stereo WAV file.
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target WAV file.
pub fn render_sample_pattern_to_wav(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), RenderError> {
    render_sample_pattern_to_file(pattern, path, cycle_count)
}

struct Evaluator {
    mode: ReplMode,
    bindings: BTreeMap<String, Value>,
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
    const fn new(mode: ReplMode) -> Self {
        Self::with_bindings(mode, BTreeMap::new())
    }

    const fn with_bindings(mode: ReplMode, bindings: BTreeMap<String, Value>) -> Self {
        Self { mode, bindings }
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
            Expr::Call { callee, args } => self.eval_call(callee, args, meter),
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
        if let Some(error) = Self::unsupported_pattern_item_error(items, "sequence") {
            return Err(error);
        }

        if let Some(nodes) = self.collect_sample_nodes(items, meter)? {
            return Ok(Value::SamplePattern(SamplePatternValue::from_nodes(nodes)));
        }

        if let Some(nodes) = self.collect_number_nodes(items)? {
            return Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)));
        }

        for item in items {
            let _ = self.eval_expr_in_meter(item, meter)?;
        }

        Err(EvalError::new(
            "sequence items must all resolve to the same structural pattern kind",
        ))
    }

    fn eval_group(&self, items: &[Expr], meter: Option<&MeterContext>) -> Result<Value, EvalError> {
        if let Some(error) = Self::unsupported_pattern_item_error(items, "group") {
            return Err(error);
        }

        if let Some(nodes) = self.collect_sample_nodes(items, meter)? {
            return Ok(Value::SamplePattern(SamplePatternValue::from_group(nodes)));
        }

        if let Some(nodes) = self.collect_number_nodes(items)? {
            return Ok(Value::NumberPattern(NumberPatternValue::from_group(nodes)));
        }

        for item in items {
            let _ = self.eval_expr_in_meter(item, meter)?;
        }

        Err(EvalError::new(
            "group items must all resolve to the same structural pattern kind",
        ))
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
                self.eval_call_with_args(callee, args, vec![lhs_value], meter)
            }
            _ => Self::apply_value(self.eval_expr_in_meter(rhs, meter)?, vec![lhs_value]),
        }
    }

    fn eval_call(
        &self,
        callee: &Expr,
        args: &[Expr],
        meter: Option<&MeterContext>,
    ) -> Result<Value, EvalError> {
        self.eval_call_with_args(callee, args, Vec::new(), meter)
    }

    fn eval_call_with_args(
        &self,
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
        Self::apply_value(callee_value, evaluated_args)
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

        self.eval_positive_integer(cycles, meter, "section cycle count")
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

    fn apply_value(callee: Value, args: Vec<Value>) -> Result<Value, EvalError> {
        match callee {
            Value::Function(function) => function.apply(args),
            Value::SamplePattern(_) | Value::NumberPattern(_) | Value::String(_) => Err(
                EvalError::new(format!("cannot call a {}", callee.kind_name())),
            ),
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
                "unresolved identifier `{name}` in loose mode; placeholder playback is not implemented in Task 5"
            ))),
            ReplMode::Strict => Err(EvalError::new(format!("unresolved identifier `{name}`"))),
        }
    }

    fn unsupported_pattern_item_error(items: &[Expr], context: &str) -> Option<EvalError> {
        for item in items {
            match item {
                Expr::Call { callee, args } => {
                    let name = match callee.as_ref() {
                        Expr::Ident(name) => name.as_str(),
                        Expr::Seq(_)
                        | Expr::Stack(_)
                        | Expr::Stream(_)
                        | Expr::Pipe { .. }
                        | Expr::Call { .. }
                        | Expr::At { .. }
                        | Expr::Meter { .. }
                        | Expr::Beat(_)
                        | Expr::Section { .. }
                        | Expr::SeqSections(_)
                        | Expr::Group(_)
                        | Expr::Rest
                        | Expr::Number(_)
                        | Expr::String(_) => "call",
                    };
                    if name == "sample" && args.len() == 1 {
                        continue;
                    }
                    return Some(EvalError::new(format!(
                        "function call `{name}` cannot appear inside a pattern {context} in Task 5; apply transforms with the pipe operator `|>` or call `{name}(..., pattern)` directly"
                    )));
                }
                Expr::Ident(name) if matches!(builtin_value(name), Some(Value::Function(_))) => {
                    return Some(EvalError::new(format!(
                        "function `{name}` cannot appear inside a pattern {context} in Task 5; apply transforms with the pipe operator `|>` or call `{name}(..., pattern)` directly"
                    )));
                }
                Expr::Stream(_)
                | Expr::At { .. }
                | Expr::Meter { .. }
                | Expr::Beat(_)
                | Expr::Section { .. }
                | Expr::SeqSections(_) => {
                    return Some(EvalError::new(format!(
                        "explicit-time forms cannot appear inside a pattern {context}; use `stream(...)` or lift the form outside the {context}"
                    )));
                }
                Expr::Group(group_items) => {
                    if let Some(error) = Self::unsupported_pattern_item_error(group_items, context)
                    {
                        return Some(error);
                    }
                }
                Expr::Seq(_)
                | Expr::Stack(_)
                | Expr::Pipe { .. }
                | Expr::Ident(_)
                | Expr::Rest
                | Expr::Number(_)
                | Expr::String(_) => {}
            }
        }

        None
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
            Expr::Seq(_)
            | Expr::Stack(_)
            | Expr::Stream(_)
            | Expr::Pipe { .. }
            | Expr::Call { .. }
            | Expr::At { .. }
            | Expr::Meter { .. }
            | Expr::Beat(_)
            | Expr::Section { .. }
            | Expr::SeqSections(_)
            | Expr::Ident(_)
            | Expr::Number(_)
            | Expr::String(_) => Ok(None),
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
            | Expr::Stream(_)
            | Expr::Pipe { .. }
            | Expr::Call { .. }
            | Expr::At { .. }
            | Expr::Meter { .. }
            | Expr::Beat(_)
            | Expr::Section { .. }
            | Expr::SeqSections(_)
            | Expr::Ident(_)
            | Expr::String(_) => Ok(None),
        }
    }
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

fn sample_trigger_from_event(event: &SampleEvent) -> SampleTrigger {
    let mut trigger = SampleTrigger::named(event.sample())
        .with_gain(event.gain())
        .with_pan(event.pan())
        .with_rate(event.rate())
        .with_slice(event.slice_start(), event.slice_end());
    if let Some(cutoff_hz) = event.hpf_cutoff_hz() {
        trigger = trigger.with_hpf_cutoff_hz(cutoff_hz);
    }
    if let Some(cutoff_hz) = event.lpf_cutoff_hz() {
        trigger = trigger.with_lpf_cutoff_hz(cutoff_hz);
    }
    trigger
}

fn extract_constant_number_rational(value: Value, context: &str) -> Result<Rational, EvalError> {
    let constant = extract_constant_number_value(value, context)?;
    f64_to_rational(constant, context)
}

fn f64_to_rational(value: f64, context: &str) -> Result<Rational, EvalError> {
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

fn render_span(cycle_count: u64) -> Result<TimeSpan, EvalError> {
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
