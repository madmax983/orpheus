//! Core runtime types and evaluator representation for Orpheus.
//!
//! This module defines how the language interprets expressions at runtime. It
//! houses the `Value` type representing evaluated programs, along with its
//! concrete representations for samples and numbers.
//!
//! When Orpheus source code is executed (e.g., `drums = bd sn`), it is parsed
//! into an AST and then evaluated into a [`Value`]. The specific kind of value
//! depends on what the expression represents. A sequence of audio samples will
//! produce a `Value::SamplePattern`, which holds a [`SamplePatternValue`].
//! This inner pattern acts as a delayed computation that can be queried over time.
//!
//! The most common workflow involves taking a bound [`Value`], unwrapping it into
//! a concrete [`SamplePatternValue`], and then asking it for the actual scheduled
//! events over a specific window of time using `query_unit()`. Those events are
//! returned as fully-materialized [`SampleEvent`]s.

use core::cmp::{max, min};
use core::fmt;

use orpheus_pattern::{
    CyclePattern, Event, EventStream, PatternError, PatternNode, Rational, TimeSpan,
};

use crate::{builtins::apply_builtin_function, eval::EvalError};

/// Identifies which core built-in function is being represented.
///
/// These variants map exactly to the standard Orpheus primitive transformations
/// available in the base language.
#[derive(Clone, Copy, Debug)]
pub enum BuiltinKind {
    Every,
    Sometimes,
    Fast,
    Slow,
    Shift,
    Rev,
    Gain,
    Hpf,
    Lpf,
    Pan,
    Pitch,
    Sample,
    Rate,
    Slice,
    SliceIdx,
    Rand,
    Jux,
}

/// A partially or fully applied built-in function at runtime.
///
/// This structure tracks the function's identity alongside arguments that have
/// already been supplied. It supports partial application up to the function's
/// required arity.
#[derive(Clone, Debug)]
pub struct BuiltinFn {
    pub(crate) kind: BuiltinKind,
    pub(crate) bound_args: Vec<Value>,
    pub(crate) site_salt: Option<u64>,
}

/// Represents the fundamental unit of an evaluated expression.
///
/// Values can be sample-based audio patterns, raw numerical envelopes, primitive
/// built-in functions, or plain strings (often used as identifiers).
///
/// # Examples
///
/// You can extract the concrete pattern out of a generic value using the provided
/// helper methods:
///
/// ```
/// use orpheus_lang::{Value, SamplePatternValue, NumberPatternValue};
///
/// let string_val = Value::String("hello".into());
/// assert!(string_val.as_sample_pattern().is_none());
/// ```
///
/// Under the hood, a `Value` acts as a unified currency passed between
/// built-in functions (like `fast` or `gain`) and the core evaluation loop.
/// This enum allows Orpheus to be dynamically typed at the expression level,
/// deferring type resolution until execution time, which enables the REPL's
/// highly interactive, loosely-coupled workflow.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use orpheus_lang::{eval_module, ReplMode, Value, SamplePatternValue, NumberPatternValue};
///
/// let source = "x = fast(2, bd) \n y = 1.5";
/// let env = eval_module(source, ReplMode::Loose).unwrap();
///
/// let sample_pattern = env.get("x").unwrap();
/// let number_pattern = env.get("y").unwrap();
///
/// assert!(matches!(sample_pattern, Value::SamplePattern(_)));
/// assert!(matches!(number_pattern, Value::NumberPattern(_)));
/// ```
#[derive(Clone, Debug)]
pub enum Value {
    SamplePattern(SamplePatternValue),
    NumberPattern(NumberPatternValue),
    Function(BuiltinFn),
    String(String),
}

impl Value {
    /// Attempts to unwrap the value into a concrete sample pattern.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::{Value, SamplePatternValue};
    ///
    /// let val = Value::String("foo".into());
    /// assert!(val.as_sample_pattern().is_none());
    /// ```
    #[must_use]
    pub const fn as_sample_pattern(&self) -> Option<&SamplePatternValue> {
        match self {
            Self::SamplePattern(pattern) => Some(pattern),
            Self::NumberPattern(_) | Self::Function(_) | Self::String(_) => None,
        }
    }

    /// Attempts to unwrap the value into a concrete number pattern.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::{Value, NumberPatternValue};
    ///
    /// let val = Value::String("foo".into());
    /// assert!(val.as_number_pattern().is_none());
    /// ```
    #[must_use]
    pub const fn as_number_pattern(&self) -> Option<&NumberPatternValue> {
        match self {
            Self::NumberPattern(pattern) => Some(pattern),
            Self::SamplePattern(_) | Self::Function(_) | Self::String(_) => None,
        }
    }

