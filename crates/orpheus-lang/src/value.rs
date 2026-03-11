use core::cmp::{max, min};
use core::fmt;

use orpheus_pattern::{
    CyclePattern, Event, EventStream, PatternError, PatternNode, Rational, TimeSpan,
};

use crate::eval::EvalError;

#[derive(Clone, Copy, Debug)]
pub enum BuiltinKind {
    Fast,
    Slow,
    Rev,
    Gain,
    Pan,
    Sample,
    Rate,
    Slice,
    SliceIdx,
}

#[derive(Clone, Debug)]
pub struct BuiltinFn {
    pub(crate) kind: BuiltinKind,
    pub(crate) bound_args: Vec<Value>,
}

#[derive(Clone, Debug)]
pub enum Value {
    SamplePattern(SamplePatternValue),
    NumberPattern(NumberPatternValue),
    Function(BuiltinFn),
    String(String),
}

impl Value {
    #[must_use]
    pub const fn as_sample_pattern(&self) -> Option<&SamplePatternValue> {
        match self {
            Self::SamplePattern(pattern) => Some(pattern),
            Self::NumberPattern(_) | Self::Function(_) | Self::String(_) => None,
        }
    }

    #[must_use]
    pub const fn as_number_pattern(&self) -> Option<&NumberPatternValue> {
        match self {
            Self::NumberPattern(pattern) => Some(pattern),
            Self::SamplePattern(_) | Self::Function(_) | Self::String(_) => None,
        }
    }

    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::SamplePattern(_) => "sample pattern",
            Self::NumberPattern(_) => "number pattern",
            Self::Function(_) => "function",
            Self::String(_) => "string",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampleEvent {
    sample: Box<str>,
    gain: f64,
    pan: f64,
    rate: f64,
    slice_start: f64,
    slice_end: f64,
}

impl SampleEvent {
    pub(crate) fn named(sample: &str) -> Self {
        Self {
            sample: sample.into(),
            gain: 1.0,
            pan: 0.0,
            rate: 1.0,
            slice_start: 0.0,
            slice_end: 1.0,
        }
    }

    #[must_use]
    pub fn sample(&self) -> &str {
        self.sample.as_ref()
    }

    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    #[must_use]
    pub const fn pan(&self) -> f64 {
        self.pan
    }

    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    #[must_use]
    pub const fn slice_start(&self) -> f64 {
        self.slice_start
    }

    #[must_use]
    pub const fn slice_end(&self) -> f64 {
        self.slice_end
    }
}

trait PatternValueTransform {
    fn adjust_gain(&self, factor: f64) -> Self;
    fn adjust_pan(&self, amount: f64) -> Self;
    fn adjust_rate(&self, factor: f64) -> Self;
    fn adjust_slice(&self, start: f64, end: f64) -> Self;
}

impl PatternValueTransform for SampleEvent {
    fn adjust_gain(&self, factor: f64) -> Self {
        Self {
            sample: self.sample.clone(),
            gain: self.gain * factor,
            pan: self.pan,
            rate: self.rate,
            slice_start: self.slice_start,
            slice_end: self.slice_end,
        }
    }

    fn adjust_pan(&self, amount: f64) -> Self {
        Self {
            sample: self.sample.clone(),
            gain: self.gain,
            pan: (self.pan + amount).clamp(-1.0, 1.0),
            rate: self.rate,
            slice_start: self.slice_start,
            slice_end: self.slice_end,
        }
    }

    fn adjust_rate(&self, factor: f64) -> Self {
        Self {
            sample: self.sample.clone(),
            gain: self.gain,
            pan: self.pan,
            rate: self.rate * factor,
            slice_start: self.slice_start,
            slice_end: self.slice_end,
        }
    }

    fn adjust_slice(&self, start: f64, end: f64) -> Self {
        let current_range = self.slice_end - self.slice_start;
        Self {
            sample: self.sample.clone(),
            gain: self.gain,
            pan: self.pan,
            rate: self.rate,
            slice_start: current_range.mul_add(start, self.slice_start),
            slice_end: current_range.mul_add(end, self.slice_start),
        }
    }
}

impl PatternValueTransform for f64 {
    fn adjust_gain(&self, _factor: f64) -> Self {
        *self
    }

