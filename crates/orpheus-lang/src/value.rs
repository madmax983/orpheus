use core::cmp::{max, min};
use core::fmt;

use orpheus_pattern::{CyclePattern, Event, PatternNode, Rational, TimeSpan};

use crate::eval::EvalError;

#[derive(Clone, Copy, Debug)]
pub enum BuiltinKind {
    Fast,
    Slow,
    Rev,
    Gain,
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
}

impl Value {
    #[must_use]
    pub const fn as_sample_pattern(&self) -> Option<&SamplePatternValue> {
        match self {
            Self::SamplePattern(pattern) => Some(pattern),
            Self::NumberPattern(_) | Self::Function(_) => None,
        }
    }

    #[must_use]
    pub const fn as_number_pattern(&self) -> Option<&NumberPatternValue> {
        match self {
            Self::NumberPattern(pattern) => Some(pattern),
            Self::SamplePattern(_) | Self::Function(_) => None,
        }
    }

    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::SamplePattern(_) => "sample pattern",
            Self::NumberPattern(_) => "number pattern",
            Self::Function(_) => "function",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampleEvent {
    sample: Box<str>,
    gain: f64,
}

impl SampleEvent {
    pub(crate) fn named(sample: &str) -> Self {
        Self {
            sample: sample.into(),
            gain: 1.0,
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
}

trait GainAdjustable {
    fn adjust_gain(&self, factor: f64) -> Self;
}

impl GainAdjustable for SampleEvent {
    fn adjust_gain(&self, factor: f64) -> Self {
        Self {
            sample: self.sample.clone(),
            gain: self.gain * factor,
        }
    }
}

impl GainAdjustable for f64 {
    fn adjust_gain(&self, _factor: f64) -> Self {
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
}

#[derive(Clone, Debug)]
enum PatternRuntime<T> {
    Cycle(CyclePattern<T>),
    Stack(Vec<Self>),
    Fast { factor: i64, inner: Box<Self> },
    Slow { factor: i64, inner: Box<Self> },
    Rev { inner: Box<Self> },
    Gain { factor: f64, inner: Box<Self> },
}

impl<T> PatternRuntime<T>
where
    T: Clone + GainAdjustable + Send + Sync + fmt::Debug,
{
    fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Cycle(pattern) => pattern
                .try_query(span)
                .map_err(|error| EvalError::new(error.to_string())),
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
        }
    }
}

fn query_fast<T>(
    inner: &PatternRuntime<T>,
    factor: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: Clone + GainAdjustable + Send + Sync + fmt::Debug,
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
    T: Clone + GainAdjustable + Send + Sync + fmt::Debug,
{
    let source_span = scale_span(span, 1, factor)?;
    let mut events = inner.try_query(&source_span)?;
    rescale_events(&mut events, factor, 1)?;
    Ok(events)
}

fn query_rev<T>(inner: &PatternRuntime<T>, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError>
where
    T: Clone + GainAdjustable + Send + Sync + fmt::Debug,
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
    TimeSpan::new(start, end).map_err(|error| EvalError::new(error.to_string()))
}

fn rational_add(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    let left_scaled = left
        .numerator()
        .checked_mul(right.denominator())
        .ok_or_else(|| EvalError::new("rational addition overflowed"))?;
    let right_scaled = right
        .numerator()
        .checked_mul(left.denominator())
        .ok_or_else(|| EvalError::new("rational addition overflowed"))?;
    let numerator = left_scaled
        .checked_add(right_scaled)
        .ok_or_else(|| EvalError::new("rational addition overflowed"))?;
    let denominator = left
        .denominator()
        .checked_mul(right.denominator())
        .ok_or_else(|| EvalError::new("rational addition overflowed"))?;

    rational_from_parts(numerator, denominator)
}

fn rational_sub(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    let numerator = right
        .numerator()
        .checked_neg()
        .ok_or_else(|| EvalError::new("rational subtraction overflowed"))?;
    let negated = rational_from_parts(numerator, right.denominator())?;
    rational_add(left, &negated)
}

fn rational_mul_parts(
    value: &Rational,
    numerator: i64,
    denominator: i64,
) -> Result<Rational, EvalError> {
    let scaled_numerator = value
        .numerator()
        .checked_mul(i128::from(numerator))
        .ok_or_else(|| EvalError::new("rational multiplication overflowed"))?;
    let scaled_denominator = value
        .denominator()
        .checked_mul(i128::from(denominator))
        .ok_or_else(|| EvalError::new("rational multiplication overflowed"))?;

    rational_from_parts(scaled_numerator, scaled_denominator)
}

fn rational_from_parts(numerator: i128, denominator: i128) -> Result<Rational, EvalError> {
    if denominator == 0 {
        return Err(EvalError::new("rational denominator cannot be zero"));
    }

    if numerator == 0 {
        return Ok(Rational::zero());
    }

    let (normalized_numerator, normalized_denominator) = if denominator < 0 {
        (
            numerator
                .checked_neg()
                .ok_or_else(|| EvalError::new("rational normalization overflowed"))?,
            denominator
                .checked_neg()
                .ok_or_else(|| EvalError::new("rational normalization overflowed"))?,
        )
    } else {
        (numerator, denominator)
    };

    let divisor = gcd_u128(
        normalized_numerator.unsigned_abs(),
        normalized_denominator.unsigned_abs(),
    );
    let divisor =
        i128::try_from(divisor).map_err(|_| EvalError::new("rational normalization overflowed"))?;
    let reduced_numerator = normalized_numerator / divisor;
    let reduced_denominator = normalized_denominator / divisor;
    let numerator = i64::try_from(reduced_numerator)
        .map_err(|_| EvalError::new("rational value exceeded evaluator range"))?;
    let denominator = i64::try_from(reduced_denominator)
        .map_err(|_| EvalError::new("rational value exceeded evaluator range"))?;

    Rational::new(numerator, denominator).map_err(|error| EvalError::new(error.to_string()))
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }

    left
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