    /// Returns a human-readable description of this value's underlying type.
    ///
    /// This is used heavily in runtime type mismatch error messages.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    ///
    /// let val = Value::String("foo".into());
    /// assert_eq!(val.kind_name(), "string");
    /// ```
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

/// A single, fully-materialized audio event bound to a slice of time.
///
/// This contains a sample's name along with all of its applied signal processing
/// parameters (gain, panning, filtering, playback rate, and slicing).
/// These events are what ultimately get sent to `orpheus_dsp` for rendering.
///
/// # Examples
///
/// Sample events are typically generated by querying a [`SamplePatternValue`]
/// over a specific span of time, but can also be manually constructed (which is
/// useful for testing).
///
/// ```
/// # use orpheus_lang::{eval_module, ReplMode};
/// # let env = eval_module("event = bd", ReplMode::Strict).unwrap();
/// # let val = env.get("event").unwrap().as_sample_pattern().unwrap();
/// # let event = &val.query_unit().unwrap()[0].value;
/// assert_eq!(event.sample(), "bd");
/// assert_eq!(event.gain(), 1.0); // Defaults to full volume.
/// ```
///
/// # Applying Effects
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode};
///
/// // Create a pattern with a sample, customized gain, and adjusted playback rate.
/// let source = "event = gain(0.8, rate(1.5, bd))";
/// let env = eval_module(source, ReplMode::Strict).unwrap();
/// let val = env.get("event").unwrap().as_sample_pattern().unwrap();
///
/// // Extract the first generated event.
/// let events = val.query_unit().unwrap();
/// let event = &events[0].value;
///
/// assert_eq!(event.sample(), "bd");
/// assert_eq!(event.gain(), 0.8);
/// assert_eq!(event.rate(), 1.5);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct SampleEvent {
    sample: Box<str>,
    gain: f64,
    hpf_cutoff_hz: Option<f64>,
    lpf_cutoff_hz: Option<f64>,
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
            hpf_cutoff_hz: None,
            lpf_cutoff_hz: None,
            pan: 0.0,
            rate: 1.0,
            slice_start: 0.0,
            slice_end: 1.0,
        }
    }

    /// The string identifier of the raw audio sample.
    #[must_use]
    pub fn sample(&self) -> &str {
        self.sample.as_ref()
    }

    /// The amplitude multiplier applied to this event.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// The high-pass filter cutoff frequency in Hertz, if one is active.
    #[must_use]
    pub const fn hpf_cutoff_hz(&self) -> Option<f64> {
        self.hpf_cutoff_hz
    }

    /// The low-pass filter cutoff frequency in Hertz, if one is active.
    #[must_use]
    pub const fn lpf_cutoff_hz(&self) -> Option<f64> {
        self.lpf_cutoff_hz
    }

    /// The stereo panning position, clamped between -1.0 (Left) and 1.0 (Right).
    #[must_use]
    pub const fn pan(&self) -> f64 {
        self.pan
    }

    /// The playback speed multiplier. Values greater than 1 speed up and pitch up.
    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    /// The normalized starting position `[0, 1]` within the raw audio sample.
    #[must_use]
    pub const fn slice_start(&self) -> f64 {
        self.slice_start
    }

    /// The normalized ending position `[0, 1]` within the raw audio sample.
    #[must_use]
    pub const fn slice_end(&self) -> f64 {
        self.slice_end
    }
}

trait PatternValueTransform {
    fn adjust_gain(&mut self, factor: f64);
    fn adjust_hpf(&mut self, cutoff_hz: f64);
    fn adjust_lpf(&mut self, cutoff_hz: f64);
    fn adjust_pan(&mut self, amount: f64);
    fn adjust_rate(&mut self, factor: f64);
    fn adjust_slice(&mut self, start: f64, end: f64);
}

trait PatternRuntimeValue: Clone + PatternValueTransform + Send + Sync + fmt::Debug + Sized {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value;
    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError>;
    fn try_from_rand(value: f64) -> Result<Self, EvalError>;
}

impl PatternValueTransform for SampleEvent {
    fn adjust_gain(&mut self, factor: f64) {
        self.gain *= factor;
    }

    fn adjust_hpf(&mut self, cutoff_hz: f64) {
        self.hpf_cutoff_hz = Some(cutoff_hz);
    }

    fn adjust_lpf(&mut self, cutoff_hz: f64) {
        self.lpf_cutoff_hz = Some(cutoff_hz);
    }

    fn adjust_pan(&mut self, amount: f64) {
        self.pan = (self.pan + amount).clamp(-1.0, 1.0);
    }

    fn adjust_rate(&mut self, factor: f64) {
        self.rate *= factor;
    }

    fn adjust_slice(&mut self, start: f64, end: f64) {
        let current_range = self.slice_end - self.slice_start;
        let new_start = current_range.mul_add(start, self.slice_start);
        let new_end = current_range.mul_add(end, self.slice_start);
        self.slice_start = new_start;
        self.slice_end = new_end;
    }
}

impl PatternValueTransform for f64 {
    fn adjust_gain(&mut self, _factor: f64) {}

    fn adjust_hpf(&mut self, _cutoff_hz: f64) {}

    fn adjust_lpf(&mut self, _cutoff_hz: f64) {}

    fn adjust_pan(&mut self, _amount: f64) {}

    fn adjust_rate(&mut self, _factor: f64) {}

    fn adjust_slice(&mut self, _start: f64, _end: f64) {}
}

impl PatternRuntimeValue for SampleEvent {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value {
        Value::SamplePattern(SamplePatternValue { pattern })
    }

    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError> {
        match value {
            Value::SamplePattern(pattern) => Ok(pattern.pattern),
            Value::NumberPattern(_) | Value::Function(_) | Value::String(_) => Err(EvalError::new(
                "transform returned an incompatible value; expected Pattern<Sample>",
            )),
        }
    }

    fn try_from_rand(_value: f64) -> Result<Self, EvalError> {
        Err(EvalError::new("rand only produces numbers"))
    }
}

impl PatternRuntimeValue for f64 {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value {
        Value::NumberPattern(NumberPatternValue { pattern })
    }

    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError> {
        match value {
            Value::NumberPattern(pattern) => Ok(pattern.pattern),
            Value::SamplePattern(_) | Value::Function(_) | Value::String(_) => Err(EvalError::new(
                "transform returned an incompatible value; expected Pattern<Number>",
            )),
        }
    }

    fn try_from_rand(value: f64) -> Result<Self, EvalError> {
        Ok(value)
    }
}