    fn adjust_pan(&self, _amount: f64) -> Self {
        *self
    }

    fn adjust_rate(&self, _factor: f64) -> Self {
        *self
    }

    fn adjust_slice(&self, _start: f64, _end: f64) -> Self {
        *self
    }
}

#[derive(Clone, Debug)]
pub struct SamplePatternValue {
    pattern: PatternRuntime<SampleEvent>,
}

impl SamplePatternValue {
    pub(crate) fn atom(sample: &str) -> Self {
        Self::from_nodes(vec![PatternNode::atom(SampleEvent::named(sample))])
    }

    pub(crate) const fn from_nodes(nodes: Vec<PatternNode<SampleEvent>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(nodes)),
        }
    }

    pub(crate) fn from_group(nodes: Vec<PatternNode<SampleEvent>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(vec![PatternNode::group(
                nodes,
            )])),
        }
    }

    pub(crate) fn stack(patterns: Vec<Self>) -> Self {
        Self {
            pattern: PatternRuntime::Stack(
                patterns
                    .into_iter()
                    .map(|pattern| pattern.pattern)
                    .collect(),
            ),
        }
    }

    pub(crate) fn fast(self, factor: i64) -> Self {
        Self {
            pattern: PatternRuntime::Fast {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slow(self, factor: i64) -> Self {
        Self {
            pattern: PatternRuntime::Slow {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rev(self) -> Self {
        Self {
            pattern: PatternRuntime::Rev {
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn gain(self, factor: f64) -> Self {
        Self {
            pattern: PatternRuntime::Gain {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn gain_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::GainPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pan(self, amount: f64) -> Self {
        Self {
            pattern: PatternRuntime::Pan {
                amount,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pan_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::PanPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rate(self, factor: f64) -> Self {
        Self {
            pattern: PatternRuntime::Rate {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rate_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::RatePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slice(self, start: f64, end: f64) -> Self {
        Self {
            pattern: PatternRuntime::Slice {
                start,
                end,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn from_events(events: Vec<Event<SampleEvent>>) -> Self {
        Self {
            pattern: PatternRuntime::Stream(EventStream::new(events)),
        }
    }

    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    /// # Panics
    ///
    /// Panics if an internal runtime transform produces an invalid span or
    /// overflows the evaluator's bounded rational arithmetic.
    #[must_use]
    pub fn query_unit(&self) -> Vec<Event<SampleEvent>> {
        self.try_query(&TimeSpan::unit())
            .unwrap_or_else(|error| panic!("sample pattern query failed: {error}"))
    }

    pub(crate) fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<SampleEvent>>, EvalError> {
        self.pattern.try_query(span)
    }
}

#[derive(Clone, Debug)]
pub struct NumberPatternValue {
    pattern: PatternRuntime<f64>,
}

impl NumberPatternValue {
    pub(crate) fn constant(value: f64) -> Self {
        Self::from_nodes(vec![PatternNode::atom(value)])
    }

    pub(crate) const fn from_nodes(nodes: Vec<PatternNode<f64>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(nodes)),
        }
    }

    pub(crate) fn from_group(nodes: Vec<PatternNode<f64>>) -> Self {
        Self {
            pattern: PatternRuntime::Cycle(CyclePattern::from_nodes(vec![PatternNode::group(
                nodes,
            )])),
        }
    }

    pub(crate) fn stack(patterns: Vec<Self>) -> Self {
        Self {
            pattern: PatternRuntime::Stack(
                patterns
                    .into_iter()
                    .map(|pattern| pattern.pattern)
                    .collect(),
            ),
        }
    }

    pub(crate) fn fast(self, factor: i64) -> Self {
        Self {
            pattern: PatternRuntime::Fast {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slow(self, factor: i64) -> Self {
        Self {
            pattern: PatternRuntime::Slow {
                factor,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn rev(self) -> Self {
        Self {
            pattern: PatternRuntime::Rev {
                inner: Box::new(self.pattern),
            },
        }
    }

    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    /// # Panics
    ///
    /// Panics if an internal runtime transform produces an invalid span or
    /// overflows the evaluator's bounded rational arithmetic.
    #[must_use]
    pub fn query_unit(&self) -> Vec<Event<f64>> {
        self.try_query(&TimeSpan::unit())
            .unwrap_or_else(|error| panic!("number pattern query failed: {error}"))
    }

    pub(crate) fn constant_value(&self) -> Result<f64, EvalError> {
        let events = self.try_query(&TimeSpan::unit())?;
        let unit = TimeSpan::unit();

        match events.as_slice() {
            [event] if event.whole.is_none() && event.part == unit => Ok(event.value),
            _ => Err(EvalError::new(
                "expected a constant number pattern over the unit cycle",
            )),
        }
    }

    pub(crate) fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<f64>>, EvalError> {
        self.pattern.try_query(span)
    }

    pub(crate) fn from_events(events: Vec<Event<f64>>) -> Self {
        Self {
            pattern: PatternRuntime::Stream(EventStream::new(events)),
        }
    }
}

#[derive(Clone, Debug)]
enum PatternRuntime<T> {
    Cycle(CyclePattern<T>),
    Stream(EventStream<T>),
    Stack(Vec<Self>),
    Fast {
        factor: i64,
        inner: Box<Self>,
    },
    Slow {
        factor: i64,
        inner: Box<Self>,
    },
    Rev {
        inner: Box<Self>,
    },
    Gain {
        factor: f64,
        inner: Box<Self>,
    },
    GainPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Pan {
        amount: f64,
        inner: Box<Self>,
    },
    PanPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Rate {
        factor: f64,
        inner: Box<Self>,
    },
    RatePattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Slice {
        start: f64,
        end: f64,
        inner: Box<Self>,
    },
}

impl<T> PatternRuntime<T>
where
    T: Clone + PatternValueTransform + Send + Sync + fmt::Debug,
{
    fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Cycle(pattern) => pattern
                .try_query(span)
                .map_err(|error| map_pattern_error(&error)),
            Self::Stream(stream) => stream
                .try_query(span)
                .map_err(|error| map_pattern_error(&error)),
            Self::Stack(layers) => {
                let mut events = Vec::new();
                for layer in layers {
                    events.extend(layer.try_query(span)?);
                }
                sort_events(&mut events);
                Ok(events)
            }
            Self::Fast { factor, inner } => query_fast(inner, *factor, span),
            Self::Slow { factor, inner } => query_slow(inner, *factor, span),
            Self::Rev { inner } => query_rev(inner, span),
            Self::Gain { factor, inner } => {
                let mut events = inner.try_query(span)?;
                for event in &mut events {
                    event.value = event.value.adjust_gain(*factor);
                }
                Ok(events)
            }
            Self::GainPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Gain)
            }
            Self::Pan { amount, inner } => {
                let mut events = inner.try_query(span)?;
                for event in &mut events {
                    event.value = event.value.adjust_pan(*amount);
                }
                Ok(events)
            }
            Self::PanPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pan)
            }
            Self::Rate { factor, inner } => {
                let mut events = inner.try_query(span)?;
                for event in &mut events {
                    event.value = event.value.adjust_rate(*factor);
                }
                Ok(events)
            }
            Self::RatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Rate)
            }
            Self::Slice { start, end, inner } => {
                let mut events = inner.try_query(span)?;
                for event in &mut events {
                    event.value = event.value.adjust_slice(*start, *end);
                }
                Ok(events)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ControlPatternKind {
    Gain,
    Pan,
    Rate,
}

fn apply_control_pattern<T>(
    inner: &PatternRuntime<T>,
    control: &PatternRuntime<f64>,
    span: &TimeSpan,
    kind: ControlPatternKind,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: Clone + PatternValueTransform + Send + Sync + fmt::Debug,
{
    let source_events = inner.try_query(span)?;
    let control_events = control.try_query(span)?;
    validate_control_events(&control_events, kind)?;
    if control_events.is_empty() {
        return Ok(source_events);
    }

    let mut composed = Vec::new();
    for event in source_events {
        let mut boundaries = vec![event.part.start().clone(), event.part.end().clone()];
        let mut has_overlap = false;
        for control_event in &control_events {
            if let Some(overlap) = clip_span(&control_event.part, &event.part)? {
                has_overlap = true;
                boundaries.push(overlap.start().clone());
                boundaries.push(overlap.end().clone());
            }
        }

        if !has_overlap {
            composed.push(event);
            continue;
        }

        boundaries.sort();
        boundaries.dedup();

        for window in boundaries.windows(2) {
            let [start, end] = window else {
                continue;
            };
            if start >= end {
                continue;
            }

            let part = build_span(start.clone(), end.clone())?;
            let mut value = event.value.clone();
            for control_event in &control_events {
                if clip_span(&control_event.part, &part)?.is_some() {
                    value = match kind {
                        ControlPatternKind::Gain => value.adjust_gain(control_event.value),
                        ControlPatternKind::Pan => value.adjust_pan(control_event.value),
                        ControlPatternKind::Rate => value.adjust_rate(control_event.value),
                    };
                }
            }

            composed.push(Event {
                whole: None,
                part,
                value,
            });
        }
    }

    sort_events(&mut composed);
    Ok(composed)
}

fn validate_control_events(
    control_events: &[Event<f64>],
    kind: ControlPatternKind,
) -> Result<(), EvalError> {
    for event in control_events {
        match kind {
            ControlPatternKind::Gain => {
                if !event.value.is_finite() {
                    return Err(EvalError::new(
                        "`gain` requires finite numeric control values",
                    ));
                }
            }
            ControlPatternKind::Pan => {
                if !event.value.is_finite() || !(-1.0..=1.0).contains(&event.value) {
                    return Err(EvalError::new(
                        "`pan` requires finite control values within [-1, 1]",
                    ));
                }
            }
            ControlPatternKind::Rate => {
                if !event.value.is_finite() || event.value.abs() <= f64::EPSILON {
                    return Err(EvalError::new(
                        "`rate` requires finite non-zero control values",
                    ));
                }
            }
        }
    }

    Ok(())
}

fn query_fast<T>(
    inner: &PatternRuntime<T>,
    factor: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: Clone + PatternValueTransform + Send + Sync + fmt::Debug,
{
    let source_span = scale_span(span, factor, 1)?;
    let mut events = inner.try_query(&source_span)?;
    rescale_events(&mut events, 1, factor)?;
    Ok(events)
}

fn query_slow<T>(
    inner: &PatternRuntime<T>,
    factor: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: Clone + PatternValueTransform + Send + Sync + fmt::Debug,
{
    let source_span = scale_span(span, 1, factor)?;
    let mut events = inner.try_query(&source_span)?;
    rescale_events(&mut events, factor, 1)?;
    Ok(events)
}

fn query_rev<T>(inner: &PatternRuntime<T>, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError>
where
    T: Clone + PatternValueTransform + Send + Sync + fmt::Debug,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    for cycle in start_cycle..end_cycle {
        let cycle_span = cycle_span(cycle)?;
        let Some(query_slice) = clip_span(&cycle_span, span)? else {
            continue;
        };

        let mirrored_query = mirror_span_in_cycle(&query_slice, cycle)?;
        let mut mirrored_events = inner.try_query(&mirrored_query)?;
        for event in &mut mirrored_events {
            event.part = mirror_span_in_cycle(&event.part, cycle)?;
            if let Some(whole) = event.whole.take() {
                event.whole = Some(mirror_span_in_cycle(&whole, cycle)?);
            }
        }
        events.extend(mirrored_events);
    }

    sort_events(&mut events);
    Ok(events)
}

fn rescale_events<T>(
    events: &mut [Event<T>],
    numerator: i64,
    denominator: i64,
) -> Result<(), EvalError> {
    for event in events {
        event.part = scale_span(&event.part, numerator, denominator)?;
        if let Some(whole) = event.whole.take() {
            event.whole = Some(scale_span(&whole, numerator, denominator)?);
        }
    }

    Ok(())
}

fn sort_events<T>(events: &mut [Event<T>]) {
    events.sort_by(|left, right| {
        left.part
            .start()
            .cmp(right.part.start())
            .then(left.part.end().cmp(right.part.end()))
    });
}

fn cycle_span(cycle: i128) -> Result<TimeSpan, EvalError> {
    let start = rational_from_parts(cycle, 1)?;
    let end = rational_from_parts(
        cycle
            .checked_add(1)
            .ok_or_else(|| EvalError::new("cycle index overflowed while reversing a pattern"))?,
        1,
    )?;
    build_span(start, end)
}

fn mirror_span_in_cycle(span: &TimeSpan, cycle: i128) -> Result<TimeSpan, EvalError> {
    let cycle_start = rational_from_parts(cycle, 1)?;
    let local_start = rational_sub(span.start(), &cycle_start)?;
    let local_end = rational_sub(span.end(), &cycle_start)?;
    let mirrored_start = rational_sub(&Rational::one(), &local_end)?;
    let mirrored_end = rational_sub(&Rational::one(), &local_start)?;

    build_span(
        rational_add(&cycle_start, &mirrored_start)?,
        rational_add(&cycle_start, &mirrored_end)?,
    )
}

fn clip_span(span: &TimeSpan, query: &TimeSpan) -> Result<Option<TimeSpan>, EvalError> {
    let start = max(span.start(), query.start()).clone();
    let end = min(span.end(), query.end()).clone();

    if start >= end {
        return Ok(None);
    }

    build_span(start, end).map(Some)
}

fn scale_span(span: &TimeSpan, numerator: i64, denominator: i64) -> Result<TimeSpan, EvalError> {
    build_span(
        rational_mul_parts(span.start(), numerator, denominator)?,
        rational_mul_parts(span.end(), numerator, denominator)?,
    )
}

fn build_span(start: Rational, end: Rational) -> Result<TimeSpan, EvalError> {
    TimeSpan::new(start, end).map_err(|error| map_pattern_error(&error))
}

fn rational_add(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    left.checked_add(right)
        .map_err(|error| map_pattern_error(&error))
}

fn rational_sub(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    left.checked_sub(right)
        .map_err(|error| map_pattern_error(&error))
}

fn rational_mul_parts(
    value: &Rational,
    numerator: i64,
    denominator: i64,
) -> Result<Rational, EvalError> {
    let factor = Rational::checked_from_parts(i128::from(numerator), i128::from(denominator))
        .map_err(|error| map_pattern_error(&error))?;
    value
        .checked_mul(&factor)
        .map_err(|error| map_pattern_error(&error))
}

fn rational_from_parts(numerator: i128, denominator: i128) -> Result<Rational, EvalError> {
    Rational::checked_from_parts(numerator, denominator).map_err(|error| map_pattern_error(&error))
}

fn map_pattern_error(error: &PatternError) -> EvalError {
    EvalError::new(error.to_string())
}

const fn floor_rational(value: &Rational) -> i128 {
    let quotient = value.numerator() / value.denominator();
    let remainder = value.numerator() % value.denominator();

    if remainder < 0 {
        quotient - 1
    } else {
        quotient
    }
}

const fn ceil_rational(value: &Rational) -> i128 {
    let quotient = value.numerator() / value.denominator();
    let remainder = value.numerator() % value.denominator();

    if remainder > 0 {
        quotient + 1
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use super::cycle_span;

    #[test]
    fn cycle_span_supports_indices_above_i64_range() {
        let start_cycle = i128::from(i64::MAX) + 1;
        let span = cycle_span(start_cycle).unwrap();

        assert_eq!(span.start().numerator(), start_cycle);
        assert_eq!(span.start().denominator(), 1);
        assert_eq!(span.end().numerator(), start_cycle + 1);
        assert_eq!(span.end().denominator(), 1);
    }
}