/// A delayed computation representing a sequence of audio sample events over time.
///
/// This pattern can be queried over specific temporal windows to yield fully-realized
/// [`SampleEvent`]s.
///
/// # Examples
///
/// You can query a sample pattern for its events using `query_unit()`.
/// Note that Orpheus operates on exact continuous time, returning `Event` objects
/// containing rational `TimeSpan`s.
///
/// Orpheus' defining characteristic is that patterns are not static arrays of
/// audio data; they are mathematical functions from time to events. This delayed
/// evaluation is what allows infinite nesting of transforms (like `fast` or `every`)
/// without memory overhead. The `SamplePatternValue` holds the actual AST node
/// graph representing these delayed transforms.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode, SamplePatternValue, SampleEvent};
/// use orpheus_pattern::{Event, TimeSpan};
///
/// // Simulate the Orpheus expression `bd sn`
/// let env = eval_module("pattern = bd", ReplMode::Strict).unwrap();
/// let pattern = env.get("pattern").unwrap().as_sample_pattern().unwrap();
/// let events = pattern.query_unit().unwrap();
///
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0].value.sample(), "bd");
/// ```
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode, SamplePatternValue, SampleEvent};
/// use orpheus_pattern::{Event, TimeSpan};
///
/// // Simulate the Orpheus expression `bd sn`
/// let env = eval_module("pattern = bd sn", ReplMode::Strict).unwrap();
/// let pattern = env.get("pattern").unwrap().as_sample_pattern().unwrap();
///
/// // By querying the unit cycle, we get the two events inside the period [0, 1).
/// let events = pattern.query_unit().unwrap();
///
/// assert_eq!(events.len(), 2);
/// assert_eq!(events[0].value.sample(), "bd");
/// assert_eq!(events[1].value.sample(), "sn");
/// ```
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

    pub(crate) fn every(self, period: i64, transform: BuiltinFn) -> Self {
        Self {
            pattern: PatternRuntime::Every {
                period,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn sometimes_with_site_salt(self, transform: BuiltinFn, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Sometimes {
                site_salt,
                transform,
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

    pub(crate) fn shift(self, offset: Rational) -> Self {
        Self {
            pattern: PatternRuntime::Shift {
                offset,
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

    pub(crate) fn hpf(self, cutoff_hz: f64) -> Self {
        Self {
            pattern: PatternRuntime::Hpf {
                cutoff_hz,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn hpf_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::HpfPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn lpf(self, cutoff_hz: f64) -> Self {
        Self {
            pattern: PatternRuntime::Lpf {
                cutoff_hz,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn lpf_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::LpfPattern {
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

    pub(crate) fn pitch(self, semitones: f64) -> Self {
        Self {
            pattern: PatternRuntime::Pitch {
                semitones,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pitch_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::PitchPattern {
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

    pub(crate) fn slice_pattern(
        self,
        start_control: NumberPatternValue,
        end_control: NumberPatternValue,
    ) -> Self {
        Self {
            pattern: PatternRuntime::SlicePattern {
                start_control: Box::new(start_control.pattern),
                end_control: Box::new(end_control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn slice_idx_pattern(self, control: NumberPatternValue, segments: u32) -> Self {
        Self {
            pattern: PatternRuntime::SliceIdxPattern {
                control: Box::new(control.pattern),
                segments,
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
    /// # Errors
    ///
    /// Returns an error if an internal runtime transform produces an invalid
    /// span or overflows the evaluator's bounded rational arithmetic.
    #[must_use = "query_unit() returns a Result; ignoring it may drop query errors"]
    pub fn query_unit(&self) -> Result<Vec<Event<SampleEvent>>, EvalError> {
        self.try_query(&TimeSpan::unit())
    }

    pub(crate) fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<SampleEvent>>, EvalError> {
        self.pattern.try_query(span)
    }
}

/// A delayed computation representing a sequence of raw numbers over time.
///
/// Often used for controlling the parameters (such as `gain` or `pan`) of
/// other, sample-based patterns.
///
/// # Examples
///
/// You can query a number pattern just like a sample pattern.
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode, NumberPatternValue};
/// use orpheus_pattern::Event;
///
/// let env = eval_module("pattern = 42.0", ReplMode::Strict).unwrap();
/// let pattern = env.get("pattern").unwrap().as_number_pattern().unwrap();
/// let events = pattern.query_unit();
///
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0].value, 42.0);
/// ```
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

    pub(crate) fn every(self, period: i64, transform: BuiltinFn) -> Self {
        Self {
            pattern: PatternRuntime::Every {
                period,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn sometimes_with_site_salt(self, transform: BuiltinFn, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Sometimes {
                site_salt,
                transform,
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

    pub(crate) fn shift(self, offset: Rational) -> Self {
        Self {
            pattern: PatternRuntime::Shift {
                offset,
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
    /// overflows the evaluator's bounded rational arithmetic. For a fallible
    /// variant, use [`NumberPatternValue::try_query_unit`].
    #[must_use]
    pub fn query_unit(&self) -> Vec<Event<f64>> {
        self.try_query_unit().unwrap_or_else(|_err| {
            // In a live-coding environment, gracefully degrade instead of crashing the UI
            Vec::new()
        })
    }

    /// Fallible variant of [`NumberPatternValue::query_unit`].
    ///
    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    /// # Errors
    ///
    /// Returns an error if an internal runtime transform produces an invalid
    /// span or overflows the evaluator's bounded rational arithmetic.
    pub fn try_query_unit(&self) -> Result<Vec<Event<f64>>, EvalError> {
        self.try_query(&TimeSpan::unit())
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

    /// # Errors
    /// Returns `EvalError` if querying fails.
    pub fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<f64>>, EvalError> {
        self.pattern.try_query(span)
    }

    pub(crate) fn from_events(events: Vec<Event<f64>>) -> Self {
        Self {
            pattern: PatternRuntime::Stream(EventStream::new(events)),
        }
    }

    pub(crate) const fn rand(site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Rand { site_salt },
        }
    }
}

#[derive(Clone, Debug)]
enum PatternRuntime<T> {
    Cycle(CyclePattern<T>),
    Stream(EventStream<T>),
    ExplicitCycle {
        origin_cycle: i128,
        stream: EventStream<T>,
    },
    Stack(Vec<Self>),
    Every {
        period: i64,
        transform: BuiltinFn,
        inner: Box<Self>,
    },
    Sometimes {
        site_salt: u64,
        transform: BuiltinFn,
        inner: Box<Self>,
    },
    Fast {
        factor: i64,
        inner: Box<Self>,
    },
    Slow {
        factor: i64,
        inner: Box<Self>,
    },
    Shift {
        offset: Rational,
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
    Hpf {
        cutoff_hz: f64,
        inner: Box<Self>,
    },
    HpfPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Lpf {
        cutoff_hz: f64,
        inner: Box<Self>,
    },
    LpfPattern {
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
    Pitch {
        semitones: f64,
        inner: Box<Self>,
    },
    PitchPattern {
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
    SlicePattern {
        start_control: Box<PatternRuntime<f64>>,
        end_control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    SliceIdxPattern {
        control: Box<PatternRuntime<f64>>,
        segments: u32,
        inner: Box<Self>,
    },
    Rand {
        site_salt: u64,
    },
}

impl<T> PatternRuntime<T>
where
    T: PatternRuntimeValue,
{
    fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Cycle(pattern) => pattern
                .try_query(span)
                .map_err(|error| map_pattern_error(&error)),
            Self::Stream(stream) => stream
                .try_query(span)
                .map_err(|error| map_pattern_error(&error)),
            Self::ExplicitCycle { stream, .. } => query_explicit_cycle(stream, span),
            Self::Stack(layers) => query_stack(layers, span),
            Self::Every {
                period,
                transform,
                inner,
            } => query_every(inner, transform, *period, span),
            Self::Sometimes {
                site_salt,
                transform,
                inner,
            } => query_sometimes(inner, transform, *site_salt, span),
            Self::Fast { factor, inner } => query_fast(inner, *factor, span),
            Self::Slow { factor, inner } => query_slow(inner, *factor, span),
            Self::Shift { offset, inner } => query_shift(inner, offset, span),
            Self::Rev { inner } => query_rev(inner, span),
            Self::Gain { factor, inner } => {
                apply_value_mutation(inner, span, |value| value.adjust_gain(*factor))
            }
            Self::GainPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Gain)
            }
            Self::Hpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                value.adjust_hpf(*cutoff_hz);
            }),
            Self::HpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Hpf)
            }
            Self::Lpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                value.adjust_lpf(*cutoff_hz);
            }),
            Self::LpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Lpf)
            }
            Self::Pan { amount, inner } => {
                apply_value_mutation(inner, span, |value| value.adjust_pan(*amount))
            }
            Self::PanPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pan)
            }
            Self::Pitch { semitones, inner } => apply_value_mutation(inner, span, |value| {
                value.adjust_rate(semitones_to_rate_multiplier(*semitones));
            }),
            Self::PitchPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pitch)
            }
            Self::Rate { factor, inner } => {
                apply_value_mutation(inner, span, |value| value.adjust_rate(*factor))
            }
            Self::RatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Rate)
            }
            Self::Slice { start, end, inner } => apply_value_mutation(inner, span, |value| {
                value.adjust_slice(*start, *end);
            }),
            Self::SlicePattern {
                start_control,
                end_control,
                inner,
            } => apply_slice_pattern(inner, start_control, end_control, span),
            Self::SliceIdxPattern {
                control,
                segments,
                inner,
            } => apply_slice_idx_pattern(inner, control, *segments, span),
            Self::Rand { site_salt } => query_rand(*site_salt, span),
        }
    }
}

fn apply_value_mutation<T, F>(
    inner: &PatternRuntime<T>,
    span: &TimeSpan,
    mut mutate: F,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&mut T),
{
    let mut events = inner.try_query(span)?;
    for event in &mut events {
        mutate(&mut event.value);
    }
    Ok(events)
}

fn query_stack<T>(layers: &[PatternRuntime<T>], span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    // ⚡ Bolt: Pre-allocate vectors inside hot evaluation loops to avoid unnecessary heap reallocations.
    // We assume a modest default capacity proportional to the number of layers.
    let mut events = Vec::with_capacity(layers.len() * 4);
    for layer in layers {
        events.extend(layer.try_query(span)?);
    }
    sort_events(&mut events);
    Ok(events)
}

#[derive(Clone, Copy, Debug)]
enum ControlPatternKind {
    Gain,
    Hpf,
    Lpf,
    Pan,
    Pitch,
    Rate,
}

fn apply_event_fragments<T, F>(
    source_events: Vec<Event<T>>,
    control_event_lists: &[&[Event<f64>]],
    mut process_fragment: F,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&TimeSpan, &T) -> Result<Option<T>, EvalError>,
{
    let mut composed = Vec::with_capacity(source_events.len());
    for event in source_events {
        let Some(boundaries) = compute_event_fragment_boundaries(&event.part, control_event_lists)
        else {
            composed.push(event);
            continue;
        };

        for window in boundaries.windows(2) {
            let &[start, end] = window else {
                continue;
            };
            if start >= end {
                continue;
            }

            let part = build_span(start.clone(), end.clone())?;
            if let Some(value) = process_fragment(&part, &event.value)? {
                composed.push(Event {
                    whole: None,
                    part,
                    value,
                });
            }
        }
    }

    sort_events(&mut composed);
    Ok(composed)
}

fn apply_control_pattern<T>(
    inner: &PatternRuntime<T>,
    control: &PatternRuntime<f64>,
    span: &TimeSpan,
    kind: ControlPatternKind,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_events = inner.try_query(span)?;
    let control_events = control.try_query(span)?;
    validate_control_events(&control_events, kind)?;
    if control_events.is_empty() {
        return Ok(source_events);
    }

    apply_event_fragments(source_events, &[&control_events[..]], |part, value| {
        let mut new_value = value.clone();
        for control_event in &control_events {
            if spans_overlap(&control_event.part, part) {
                match kind {
                    ControlPatternKind::Gain => new_value.adjust_gain(control_event.value),
                    ControlPatternKind::Hpf => new_value.adjust_hpf(control_event.value),
                    ControlPatternKind::Lpf => new_value.adjust_lpf(control_event.value),
                    ControlPatternKind::Pan => new_value.adjust_pan(control_event.value),
                    ControlPatternKind::Pitch => {
                        new_value.adjust_rate(semitones_to_rate_multiplier(control_event.value));
                    }
                    ControlPatternKind::Rate => new_value.adjust_rate(control_event.value),
                }
            }
        }
        Ok(Some(new_value))
    })
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
            ControlPatternKind::Hpf => {
                if !event.value.is_finite() || event.value <= f64::EPSILON {
                    return Err(EvalError::new(
                        "`hpf` requires positive finite control values",
                    ));
                }
            }
            ControlPatternKind::Lpf => {
                if !event.value.is_finite() || event.value <= f64::EPSILON {
                    return Err(EvalError::new(
                        "`lpf` requires positive finite control values",
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
            ControlPatternKind::Pitch => {
                if !event.value.is_finite() {
                    return Err(EvalError::new(
                        "`pitch` requires finite numeric control values",
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

fn semitones_to_rate_multiplier(semitones: f64) -> f64 {
    (semitones / 12.0).exp2()
}

fn apply_slice_pattern<T>(
    inner: &PatternRuntime<T>,
    start_control: &PatternRuntime<f64>,
    end_control: &PatternRuntime<f64>,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_events = inner.try_query(span)?;
    let start_events = start_control.try_query(span)?;
    let end_events = end_control.try_query(span)?;
    validate_slice_endpoint_events(&start_events, "slice start")?;
    validate_slice_endpoint_events(&end_events, "slice end")?;
    if start_events.is_empty() && end_events.is_empty() {
        return Ok(source_events);
    }

    apply_event_fragments(
        source_events,
        &[&start_events[..], &end_events[..]],
        |part, value| {
            let mut relative_start = 0.0;
            let mut relative_end = 1.0;
            for control_event in &start_events {
                if spans_overlap(&control_event.part, part) {
                    relative_start = control_event.value;
                }
            }
            for control_event in &end_events {
                if spans_overlap(&control_event.part, part) {
                    relative_end = control_event.value;
                }
            }

            if relative_start >= relative_end {
                return Err(EvalError::new(
                    "`slice` requires control values with start < end",
                ));
            }

            let mut new_value = value.clone();
            new_value.adjust_slice(relative_start, relative_end);
            Ok(Some(new_value))
        },
    )
}

fn apply_slice_idx_pattern<T>(
    inner: &PatternRuntime<T>,
    control: &PatternRuntime<f64>,
    segments: u32,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_events = inner.try_query(span)?;
    let control_events = control.try_query(span)?;
    validate_slice_idx_control_events(&control_events, segments)?;
    if control_events.is_empty() {
        return Ok(source_events);
    }

    apply_event_fragments(source_events, &[&control_events[..]], |part, value| {
        let mut new_value = value.clone();
        for control_event in &control_events {
            if spans_overlap(&control_event.part, part) {
                let index = whole_number_from_slice_idx_value(control_event.value)?;
                let slice_start = f64::from(index) / f64::from(segments);
                let slice_end = f64::from(index.checked_add(1).ok_or_else(|| {
                    EvalError::new(
                        "`slice_idx` control index exceeded the supported evaluator range",
                    )
                })?) / f64::from(segments);
                new_value.adjust_slice(slice_start, slice_end);
            }
        }
        Ok(Some(new_value))
    })
}

fn validate_slice_endpoint_events(
    control_events: &[Event<f64>],
    context: &str,
) -> Result<(), EvalError> {
    for event in control_events {
        if !event.value.is_finite() || !(0.0..=1.0).contains(&event.value) {
            return Err(EvalError::new(format!(
                "`{context}` requires finite control values within [0, 1]"
            )));
        }
    }

    Ok(())
}

fn validate_slice_idx_control_events(
    control_events: &[Event<f64>],
    segments: u32,
) -> Result<(), EvalError> {
    for event in control_events {
        let index = whole_number_from_slice_idx_value(event.value)?;
        if index >= segments {
            return Err(EvalError::new(
                "`slice_idx` requires control values with index < segments",
            ));
        }
    }

    Ok(())
}

fn whole_number_from_slice_idx_value(value: f64) -> Result<u32, EvalError> {
    if !value.is_finite() || value < 0.0 || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(
            "`slice_idx` requires whole-number control values",
        ));
    }

    format!("{value:.0}").parse::<u32>().map_err(|_| {
        EvalError::new("`slice_idx` control value exceeded the supported evaluator range")
    })
}

fn query_rand<T>(site_salt: u64, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let numerator = span.start().numerator() as u64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let denominator = span.start().denominator() as u64;

    let mut state = site_salt ^ numerator.rotate_left(11) ^ denominator.rotate_left(23);
    state = state.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    state ^= state >> 30;
    state = state.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state ^= state >> 27;
    state = state.wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;

    #[allow(clippy::cast_precision_loss)]
    let value_f64 = (state as f64) / (u64::MAX as f64);
    let t_val = T::try_from_rand(value_f64)?;

    Ok(vec![Event {
        whole: None,
        part: span.clone(),
        value: t_val,
    }])
}

fn query_fast<T>(
    inner: &PatternRuntime<T>,
    factor: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_span = scale_span(span, factor, 1)?;
    let mut events = inner.try_query(&source_span)?;
    rescale_events(&mut events, 1, factor)?;
    Ok(events)
}

fn query_explicit_cycle<T>(
    stream: &EventStream<T>,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    // ⚡ Bolt: Pre-allocate vectors inside hot evaluation loops to avoid unnecessary heap reallocations.
    // A capacity of 8 is a reasonable starting point for cycle-based event sequences.
    let mut events = Vec::with_capacity(8);
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    for cycle in start_cycle..end_cycle {
        let repeated_cycle_span = cycle_span(cycle)?;
        let Some(query_slice) = clip_span(&repeated_cycle_span, span)? else {
            continue;
        };
        let cycle_offset = rational_from_parts(cycle, 1)?;
        let local_offset = rational_sub(&Rational::zero(), &cycle_offset)?;
        let local_query = translate_span(&query_slice, &local_offset)?;
        let mut cycle_events = stream
            .try_query(&local_query)
            .map_err(|error| map_pattern_error(&error))?;
        shift_events(&mut cycle_events, &cycle_offset)?;
        events.extend(cycle_events);
    }

    sort_events(&mut events);
    Ok(events)
}

fn query_every<T>(
    inner: &PatternRuntime<T>,
    transform: &BuiltinFn,
    period: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    let period = i128::from(period);
    query_transform_cycles(inner, transform, span, |cycle| {
        cycle.rem_euclid(period) == 0
    })
}

fn query_sometimes<T>(
    inner: &PatternRuntime<T>,
    transform: &BuiltinFn,
    site_salt: u64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    query_transform_cycles(inner, transform, span, |cycle| {
        sometimes_applies_on_cycle(cycle, site_salt)
    })
}

fn query_transform_cycles<T, F>(
    inner: &PatternRuntime<T>,
    transform: &BuiltinFn,
    span: &TimeSpan,
    mut should_transform: F,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(i128) -> bool,
{
    // ⚡ Bolt: Pre-allocate vectors inside hot evaluation loops to avoid unnecessary heap reallocations.
    let mut events = Vec::with_capacity(8);
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    for cycle in start_cycle..end_cycle {
        let cycle_span = cycle_span(cycle)?;
        let Some(query_slice) = clip_span(&cycle_span, span)? else {
            continue;
        };

        let absolute_cycle = absolute_cycle_for_runtime(inner, cycle)?;
        if should_transform(absolute_cycle) {
            let cycle_offset = rational_from_parts(cycle, 1)?;
            let local_offset = rational_sub(&Rational::zero(), &cycle_offset)?;
            let local_query = translate_span(&query_slice, &local_offset)?;
            let localized = localize_cycle_runtime(inner, cycle, absolute_cycle)?;
            let transformed =
                apply_builtin_function(transform, vec![T::into_runtime_value(localized)])?;
            let mut transformed_events =
                T::try_from_runtime_value(transformed)?.try_query(&local_query)?;
            shift_events(&mut transformed_events, &cycle_offset)?;
            events.extend(transformed_events);
        } else {
            events.extend(inner.try_query(&query_slice)?);
        }
    }

    sort_events(&mut events);
    Ok(events)
}

fn query_slow<T>(
    inner: &PatternRuntime<T>,
    factor: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_span = scale_span(span, 1, factor)?;
    let mut events = inner.try_query(&source_span)?;
    rescale_events(&mut events, factor, 1)?;
    Ok(events)
}

fn query_shift<T>(
    inner: &PatternRuntime<T>,
    offset: &Rational,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let inverse_offset = rational_sub(&Rational::zero(), offset)?;
    let source_span = translate_span(span, &inverse_offset)?;
    let mut events = inner.try_query(&source_span)?;
    shift_events(&mut events, offset)?;
    Ok(events)
}

fn query_rev<T>(inner: &PatternRuntime<T>, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    // ⚡ Bolt: Pre-allocate vectors inside hot evaluation loops to avoid unnecessary heap reallocations.
    let mut events = Vec::with_capacity(8);
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

fn localize_cycle_runtime<T>(
    inner: &PatternRuntime<T>,
    query_cycle: i128,
    origin_cycle: i128,
) -> Result<PatternRuntime<T>, EvalError>
where
    T: PatternRuntimeValue,
{
    let cycle_span = cycle_span(query_cycle)?;
    let cycle_offset = rational_from_parts(query_cycle, 1)?;
    let local_offset = rational_sub(&Rational::zero(), &cycle_offset)?;
    let mut localized_events = inner.try_query(&cycle_span)?;

    for event in &mut localized_events {
        event.part = translate_span(&event.part, &local_offset)?;
        event.whole = None;
    }

    Ok(PatternRuntime::ExplicitCycle {
        origin_cycle,
        stream: EventStream::new(localized_events),
    })
}

fn absolute_cycle_for_runtime<T>(
    runtime: &PatternRuntime<T>,
    cycle: i128,
) -> Result<i128, EvalError> {
    match runtime {
        PatternRuntime::ExplicitCycle { origin_cycle, .. } => origin_cycle
            .checked_add(cycle)
            .ok_or_else(|| EvalError::new("cycle index overflowed while localizing a pattern")),
        PatternRuntime::Every { inner, .. }
        | PatternRuntime::Sometimes { inner, .. }
        | PatternRuntime::Fast { inner, .. }
        | PatternRuntime::Slow { inner, .. }
        | PatternRuntime::Shift { inner, .. }
        | PatternRuntime::Rev { inner }
        | PatternRuntime::Gain { inner, .. }
        | PatternRuntime::GainPattern { inner, .. }
        | PatternRuntime::Hpf { inner, .. }
        | PatternRuntime::HpfPattern { inner, .. }
        | PatternRuntime::Lpf { inner, .. }
        | PatternRuntime::LpfPattern { inner, .. }
        | PatternRuntime::Pan { inner, .. }
        | PatternRuntime::PanPattern { inner, .. }
        | PatternRuntime::Pitch { inner, .. }
        | PatternRuntime::PitchPattern { inner, .. }
        | PatternRuntime::Rate { inner, .. }
        | PatternRuntime::RatePattern { inner, .. }
        | PatternRuntime::Slice { inner, .. }
        | PatternRuntime::SlicePattern { inner, .. }
        | PatternRuntime::SliceIdxPattern { inner, .. } => absolute_cycle_for_runtime(inner, cycle),
        PatternRuntime::Stack(layers) => layers
            .first()
            .map_or(Ok(cycle), |layer| absolute_cycle_for_runtime(layer, cycle)),
        PatternRuntime::Cycle(_) | PatternRuntime::Stream(_) | PatternRuntime::Rand { .. } => {
            Ok(cycle)
        }
    }
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

fn shift_events<T>(events: &mut [Event<T>], offset: &Rational) -> Result<(), EvalError> {
    for event in &mut *events {
        event.part = translate_span(&event.part, offset)?;
        if let Some(whole) = event.whole.take() {
            event.whole = Some(translate_span(&whole, offset)?);
        }
    }

    sort_events(events);
    Ok(())
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
    let start = max(span.start(), query.start());
    let end = min(span.end(), query.end());

    if start >= end {
        return Ok(None);
    }

    build_span(start.clone(), end.clone()).map(Some)
}

fn spans_overlap(a: &TimeSpan, b: &TimeSpan) -> bool {
    max(a.start(), b.start()) < min(a.end(), b.end())
}

fn compute_event_fragment_boundaries<'a>(
    source_span: &'a TimeSpan,
    control_event_lists: &[&'a [Event<f64>]],
) -> Option<Vec<&'a Rational>> {
    let capacity_estimate = 2 + control_event_lists
        .iter()
        .map(|list| list.len())
        .sum::<usize>()
        * 2;
    // PRE-ALLOCATE: prevents heap reallocations when collecting span boundaries.
    let mut boundaries = Vec::with_capacity(capacity_estimate);
    boundaries.push(source_span.start());
    boundaries.push(source_span.end());
    let mut has_overlap = false;

    for control_events in control_event_lists {
        for control_event in *control_events {
            let start = max(control_event.part.start(), source_span.start());
            let end = min(control_event.part.end(), source_span.end());
            if start < end {
                has_overlap = true;
                boundaries.push(start);
                boundaries.push(end);
            }
        }
    }

    if !has_overlap {
        return None;
    }

    boundaries.sort();
    boundaries.dedup();
    Some(boundaries)
}

fn scale_span(span: &TimeSpan, numerator: i64, denominator: i64) -> Result<TimeSpan, EvalError> {
    build_span(
        rational_mul_parts(span.start(), numerator, denominator)?,
        rational_mul_parts(span.end(), numerator, denominator)?,
    )
}

fn translate_span(span: &TimeSpan, offset: &Rational) -> Result<TimeSpan, EvalError> {
    build_span(
        rational_add(span.start(), offset)?,
        rational_add(span.end(), offset)?,
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

/// Calculates a deterministic, pseudorandom Boolean flag indicating if the
/// `sometimes` built-in function should apply its transformation to a given cycle.
///
/// This relies on the absolute cycle number and a lexical "site salt" to ensure
/// multiple usages of `sometimes` do not synchronize their coin flips.
#[doc(hidden)]
pub const fn sometimes_applies_on_cycle(cycle: i128, site_salt: u64) -> bool {
    let [
        b0,
        b1,
        b2,
        b3,
        b4,
        b5,
        b6,
        b7,
        b8,
        b9,
        b10,
        b11,
        b12,
        b13,
        b14,
        b15,
    ] = cycle.to_le_bytes();
    let lower = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]);
    let upper = u64::from_le_bytes([b8, b9, b10, b11, b12, b13, b14, b15]);
    let mut state = lower ^ upper.rotate_left(32) ^ site_salt.rotate_left(17);
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;
    (state & 1) != 0
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
    use super::{
        BuiltinFn, BuiltinKind, NumberPatternValue, SampleEvent, SamplePatternValue, Value,
        cycle_span, sometimes_applies_on_cycle,
    };
    use orpheus_pattern::{Event, PatternNode, Rational, TimeSpan};

    fn unary_transform(kind: BuiltinKind, args: Vec<Value>) -> BuiltinFn {
        let function = BuiltinFn::new(kind);
        match function.apply(args).unwrap() {
            Value::Function(function) => function,
            other => panic!("expected partially applied unary transform, got {other:?}"),
        }
    }

    fn unary_transform_with_site_salt(
        kind: BuiltinKind,
        args: Vec<Value>,
        site_salt: u64,
    ) -> BuiltinFn {
        let function = BuiltinFn::new(kind).with_site_salt(site_salt);
        match function.apply(args).unwrap() {
            Value::Function(function) => function,
            other => panic!("expected partially applied unary transform, got {other:?}"),
        }
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

    #[test]
    fn cycle_span_supports_indices_above_i64_range() {
        let start_cycle = i128::from(i64::MAX) + 1;
        let span = cycle_span(start_cycle).unwrap();

        assert_eq!(span.start().numerator(), start_cycle);
        assert_eq!(span.start().denominator(), 1);
        assert_eq!(span.end().numerator(), start_cycle + 1);
        assert_eq!(span.end().denominator(), 1);
    }

    #[test]
    fn every_uses_the_transformed_pattern_only_on_matching_cycles() {
        let base = SamplePatternValue::from_nodes(vec![
            PatternNode::atom(SampleEvent::named("bd")),
            PatternNode::atom(SampleEvent::named("sn")),
        ]);
        let transform = unary_transform(
            BuiltinKind::Fast,
            vec![Value::NumberPattern(NumberPatternValue::constant(2.0))],
        );
        let pattern = base.every(2, transform);
        let span = TimeSpan::new(Rational::zero(), Rational::new(2, 1).unwrap()).unwrap();
        let events = pattern.try_query(&span).unwrap();

        assert_eq!(
            events
                .iter()
                .map(|event| event.value.sample())
                .collect::<Vec<_>>(),
            vec!["bd", "sn", "bd", "sn", "bd", "sn"]
        );
        assert_eq!(events[0].part.start(), &Rational::zero());
        assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
        assert_eq!(events[3].part.start(), &Rational::new(3, 4).unwrap());
        assert_eq!(events[3].part.end(), &Rational::one());
        assert_eq!(events[4].part.start(), &Rational::one());
        assert_eq!(events[4].part.end(), &Rational::new(3, 2).unwrap());
        assert_eq!(events[5].part.start(), &Rational::new(3, 2).unwrap());
        assert_eq!(events[5].part.end(), &Rational::new(2, 1).unwrap());
    }

    #[test]
    fn every_fast_localizes_nested_cycle_varying_patterns() {
        let base = SamplePatternValue::from_nodes(vec![
            PatternNode::atom(SampleEvent::named("bd")),
            PatternNode::atom(SampleEvent::named("sn")),
            PatternNode::atom(SampleEvent::named("cp")),
        ]);
        let rev = unary_transform(BuiltinKind::Rev, Vec::new());
        let fast_two = unary_transform(
            BuiltinKind::Fast,
            vec![Value::NumberPattern(NumberPatternValue::constant(2.0))],
        );
        let nested = base.every(3, rev);
        let pattern = nested.every(2, fast_two);
        let events = pattern.try_query(&TimeSpan::unit()).unwrap();

        assert_eq!(
            events
                .iter()
                .map(|event| event.value.sample())
                .collect::<Vec<_>>(),
            vec!["cp", "sn", "bd", "cp", "sn", "bd"]
        );
    }

    #[test]
    fn nested_sometimes_inside_every_uses_absolute_cycle_numbers() {
        let site_salt = (0_u64..512)
            .find(|salt| {
                sometimes_applies_on_cycle(0, *salt) != sometimes_applies_on_cycle(2, *salt)
            })
            .expect("expected a salt that differentiates cycle 0 from cycle 2");
        let base = SamplePatternValue::from_nodes(vec![
            PatternNode::atom(SampleEvent::named("bd")),
            PatternNode::atom(SampleEvent::named("sn")),
        ]);
        let rev = unary_transform(BuiltinKind::Rev, Vec::new());
        let sometimes_rev = unary_transform_with_site_salt(
            BuiltinKind::Sometimes,
            vec![Value::Function(rev)],
            site_salt,
        );
        let pattern = base.every(2, sometimes_rev);
        let span = TimeSpan::new(Rational::zero(), Rational::new(4, 1).unwrap()).unwrap();
        let events = pattern.try_query(&span).unwrap();
        let cycle_zero = sample_names_in_cycle(&events, 0);
        let cycle_two = sample_names_in_cycle(&events, 2);

        assert_eq!(
            cycle_zero,
            if sometimes_applies_on_cycle(0, site_salt) {
                vec!["sn".to_owned(), "bd".to_owned()]
            } else {
                vec!["bd".to_owned(), "sn".to_owned()]
            }
        );
        assert_eq!(
            cycle_two,
            if sometimes_applies_on_cycle(2, site_salt) {
                vec!["sn".to_owned(), "bd".to_owned()]
            } else {
                vec!["bd".to_owned(), "sn".to_owned()]
            }
        );
        assert_ne!(cycle_zero, cycle_two);
    }

    #[test]
    fn sometimes_uses_a_deterministic_cycle_selection() {
        let base = SamplePatternValue::from_nodes(vec![
            PatternNode::atom(SampleEvent::named("bd")),
            PatternNode::atom(SampleEvent::named("sn")),
        ]);
        let rev = unary_transform(BuiltinKind::Rev, Vec::new());
        let pattern = base.sometimes_with_site_salt(rev, 0);
        let span = TimeSpan::new(Rational::zero(), Rational::new(4, 1).unwrap()).unwrap();

        let first = pattern.try_query(&span).unwrap();
        let second = pattern.try_query(&span).unwrap();

        assert_eq!(first, second);
        assert_eq!(
            first
                .iter()
                .map(|event| event.value.sample())
                .collect::<Vec<_>>(),
            vec!["sn", "bd", "sn", "bd", "bd", "sn", "sn", "bd"]
        );
    }
}
