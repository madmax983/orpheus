#![allow(clippy::large_enum_variant)]
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
use std::collections::BTreeMap;
use std::sync::Arc;

use orpheus_pattern::{CyclePattern, Event, EventStream, PatternNode, Rational, TimeSpan};

use crate::{
    ReplMode,
    ast::Expr,
    eval::{EvalError, apply_function_value},
    pedal::PedalValue,
};

/// Identifies which core built-in function is being represented.
///
/// These variants map exactly to the standard Orpheus primitive transformations
/// available in the base language.
#[derive(Clone, Copy, Debug)]
pub enum BuiltinKind {
    Every,
    When,
    Sometimes,
    Within,
    Mask,
    Strum,
    Roll,
    Arp,
    Invert,
    Drop,
    Chord,
    Euclid,
    Lsystem,
    Wolfram,
    PitchClassSet,
    Degrees,
    Fast,
    Slow,
    Shift,
    Rev,
    Gain,
    Delay,
    DelayTime,
    DelayFeedback,
    Hpf,
    Lpf,
    Reverb,
    ReverbRoom,
    ReverbDamp,
    Cutoff,
    Chorus,
    ChorusDepth,
    ChorusRate,
    Compressor,
    CompressorThreshold,
    CompressorRatio,
    Res,
    Drive,
    Pw,
    Pan,
    Pitch,
    Transpose,
    Sample,
    Onset,
    Rate,
    Slice,
    SliceIdx,
    Rand,
    Jux,
    Through,
    MidiCc,
    Chaos,
    Palindrome,
    Tuning,
    LoadScl,
    Tune,
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

/// A user-defined top-level curried function with captured bindings.
#[derive(Clone, Debug)]
pub struct UserFn {
    pub(crate) mode: ReplMode,
    pub(crate) remaining_params: Vec<String>,
    pub(crate) body: Expr,
    pub(crate) captured_bindings: BTreeMap<String, Value>,
    pub(crate) expr_site_salts: BTreeMap<usize, u64>,
    pub(crate) depth: usize,
}

/// A callable runtime value, either builtin or user-defined.
#[derive(Clone, Debug)]
pub enum FunctionValue {
    /// A core primitive transformation provided by the language standard library.
    Builtin(BuiltinFn),
    /// A custom function defined by the user in the REPL or a script file.
    User(UserFn),
}

/// A gate pattern passed to structural combinators like `mask`.
#[derive(Clone, Debug)]
pub enum GatePatternValue {
    Sample(SamplePatternValue),
    Number(NumberPatternValue),
}

/// Indicates the order in which an arpeggiator traverses the notes of a chord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArpDirectionValue {
    Up,
    Down,
    PingPong,
}

const IONIAN_INTERVALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const DORIAN_INTERVALS: [i32; 7] = [0, 2, 3, 5, 7, 9, 10];
const PHRYGIAN_INTERVALS: [i32; 7] = [0, 1, 3, 5, 7, 8, 10];
const MIXOLYDIAN_INTERVALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 10];
const AEOLIAN_INTERVALS: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const MINOR_PENTATONIC_INTERVALS: [i32; 5] = [0, 3, 5, 7, 10];

/// A constant collection of musical pitches (a scale or chord) used as a musical palette.
///
/// This represents relative scale intervals from a root of `0`. For example,
/// a major scale is `[0, 2, 4, 5, 7, 9, 11]`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::eval_module;
/// use orpheus_lang::ReplMode;
///
/// let env = eval_module("scale = ionian", ReplMode::Strict).unwrap();
/// let val = env.get("scale").unwrap();
/// assert!(val.as_pitch_class_set().is_some());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PitchClassSetValue {
    pitch_classes: Vec<i32>,
}

impl PitchClassSetValue {
    pub(crate) fn new(pitch_classes: Vec<i32>) -> Result<Self, EvalError> {
        if pitch_classes.first().copied() != Some(0) {
            return Err(EvalError::new(
                "`pitch_class_set` requires the first pitch class to be 0",
            ));
        }

        let mut previous = None;
        for pitch_class in &pitch_classes {
            if !(0..=11).contains(pitch_class) {
                return Err(EvalError::new(
                    "`pitch_class_set` requires pitch classes within [0, 11]",
                ));
            }

            if let Some(previous) = previous
                && *pitch_class <= previous
            {
                return Err(EvalError::new(
                    "`pitch_class_set` requires strictly increasing pitch classes",
                ));
            }
            previous = Some(*pitch_class);
        }

        Ok(Self { pitch_classes })
    }

    fn from_slice(pitch_classes: &[i32]) -> Self {
        Self {
            pitch_classes: pitch_classes.to_vec(),
        }
    }

    pub(crate) fn ionian() -> Self {
        Self::from_slice(&IONIAN_INTERVALS)
    }

    pub(crate) fn dorian() -> Self {
        Self::from_slice(&DORIAN_INTERVALS)
    }

    pub(crate) fn phrygian() -> Self {
        Self::from_slice(&PHRYGIAN_INTERVALS)
    }

    pub(crate) fn mixolydian() -> Self {
        Self::from_slice(&MIXOLYDIAN_INTERVALS)
    }

    pub(crate) fn aeolian() -> Self {
        Self::from_slice(&AEOLIAN_INTERVALS)
    }

    pub(crate) fn minor_pentatonic() -> Self {
        Self::from_slice(&MINOR_PENTATONIC_INTERVALS)
    }

    pub(crate) fn intervals(&self) -> &[i32] {
        &self.pitch_classes
    }
}

/// The period at which a scale repeats, expressed as a frequency ratio.
///
/// Phase 1 only supports octaves (2/1). The [`TuningValue::new`] constructor
/// rejects other periods. The field is still stored so Bohlen-Pierce and
/// other non-octave-periodic scales can be unlocked in a later phase without
/// a data-model migration.
pub const TUNING_OCTAVE_PERIOD: f64 = 2.0;

/// A static microtonal tuning table used to override 12-TET pitch math.
///
/// `ratios[0]` is always `1.0` (the implicit 1/1 root). Every other entry is a
/// ratio strictly greater than the previous and strictly less than `period`.
/// Out-of-table steps wrap via `ratios[step mod N] * period^floor(step/N)`.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::{eval_module, ReplMode};
///
/// let env = eval_module("t = tuning(1.0 1.125 1.25 1.5 2.0)", ReplMode::Loose).unwrap();
/// let tuning = env.get("t").unwrap().as_tuning().unwrap();
/// assert_eq!(tuning.ratios().len(), 4);
/// ```
#[derive(Clone, Debug)]
pub struct TuningValue {
    name: std::sync::Arc<str>,
    ratios: std::sync::Arc<[f64]>,
    period: f64,
    ref_semitone: i32,
}

impl TuningValue {
    /// Constructs a tuning from a list of ratios and a period.
    ///
    /// # Errors
    ///
    /// Returns an [`EvalError`] if the input violates any of:
    /// - non-empty and `ratios[0] == 1.0`
    /// - ratios are strictly increasing
    /// - every ratio lies in `[1.0, period)`
    /// - `period == 2.0` in Phase 1
    pub(crate) fn new(
        name: impl Into<std::sync::Arc<str>>,
        ratios: Vec<f64>,
        period: f64,
    ) -> Result<Self, EvalError> {
        if ratios.is_empty() {
            return Err(EvalError::new("`tuning` requires at least one ratio"));
        }
        if (period - TUNING_OCTAVE_PERIOD).abs() > f64::EPSILON {
            return Err(EvalError::new(
                "`tuning` period must be 2.0 (octave) in Phase 1",
            ));
        }
        if !period.is_finite() || period <= 1.0 {
            return Err(EvalError::new(
                "`tuning` period must be a finite value greater than 1.0",
            ));
        }
        if (ratios[0] - 1.0).abs() > f64::EPSILON {
            return Err(EvalError::new(
                "`tuning` ratios must start at 1.0 (1/1 root)",
            ));
        }
        let mut previous = f64::NEG_INFINITY;
        for ratio in &ratios {
            if !ratio.is_finite() || *ratio <= 0.0 {
                return Err(EvalError::new("`tuning` requires finite positive ratios"));
            }
            if *ratio <= previous {
                return Err(EvalError::new(
                    "`tuning` requires strictly monotone increasing ratios",
                ));
            }
            previous = *ratio;
        }
        if *ratios.last().unwrap() >= period + f64::EPSILON {
            return Err(EvalError::new(
                "`tuning` ratios must be strictly less than the period",
            ));
        }
        Ok(Self {
            name: name.into(),
            ratios: ratios.into(),
            period,
            ref_semitone: 0,
        })
    }

    /// The scale's display name (e.g., `"just_intonation"` or an anonymous label).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The stored scale steps. `ratios[0]` is always `1.0`.
    #[must_use]
    pub fn ratios(&self) -> &[f64] {
        &self.ratios
    }

    /// The repetition interval (2.0 = octave).
    #[must_use]
    pub const fn period(&self) -> f64 {
        self.period
    }

    /// The semitone aligned with `ratios[0]`. Defaults to `0`.
    #[must_use]
    pub const fn ref_semitone(&self) -> i32 {
        self.ref_semitone
    }

    fn as_table(&self) -> TuningTable {
        TuningTable {
            ratios: std::sync::Arc::clone(&self.ratios),
            period: self.period,
            ref_semitone: self.ref_semitone,
        }
    }
}

/// A cheaply-cloneable shared handle to a tuning's scale steps.
///
/// Embedded in `TunedPitch` / `TunedPitchPattern` `PatternRuntime` nodes.
/// All fields are immutable once constructed; cloning only bumps an `Arc`
/// refcount.
#[derive(Clone, Debug)]
struct TuningTable {
    ratios: std::sync::Arc<[f64]>,
    period: f64,
    ref_semitone: i32,
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
    /// A sequenced pattern of audio sample identifiers or parameters.
    SamplePattern(SamplePatternValue),
    /// A sequenced pattern of numerical values (e.g., gains, tempos).
    NumberPattern(NumberPatternValue),
    /// A constant value indicating the direction of an arpeggiator.
    ArpDirection(ArpDirectionValue),
    /// A constant collection of musical pitches (a scale or chord).
    PitchClassSet(PitchClassSetValue),
    /// An executable function closure, either built-in or user-defined.
    Function(FunctionValue),
    /// A validated pedal graph ready for later lowering.
    Pedal(PedalValue),
    /// A static microtonal tuning table used by `tune(...)` to override 12-TET.
    Tuning(TuningValue),
    /// A primitive string value.
    String(std::sync::Arc<str>),
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
            Self::NumberPattern(_)
            | Self::ArpDirection(_)
            | Self::PitchClassSet(_)
            | Self::Function(_)
            | Self::Pedal(_)
            | Self::Tuning(_)
            | Self::String(_) => None,
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
            Self::SamplePattern(_)
            | Self::ArpDirection(_)
            | Self::PitchClassSet(_)
            | Self::Tuning(_)
            | Self::Function(_)
            | Self::Pedal(_)
            | Self::String(_) => None,
        }
    }

    /// Attempts to unwrap the value into a concrete pitch class set.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    ///
    /// let val = Value::String("foo".into());
    /// assert!(val.as_pitch_class_set().is_none());
    /// ```
    #[must_use]
    pub const fn as_pitch_class_set(&self) -> Option<&PitchClassSetValue> {
        match self {
            Self::PitchClassSet(pitch_class_set) => Some(pitch_class_set),
            Self::SamplePattern(_)
            | Self::NumberPattern(_)
            | Self::ArpDirection(_)
            | Self::Function(_)
            | Self::Pedal(_)
            | Self::Tuning(_)
            | Self::String(_) => None,
        }
    }

    /// Attempts to unwrap the value into a tuning table.
    ///
    /// Returns `None` for any other variant. Used by the `tune` builtin and
    /// by application code that reads a tuning bound via
    /// `t = tuning(...)` or `t = load_scl(...)`.
    #[must_use]
    pub const fn as_tuning(&self) -> Option<&TuningValue> {
        match self {
            Self::Tuning(tuning) => Some(tuning),
            Self::SamplePattern(_)
            | Self::NumberPattern(_)
            | Self::ArpDirection(_)
            | Self::PitchClassSet(_)
            | Self::Function(_)
            | Self::Pedal(_)
            | Self::String(_) => None,
        }
    }

    /// Attempts to unwrap the value into a concrete arp direction.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    ///
    /// let val = Value::String("foo".into());
    /// assert!(val.as_arp_direction().is_none());
    /// ```
    #[must_use]
    pub const fn as_arp_direction(&self) -> Option<ArpDirectionValue> {
        match self {
            Self::ArpDirection(direction) => Some(*direction),
            Self::SamplePattern(_)
            | Self::NumberPattern(_)
            | Self::PitchClassSet(_)
            | Self::Function(_)
            | Self::Pedal(_)
            | Self::Tuning(_)
            | Self::String(_) => None,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn as_pedal(&self) -> Option<&PedalValue> {
        match self {
            Self::Pedal(pedal) => Some(pedal),
            Self::SamplePattern(_)
            | Self::NumberPattern(_)
            | Self::ArpDirection(_)
            | Self::PitchClassSet(_)
            | Self::Function(_)
            | Self::Tuning(_)
            | Self::String(_) => None,
        }
    }

    /// Extracts a human-readable description of this value's underlying type.
    ///
    /// This string maps the internal enum variant (like `SamplePattern`) to its lowercase
    /// noun equivalent (like `"sample pattern"`), which is heavily used when formatting friendly
    /// runtime type mismatch error messages to the user.
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
            Self::ArpDirection(_) => "arp direction",
            Self::PitchClassSet(_) => "pitch class set",
            Self::Function(_) => "function",
            Self::Pedal(_) => "pedal",
            Self::Tuning(_) => "tuning",
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
    /// ⚡ Bolt: `Arc<str>` is used instead of `Box<str>` because pattern transformations
    /// frequently clone `SampleEvent`. Using `Arc` replaces deep string allocations with
    /// a fast, atomic reference count increment while remaining `Send + Sync`.
    sample: std::sync::Arc<str>,
    gain: f64,
    hpf_cutoff_hz: Option<f64>,
    lpf_cutoff_hz: Option<f64>,
    resonance: f64,
    drive: f64,
    pulse_width: f64,
    pan: f64,
    rate: f64,
    delay_mix: f64,
    delay_time: f64,
    delay_feedback: f64,
    reverb_mix: f64,
    reverb_room: f64,
    reverb_damp: f64,
    chorus_mix: f64,
    chorus_depth: f64,
    chorus_rate: f64,
    compressor_mix: f64,
    compressor_threshold: f64,
    compressor_ratio: f64,
    onset_index: Option<u32>,
    slice_start: f64,
    slice_end: f64,
    pedal_program: Option<Arc<orpheus_dsp::PedalProgram>>,
}

impl SampleEvent {
    pub(crate) fn named(sample: &str) -> Self {
        Self {
            sample: sample.into(),
            gain: 1.0,
            hpf_cutoff_hz: None,
            lpf_cutoff_hz: None,
            resonance: 0.2,
            drive: 1.0,
            pulse_width: 0.5,
            pan: 0.0,
            rate: 1.0,
            delay_mix: 0.0,
            delay_time: 0.125,
            delay_feedback: 0.35,
            reverb_mix: 0.0,
            reverb_room: 0.75,
            reverb_damp: 0.35,
            chorus_mix: 0.0,
            chorus_depth: 0.4,
            chorus_rate: 0.5,
            compressor_mix: 0.0,
            compressor_threshold: 0.5,
            compressor_ratio: 4.0,
            onset_index: None,
            slice_start: 0.0,
            slice_end: 1.0,
            pedal_program: None,
        }
    }

    /// The string identifier of the raw audio sample.
    ///
    /// This is typically the name of a `.wav` file mapped in an active `SampleBank`.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    /// use orpheus_lang::eval_module;
    /// use orpheus_lang::ReplMode;
    ///
    /// let env = eval_module("s = bd", ReplMode::Strict).unwrap();
    /// let val = env.get("s").unwrap();
    /// let pat = val.as_sample_pattern().unwrap();
    /// let events = pat.query_unit().unwrap();
    /// assert_eq!(events[0].value.sample(), "bd");
    /// ```
    #[must_use]
    pub fn sample(&self) -> &str {
        self.sample.as_ref()
    }

    /// The amplitude multiplier applied to this event.
    ///
    /// By default, events have a gain of `1.0`. Value `0.0` is completely silent.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    /// use orpheus_lang::eval_module;
    /// use orpheus_lang::ReplMode;
    ///
    /// let env = eval_module("s = bd |> gain(0.5)", ReplMode::Strict).unwrap();
    /// let val = env.get("s").unwrap();
    /// let pat = val.as_sample_pattern().unwrap();
    /// let events = pat.query_unit().unwrap();
    /// assert_eq!(events[0].value.gain(), 0.5);
    /// ```
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// The high-pass filter cutoff frequency in Hertz, if one is active.
    ///
    /// Frequencies below this threshold will be attenuated during DSP playback.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    /// use orpheus_lang::eval_module;
    /// use orpheus_lang::ReplMode;
    ///
    /// let env = eval_module("s = bd |> hpf(400.0)", ReplMode::Strict).unwrap();
    /// let val = env.get("s").unwrap();
    /// let pat = val.as_sample_pattern().unwrap();
    /// let events = pat.query_unit().unwrap();
    /// assert_eq!(events[0].value.hpf_cutoff_hz(), Some(400.0));
    /// ```
    #[must_use]
    pub const fn hpf_cutoff_hz(&self) -> Option<f64> {
        self.hpf_cutoff_hz
    }

    /// The low-pass filter cutoff frequency in Hertz, if one is active.
    ///
    /// Frequencies above this threshold will be attenuated during DSP playback.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::Value;
    /// use orpheus_lang::eval_module;
    /// use orpheus_lang::ReplMode;
    ///
    /// let env = eval_module("s = bd |> lpf(200.0)", ReplMode::Strict).unwrap();
    /// let val = env.get("s").unwrap();
    /// let pat = val.as_sample_pattern().unwrap();
    /// let events = pat.query_unit().unwrap();
    /// assert_eq!(events[0].value.lpf_cutoff_hz(), Some(200.0));
    /// ```
    #[must_use]
    pub const fn lpf_cutoff_hz(&self) -> Option<f64> {
        self.lpf_cutoff_hz
    }

    /// The ladder filter resonance amount for synth-style events.
    #[must_use]
    pub const fn resonance(&self) -> f64 {
        self.resonance
    }

    /// The synth drive amount.
    #[must_use]
    pub const fn drive(&self) -> f64 {
        self.drive
    }

    /// The pulse oscillator width, in the open interval `(0, 1)`.
    #[must_use]
    pub const fn pulse_width(&self) -> f64 {
        self.pulse_width
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

    /// The insert delay wet mix.
    #[must_use]
    pub const fn delay_mix(&self) -> f64 {
        self.delay_mix
    }

    /// The insert delay time expressed in cycle units.
    #[must_use]
    pub const fn delay_time(&self) -> f64 {
        self.delay_time
    }

    /// The insert delay feedback coefficient.
    #[must_use]
    pub const fn delay_feedback(&self) -> f64 {
        self.delay_feedback
    }

    /// The insert reverb wet mix.
    #[must_use]
    pub const fn reverb_mix(&self) -> f64 {
        self.reverb_mix
    }

    /// The insert reverb room-size control.
    #[must_use]
    pub const fn reverb_room(&self) -> f64 {
        self.reverb_room
    }

    /// The insert reverb damping control.
    #[must_use]
    pub const fn reverb_damp(&self) -> f64 {
        self.reverb_damp
    }

    /// The insert chorus wet mix.
    #[must_use]
    pub const fn chorus_mix(&self) -> f64 {
        self.chorus_mix
    }

    /// The insert chorus depth control.
    #[must_use]
    pub const fn chorus_depth(&self) -> f64 {
        self.chorus_depth
    }

    /// The insert chorus modulation rate.
    #[must_use]
    pub const fn chorus_rate(&self) -> f64 {
        self.chorus_rate
    }

    /// The insert compressor wet mix.
    #[must_use]
    pub const fn compressor_mix(&self) -> f64 {
        self.compressor_mix
    }

    /// The insert compressor threshold.
    #[must_use]
    pub const fn compressor_threshold(&self) -> f64 {
        self.compressor_threshold
    }

    /// The insert compressor ratio.
    #[must_use]
    pub const fn compressor_ratio(&self) -> f64 {
        self.compressor_ratio
    }

    /// The transient slice index selected for later sample-bank resolution.
    #[must_use]
    pub const fn onset_index(&self) -> Option<u32> {
        self.onset_index
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

    #[doc(hidden)]
    #[must_use]
    pub const fn pedal_program(&self) -> Option<&Arc<orpheus_dsp::PedalProgram>> {
        self.pedal_program.as_ref()
    }

    fn clone_with(&self, mutate: impl FnOnce(&mut Self)) -> Self {
        let mut cloned = self.clone();
        mutate(&mut cloned);
        cloned
    }
}

trait PatternValueTransform: Sized {
    fn adjust_gain(&self, factor: f64) -> Self;
    fn adjust_delay_mix(&self, mix: f64) -> Self;
    fn adjust_delay_time(&self, time: f64) -> Self;
    fn adjust_delay_feedback(&self, feedback: f64) -> Self;
    fn adjust_hpf(&self, cutoff_hz: f64) -> Self;
    fn adjust_lpf(&self, cutoff_hz: f64) -> Self;
    fn adjust_reverb_mix(&self, mix: f64) -> Self;
    fn adjust_reverb_room(&self, room: f64) -> Self;
    fn adjust_reverb_damp(&self, damp: f64) -> Self;
    fn adjust_chorus_mix(&self, mix: f64) -> Self;
    fn adjust_chorus_depth(&self, depth: f64) -> Self;
    fn adjust_chorus_rate(&self, rate: f64) -> Self;
    fn adjust_compressor_mix(&self, mix: f64) -> Self;
    fn adjust_compressor_threshold(&self, threshold: f64) -> Self;
    fn adjust_compressor_ratio(&self, ratio: f64) -> Self;
    fn adjust_resonance(&self, resonance: f64) -> Self;
    fn adjust_drive(&self, drive: f64) -> Self;
    fn adjust_pulse_width(&self, pulse_width: f64) -> Self;
    fn adjust_pan(&self, amount: f64) -> Self;
    fn adjust_rate(&self, factor: f64) -> Self;
    fn adjust_onset(&self, onset_index: u32) -> Self;
    fn adjust_slice(&self, start: f64, end: f64) -> Self;
    fn attach_pedal_program(&self, pedal_program: &Arc<orpheus_dsp::PedalProgram>) -> Self;
    fn map_degrees(&self, collection: &PitchClassSetValue) -> Result<Self, EvalError>;
    fn transpose_semitones(&self, semitones: f64) -> Result<Self, EvalError>;
}

trait PatternRuntimeValue: Clone + PatternValueTransform + Send + Sync + fmt::Debug + Sized {
    fn is_finite_numeric(&self) -> bool {
        true
    }
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value;
    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError>;
    fn try_from_rand(value: f64) -> Result<Self, EvalError>;
    fn roll_events(events: Vec<Event<Self>>, steps: u32) -> Result<Vec<Event<Self>>, EvalError>;
    fn strum_events(events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError>;
    fn arp_events(
        events: Vec<Event<Self>>,
        steps: u32,
        direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError>;
    fn invert_events(events: Vec<Event<Self>>, count: u32) -> Result<Vec<Event<Self>>, EvalError>;
    fn drop_events(events: Vec<Event<Self>>, count: u32) -> Result<Vec<Event<Self>>, EvalError>;
}

impl PatternValueTransform for SampleEvent {
    fn adjust_gain(&self, factor: f64) -> Self {
        self.clone_with(|event| event.gain *= factor)
    }

    fn adjust_delay_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.delay_mix = mix)
    }

    fn adjust_delay_time(&self, time: f64) -> Self {
        self.clone_with(|event| event.delay_time = time)
    }

    fn adjust_delay_feedback(&self, feedback: f64) -> Self {
        self.clone_with(|event| event.delay_feedback = feedback)
    }

    fn adjust_hpf(&self, cutoff_hz: f64) -> Self {
        self.clone_with(|event| event.hpf_cutoff_hz = Some(cutoff_hz))
    }

    fn adjust_lpf(&self, cutoff_hz: f64) -> Self {
        self.clone_with(|event| event.lpf_cutoff_hz = Some(cutoff_hz))
    }

    fn adjust_reverb_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.reverb_mix = mix)
    }

    fn adjust_reverb_room(&self, room: f64) -> Self {
        self.clone_with(|event| event.reverb_room = room)
    }

    fn adjust_reverb_damp(&self, damp: f64) -> Self {
        self.clone_with(|event| event.reverb_damp = damp)
    }

    fn adjust_chorus_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.chorus_mix = mix)
    }

    fn adjust_chorus_depth(&self, depth: f64) -> Self {
        self.clone_with(|event| event.chorus_depth = depth)
    }

    fn adjust_chorus_rate(&self, rate: f64) -> Self {
        self.clone_with(|event| event.chorus_rate = rate)
    }

    fn adjust_compressor_mix(&self, mix: f64) -> Self {
        self.clone_with(|event| event.compressor_mix = mix)
    }

    fn adjust_compressor_threshold(&self, threshold: f64) -> Self {
        self.clone_with(|event| event.compressor_threshold = threshold)
    }

    fn adjust_compressor_ratio(&self, ratio: f64) -> Self {
        self.clone_with(|event| event.compressor_ratio = ratio)
    }

    fn adjust_resonance(&self, resonance: f64) -> Self {
        self.clone_with(|event| event.resonance = resonance)
    }

    fn adjust_drive(&self, drive: f64) -> Self {
        self.clone_with(|event| event.drive = drive)
    }

    fn adjust_pulse_width(&self, pulse_width: f64) -> Self {
        self.clone_with(|event| event.pulse_width = pulse_width)
    }

    fn adjust_pan(&self, amount: f64) -> Self {
        self.clone_with(|event| event.pan = (event.pan + amount).clamp(-1.0, 1.0))
    }

    fn adjust_rate(&self, factor: f64) -> Self {
        self.clone_with(|event| event.rate *= factor)
    }

    fn adjust_onset(&self, onset_index: u32) -> Self {
        self.clone_with(|event| event.onset_index = Some(onset_index))
    }

    fn adjust_slice(&self, start: f64, end: f64) -> Self {
        self.clone_with(|event| {
            let current_start = event.slice_start;
            let current_range = event.slice_end - current_start;
            event.slice_start = current_range.mul_add(start, current_start);
            event.slice_end = current_range.mul_add(end, current_start);
        })
    }

    fn attach_pedal_program(&self, pedal_program: &Arc<orpheus_dsp::PedalProgram>) -> Self {
        self.clone_with(|event| event.pedal_program = Some(pedal_program.clone()))
    }

    fn map_degrees(&self, _collection: &PitchClassSetValue) -> Result<Self, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: degree mapping only applies to number patterns",
        ))
    }

    fn transpose_semitones(&self, _semitones: f64) -> Result<Self, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: transposition only applies to number patterns",
        ))
    }
}

impl PatternValueTransform for f64 {
    fn adjust_gain(&self, _factor: f64) -> Self {
        *self
    }

    fn adjust_delay_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_delay_time(&self, _time: f64) -> Self {
        *self
    }

    fn adjust_delay_feedback(&self, _feedback: f64) -> Self {
        *self
    }

    fn adjust_hpf(&self, _cutoff_hz: f64) -> Self {
        *self
    }

    fn adjust_lpf(&self, _cutoff_hz: f64) -> Self {
        *self
    }

    fn adjust_reverb_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_reverb_room(&self, _room: f64) -> Self {
        *self
    }

    fn adjust_reverb_damp(&self, _damp: f64) -> Self {
        *self
    }

    fn adjust_chorus_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_chorus_depth(&self, _depth: f64) -> Self {
        *self
    }

    fn adjust_chorus_rate(&self, _rate: f64) -> Self {
        *self
    }

    fn adjust_compressor_mix(&self, _mix: f64) -> Self {
        *self
    }

    fn adjust_compressor_threshold(&self, _threshold: f64) -> Self {
        *self
    }

    fn adjust_compressor_ratio(&self, _ratio: f64) -> Self {
        *self
    }

    fn adjust_resonance(&self, _resonance: f64) -> Self {
        *self
    }

    fn adjust_drive(&self, _drive: f64) -> Self {
        *self
    }

    fn adjust_pulse_width(&self, _pulse_width: f64) -> Self {
        *self
    }

    fn adjust_pan(&self, _amount: f64) -> Self {
        *self
    }

    fn adjust_rate(&self, _factor: f64) -> Self {
        *self
    }

    fn adjust_onset(&self, _onset_index: u32) -> Self {
        *self
    }

    fn adjust_slice(&self, _start: f64, _end: f64) -> Self {
        *self
    }

    fn attach_pedal_program(&self, _pedal_program: &Arc<orpheus_dsp::PedalProgram>) -> Self {
        *self
    }

    fn map_degrees(&self, collection: &PitchClassSetValue) -> Result<Self, EvalError> {
        let degree = whole_number_from_degree_value(*self)?;
        map_degree_to_semitones(degree, collection)
    }

    fn transpose_semitones(&self, semitones: f64) -> Result<Self, EvalError> {
        let value = *self + semitones;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(EvalError::new(
                "`transpose` produced a non-finite numeric value",
            ))
        }
    }
}

impl PatternRuntimeValue for SampleEvent {
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value {
        Value::SamplePattern(SamplePatternValue { pattern })
    }

    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError> {
        match value {
            Value::SamplePattern(pattern) => Ok(pattern.pattern),
            Value::NumberPattern(_)
            | Value::ArpDirection(_)
            | Value::PitchClassSet(_)
            | Value::Function(_)
            | Value::Pedal(_)
            | Value::Tuning(_)
            | Value::String(_) => Err(EvalError::new(
                "transform returned an incompatible value; expected Pattern<Sample>",
            )),
        }
    }

    fn try_from_rand(_value: f64) -> Result<Self, EvalError> {
        Err(EvalError::new("rand only produces numbers"))
    }

    /// ⚡ Bolt: Uses slice bounds (`&events[start_index..index]`) instead of allocating a temporary `cluster` Vec for every group of events with the same span, eliminating redundant heap allocations in the hot evaluation loop.
    /// ⚡ Bolt: Uses slice bounds (`&events[start_index].part`) instead of cloning `TimeSpan`
    /// for every group of events with the same span, eliminating redundant memory copying
    /// in the hot evaluation loop.
    fn roll_events(
        mut events: Vec<Event<Self>>,
        steps: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut rolled = Vec::with_capacity(events.len());
        let mut index = 0;

        while index < events.len() {
            let span = events[index].part;
            let start_index = index;
            while index < events.len() && events[index].part == span {
                index += 1;
            }

            let cluster_events = roll_event_cluster(&events[start_index..index], steps)?;
            if rolled.len() + cluster_events.len() > 100_000 {
                return Err(EvalError::new(
                    "evaluation exceeded the maximum allowed event limit",
                ));
            }
            rolled.extend(cluster_events);
        }

        sort_events(&mut rolled);
        Ok(rolled)
    }

    fn strum_events(_events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: strum only applies to number patterns",
        ))
    }

    fn arp_events(
        _events: Vec<Event<Self>>,
        _steps: u32,
        _direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: arp only applies to number patterns",
        ))
    }

    fn invert_events(
        _events: Vec<Event<Self>>,
        _count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: inversion only applies to number patterns",
        ))
    }

    fn drop_events(_events: Vec<Event<Self>>, _count: u32) -> Result<Vec<Event<Self>>, EvalError> {
        Err(EvalError::new(
            "internal evaluator error: drop voicings only apply to number patterns",
        ))
    }
}

impl PatternRuntimeValue for f64 {
    fn is_finite_numeric(&self) -> bool {
        self.is_finite()
    }
    fn into_runtime_value(pattern: PatternRuntime<Self>) -> Value {
        Value::NumberPattern(NumberPatternValue { pattern })
    }

    fn try_from_runtime_value(value: Value) -> Result<PatternRuntime<Self>, EvalError> {
        match value {
            Value::NumberPattern(pattern) => Ok(pattern.pattern),
            Value::SamplePattern(_)
            | Value::ArpDirection(_)
            | Value::PitchClassSet(_)
            | Value::Function(_)
            | Value::Pedal(_)
            | Value::Tuning(_)
            | Value::String(_) => Err(EvalError::new(
                "transform returned an incompatible value; expected Pattern<Number>",
            )),
        }
    }

    fn try_from_rand(value: f64) -> Result<Self, EvalError> {
        Ok(value)
    }

    /// ⚡ Bolt: Uses slice bounds (`&events[start_index..index]`) instead of allocating a temporary `cluster` Vec for every group of events with the same span, eliminating redundant heap allocations in the hot evaluation loop.
    fn roll_events(
        mut events: Vec<Event<Self>>,
        steps: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut rolled = process_event_clusters(&events, "roll", |cluster| {
            roll_event_cluster(cluster, steps)
        })?;
        sort_events(&mut rolled);
        Ok(rolled)
    }

    /// Applies a strum effect across overlapping events.
    ///
    /// ⚡ Bolt: Mutates overlapping clusters in-place, eliminating the need to allocate and copy into an intermediate `strummed` vector.
    /// ⚡ Bolt: Uses slice bounds (`&events[start_index].part`) instead of cloning `TimeSpan`
    /// for every group of events with the same span, eliminating redundant memory copying
    /// in the hot evaluation loop.
    fn strum_events(mut events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError> {
        mutate_event_clusters(&mut events, "strum", strum_event_cluster)?;
        sort_events(&mut events);
        Ok(events)
    }

    /// ⚡ Bolt: Uses slice bounds (`&events[start_index].part`) instead of cloning `TimeSpan`
    /// for every group of events with the same span, eliminating redundant memory copying
    /// in the hot evaluation loop.
    fn arp_events(
        mut events: Vec<Event<Self>>,
        steps: u32,
        direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        sort_events(&mut events);
        let mut arped = process_event_clusters(&events, "arp", |cluster| {
            let mut cluster_clone = cluster.to_vec();
            arp_event_cluster(&mut cluster_clone, steps, direction)
        })?;
        sort_events(&mut arped);
        Ok(arped)
    }

    /// Applies a chord inversion effect to overlapping events.
    ///
    /// ⚡ Bolt: Modifies clusters in-place and directly returns the original `events` vector, bypassing O(N) allocation overhead for intermediate `inverted` tracking.
    fn invert_events(
        mut events: Vec<Event<Self>>,
        count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        mutate_event_clusters(&mut events, "invert", |cluster| {
            invert_event_cluster(cluster, count)
        })?;
        Ok(events)
    }

    /// Drops the lowest `count` voices from overlapping chords down an octave.
    ///
    /// ⚡ Bolt: Applies the pitch drop in-place over mutable subslices of `events`, completely removing the `dropped` vector allocation step from the hot path.
    fn drop_events(
        mut events: Vec<Event<Self>>,
        count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        mutate_event_clusters(&mut events, "drop", |cluster| {
            drop_event_cluster(cluster, count)
        })?;
        Ok(events)
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

    pub(crate) fn chaos_with_site_salt(self, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Chaos {
                site_salt,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn every(self, period: i64, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::Every {
                period,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn when(self, period: i64, offset: i64, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::When {
                period,
                offset,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn sometimes_with_site_salt(self, transform: FunctionValue, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Sometimes {
                site_salt,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn within(self, start: Rational, end: Rational, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::Within {
                start,
                end,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn mask(self, gate: GatePatternValue) -> Self {
        Self {
            pattern: PatternRuntime::Mask {
                gate: Box::new(gate.into_runtime()),
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

    pub(crate) fn roll(self, steps: u32) -> Self {
        Self {
            pattern: PatternRuntime::Roll {
                steps,
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

    pub(crate) fn delay(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Delay {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DelayPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_time(self, time: f64) -> Self {
        Self {
            pattern: PatternRuntime::DelayTime {
                time,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_time_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DelayTimePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_feedback(self, feedback: f64) -> Self {
        Self {
            pattern: PatternRuntime::DelayFeedback {
                feedback,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn delay_feedback_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DelayFeedbackPattern {
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

    pub(crate) fn reverb(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Reverb {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ReverbPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_room(self, room: f64) -> Self {
        Self {
            pattern: PatternRuntime::ReverbRoom {
                room,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_room_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ReverbRoomPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_damp(self, damp: f64) -> Self {
        Self {
            pattern: PatternRuntime::ReverbDamp {
                damp,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn reverb_damp_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ReverbDampPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn cutoff(self, cutoff_hz: f64) -> Self {
        self.lpf(cutoff_hz)
    }

    pub(crate) fn cutoff_pattern(self, control: NumberPatternValue) -> Self {
        self.lpf_pattern(control)
    }

    pub(crate) fn res(self, resonance: f64) -> Self {
        Self {
            pattern: PatternRuntime::Res {
                resonance,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn res_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ResPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn drive(self, drive: f64) -> Self {
        Self {
            pattern: PatternRuntime::Drive {
                drive,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn drive_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::DrivePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Chorus {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ChorusPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_depth(self, depth: f64) -> Self {
        Self {
            pattern: PatternRuntime::ChorusDepth {
                depth,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_depth_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ChorusDepthPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_rate(self, rate: f64) -> Self {
        Self {
            pattern: PatternRuntime::ChorusRate {
                rate,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chorus_rate_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::ChorusRatePattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pulse_width(self, pulse_width: f64) -> Self {
        Self {
            pattern: PatternRuntime::PulseWidth {
                pulse_width,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn pulse_width_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::PulseWidthPattern {
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

    pub(crate) fn compressor(self, mix: f64) -> Self {
        Self {
            pattern: PatternRuntime::Compressor {
                mix,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::CompressorPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_threshold(self, threshold: f64) -> Self {
        Self {
            pattern: PatternRuntime::CompressorThreshold {
                threshold,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_threshold_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::CompressorThresholdPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_ratio(self, ratio: f64) -> Self {
        Self {
            pattern: PatternRuntime::CompressorRatio {
                ratio,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn compressor_ratio_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::CompressorRatioPattern {
                control: Box::new(control.pattern),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn tune(self, tuning: &TuningValue) -> Self {
        Self {
            pattern: rewrite_pitch_with_tuning(self.pattern, &tuning.as_table()),
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

    pub(crate) fn onset(self, onset_index: u32) -> Self {
        Self {
            pattern: PatternRuntime::Onset {
                onset_index,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn onset_pattern(self, control: NumberPatternValue) -> Self {
        Self {
            pattern: PatternRuntime::OnsetPattern {
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

    pub(crate) fn through(self, pedal_program: Arc<orpheus_dsp::PedalProgram>) -> Self {
        Self {
            pattern: PatternRuntime::Pedal {
                pedal_program,
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
    /// This is the primary way the runtime extracts discrete audio events from a delayed
    /// pattern computation for a single cycle. It evaluates all transformations and returns
    /// the materialized `SampleEvent`s.
    ///
    /// # Errors
    ///
    /// Returns an error if an internal runtime transform produces an invalid
    /// span or overflows the evaluator's bounded rational arithmetic.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::{eval_module, ReplMode};
    ///
    /// // Evaluate a simple sequence
    /// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
    /// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
    ///
    /// // Querying the unit cycle yields 2 events
    /// let events = pattern.query_unit().unwrap();
    /// assert_eq!(events.len(), 2);
    /// ```
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

    pub(crate) fn every(self, period: i64, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::Every {
                period,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn when(self, period: i64, offset: i64, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::When {
                period,
                offset,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn sometimes_with_site_salt(self, transform: FunctionValue, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Sometimes {
                site_salt,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn within(self, start: Rational, end: Rational, transform: FunctionValue) -> Self {
        Self {
            pattern: PatternRuntime::Within {
                start,
                end,
                transform,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn mask(self, gate: GatePatternValue) -> Self {
        Self {
            pattern: PatternRuntime::Mask {
                gate: Box::new(gate.into_runtime()),
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn degrees(self, collection: PitchClassSetValue) -> Self {
        Self {
            pattern: PatternRuntime::Degrees {
                collection,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn chord(self, intervals: &[f64]) -> Self {
        let mut layers = Vec::with_capacity(intervals.len());
        if let Some((last, rest)) = intervals.split_last() {
            for interval in rest {
                layers.push(self.clone().transpose(*interval));
            }
            layers.push(self.transpose(*last));
        }
        Self::stack(layers)
    }

    pub(crate) fn invert(self, count: u32) -> Self {
        Self {
            pattern: PatternRuntime::Invert {
                count,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn strum(self) -> Self {
        Self {
            pattern: PatternRuntime::Strum {
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn roll(self, steps: u32) -> Self {
        Self {
            pattern: PatternRuntime::Roll {
                steps,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn arp(self, steps: u32, direction: ArpDirectionValue) -> Self {
        Self {
            pattern: PatternRuntime::Arp {
                steps,
                direction,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn drop_voice(self, count: u32) -> Self {
        Self {
            pattern: PatternRuntime::Drop {
                count,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn transpose(self, semitones: f64) -> Self {
        Self {
            pattern: PatternRuntime::Transpose {
                semitones,
                inner: Box::new(self.pattern),
            },
        }
    }

    pub(crate) fn transpose_pattern(self, control: Self) -> Self {
        Self {
            pattern: PatternRuntime::TransposePattern {
                control: Box::new(control.pattern),
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

    pub(crate) fn chaos_with_site_salt(self, site_salt: u64) -> Self {
        Self {
            pattern: PatternRuntime::Chaos {
                site_salt,
                inner: Box::new(self.pattern),
            },
        }
    }

    /// Queries the pattern over the default unit cycle `[0, 1)`.
    ///
    /// This method performs the delayed evaluation of the number sequence for
    /// a single cycle, returning materialized values. If evaluation fails due to
    /// an invalid transform or overflow, it gracefully degrades by returning an
    /// empty sequence instead of crashing.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::{eval_module, ReplMode};
    ///
    /// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
    /// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
    ///
    /// let events = pattern.query_unit();
    /// assert_eq!(events.len(), 3);
    /// assert_eq!(events[0].value, 1.0);
    /// ```
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
    /// Use this when you need to explicitly handle evaluation failures
    /// rather than silently ignoring them.
    ///
    /// # Errors
    ///
    /// Returns an error if an internal runtime transform produces an invalid
    /// span or overflows the evaluator's bounded rational arithmetic.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::{eval_module, ReplMode};
    ///
    /// let env = eval_module("x = 1 2", ReplMode::Loose).unwrap();
    /// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
    ///
    /// let events = pattern.try_query_unit().unwrap();
    /// assert_eq!(events.len(), 2);
    /// ```
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

    /// Evaluates the pattern over an arbitrary rational time span.
    ///
    /// This allows querying sequences across multiple cycles or fraction of a cycle.
    ///
    /// # Errors
    /// Returns `EvalError` if querying fails due to invalid arithmetic limits.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::{eval_module, ReplMode, render_span};
    ///
    /// let env = eval_module("x = 1 2", ReplMode::Loose).unwrap();
    /// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
    ///
    /// // Query the first 4 cycles
    /// let span = render_span(4).unwrap();
    /// let events = pattern.try_query(&span).unwrap();
    ///
    /// // 2 events per cycle * 4 cycles = 8 events
    /// assert_eq!(events.len(), 8);
    /// ```
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
        transform: FunctionValue,
        inner: Box<Self>,
    },
    When {
        period: i64,
        offset: i64,
        transform: FunctionValue,
        inner: Box<Self>,
    },
    Sometimes {
        site_salt: u64,
        transform: FunctionValue,
        inner: Box<Self>,
    },
    Within {
        start: Rational,
        end: Rational,
        transform: FunctionValue,
        inner: Box<Self>,
    },
    Mask {
        gate: Box<GatePatternRuntime>,
        inner: Box<Self>,
    },
    Strum {
        inner: Box<Self>,
    },
    Roll {
        steps: u32,
        inner: Box<Self>,
    },
    Arp {
        steps: u32,
        direction: ArpDirectionValue,
        inner: Box<Self>,
    },
    Invert {
        count: u32,
        inner: Box<Self>,
    },
    Drop {
        count: u32,
        inner: Box<Self>,
    },
    Degrees {
        collection: PitchClassSetValue,
        inner: Box<Self>,
    },
    Transpose {
        semitones: f64,
        inner: Box<Self>,
    },
    TransposePattern {
        control: Box<PatternRuntime<f64>>,
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
    Chaos {
        site_salt: u64,
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
    Delay {
        mix: f64,
        inner: Box<Self>,
    },
    DelayPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    DelayTime {
        time: f64,
        inner: Box<Self>,
    },
    DelayTimePattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    DelayFeedback {
        feedback: f64,
        inner: Box<Self>,
    },
    DelayFeedbackPattern {
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
    Reverb {
        mix: f64,
        inner: Box<Self>,
    },
    ReverbPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    ReverbRoom {
        room: f64,
        inner: Box<Self>,
    },
    ReverbRoomPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    ReverbDamp {
        damp: f64,
        inner: Box<Self>,
    },
    ReverbDampPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Res {
        resonance: f64,
        inner: Box<Self>,
    },
    ResPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Drive {
        drive: f64,
        inner: Box<Self>,
    },
    DrivePattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    Chorus {
        mix: f64,
        inner: Box<Self>,
    },
    ChorusPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    ChorusDepth {
        depth: f64,
        inner: Box<Self>,
    },
    ChorusDepthPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    ChorusRate {
        rate: f64,
        inner: Box<Self>,
    },
    ChorusRatePattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    PulseWidth {
        pulse_width: f64,
        inner: Box<Self>,
    },
    PulseWidthPattern {
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
    Compressor {
        mix: f64,
        inner: Box<Self>,
    },
    CompressorPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    CompressorThreshold {
        threshold: f64,
        inner: Box<Self>,
    },
    CompressorThresholdPattern {
        control: Box<PatternRuntime<f64>>,
        inner: Box<Self>,
    },
    CompressorRatio {
        ratio: f64,
        inner: Box<Self>,
    },
    CompressorRatioPattern {
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
    /// Same semantics as `Pitch` but consults a microtonal tuning table.
    TunedPitch {
        semitones: f64,
        tuning: TuningTable,
        inner: Box<Self>,
    },
    /// Same semantics as `PitchPattern` but consults a microtonal tuning table.
    TunedPitchPattern {
        control: Box<PatternRuntime<f64>>,
        tuning: TuningTable,
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
    Onset {
        onset_index: u32,
        inner: Box<Self>,
    },
    OnsetPattern {
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
    Pedal {
        pedal_program: Arc<orpheus_dsp::PedalProgram>,
        inner: Box<Self>,
    },
    Rand {
        site_salt: u64,
    },
}

impl<T> PatternRuntime<T> {

    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    pub fn map_inner<F>(self, mut f: F) -> Self
    where
        F: FnMut(Box<Self>) -> Box<Self>,
    {
        match self {
            Self::Pitch { semitones, inner } => Self::Pitch { semitones, inner: f(inner) },
            Self::PitchPattern { control, inner } => Self::PitchPattern { control, inner: f(inner) },
            Self::TunedPitch { semitones, tuning, inner } => Self::TunedPitch { semitones, tuning, inner: f(inner) },
            Self::TunedPitchPattern { control, tuning, inner } => Self::TunedPitchPattern { control, tuning, inner: f(inner) },
            Self::Every { period, transform, inner } => Self::Every { period, transform, inner: f(inner) },
            Self::When { period, offset, transform, inner } => Self::When { period, offset, transform, inner: f(inner) },
            Self::Sometimes { site_salt, transform, inner } => Self::Sometimes { site_salt, transform, inner: f(inner) },
            Self::Within { start, end, transform, inner } => Self::Within { start, end, transform, inner: f(inner) },
            Self::Mask { gate, inner } => Self::Mask { gate, inner: f(inner) },
            Self::Strum { inner } => Self::Strum { inner: f(inner) },
            Self::Roll { steps, inner } => Self::Roll { steps, inner: f(inner) },
            Self::Arp { steps, direction, inner } => Self::Arp { steps, direction, inner: f(inner) },
            Self::Invert { count, inner } => Self::Invert { count, inner: f(inner) },
            Self::Drop { count, inner } => Self::Drop { count, inner: f(inner) },
            Self::Degrees { collection, inner } => Self::Degrees { collection, inner: f(inner) },
            Self::Transpose { semitones, inner } => Self::Transpose { semitones, inner: f(inner) },
            Self::TransposePattern { control, inner } => Self::TransposePattern { control, inner: f(inner) },
            Self::Fast { factor, inner } => Self::Fast { factor, inner: f(inner) },
            Self::Slow { factor, inner } => Self::Slow { factor, inner: f(inner) },
            Self::Shift { offset, inner } => Self::Shift { offset, inner: f(inner) },
            Self::Rev { inner } => Self::Rev { inner: f(inner) },
            Self::Chaos { site_salt, inner } => Self::Chaos { site_salt, inner: f(inner) },
            Self::Gain { factor, inner } => Self::Gain { factor, inner: f(inner) },
            Self::GainPattern { control, inner } => Self::GainPattern { control, inner: f(inner) },
            Self::Delay { mix, inner } => Self::Delay { mix, inner: f(inner) },
            Self::DelayPattern { control, inner } => Self::DelayPattern { control, inner: f(inner) },
            Self::DelayTime { time, inner } => Self::DelayTime { time, inner: f(inner) },
            Self::DelayTimePattern { control, inner } => Self::DelayTimePattern { control, inner: f(inner) },
            Self::DelayFeedback { feedback, inner } => Self::DelayFeedback { feedback, inner: f(inner) },
            Self::DelayFeedbackPattern { control, inner } => Self::DelayFeedbackPattern { control, inner: f(inner) },
            Self::Hpf { cutoff_hz, inner } => Self::Hpf { cutoff_hz, inner: f(inner) },
            Self::HpfPattern { control, inner } => Self::HpfPattern { control, inner: f(inner) },
            Self::Lpf { cutoff_hz, inner } => Self::Lpf { cutoff_hz, inner: f(inner) },
            Self::LpfPattern { control, inner } => Self::LpfPattern { control, inner: f(inner) },
            Self::Reverb { mix, inner } => Self::Reverb { mix, inner: f(inner) },
            Self::ReverbPattern { control, inner } => Self::ReverbPattern { control, inner: f(inner) },
            Self::ReverbRoom { room, inner } => Self::ReverbRoom { room, inner: f(inner) },
            Self::ReverbRoomPattern { control, inner } => Self::ReverbRoomPattern { control, inner: f(inner) },
            Self::ReverbDamp { damp, inner } => Self::ReverbDamp { damp, inner: f(inner) },
            Self::ReverbDampPattern { control, inner } => Self::ReverbDampPattern { control, inner: f(inner) },
            Self::Res { resonance, inner } => Self::Res { resonance, inner: f(inner) },
            Self::ResPattern { control, inner } => Self::ResPattern { control, inner: f(inner) },
            Self::Drive { drive, inner } => Self::Drive { drive, inner: f(inner) },
            Self::DrivePattern { control, inner } => Self::DrivePattern { control, inner: f(inner) },
            Self::Chorus { mix, inner } => Self::Chorus { mix, inner: f(inner) },
            Self::ChorusPattern { control, inner } => Self::ChorusPattern { control, inner: f(inner) },
            Self::ChorusDepth { depth, inner } => Self::ChorusDepth { depth, inner: f(inner) },
            Self::ChorusDepthPattern { control, inner } => Self::ChorusDepthPattern { control, inner: f(inner) },
            Self::ChorusRate { rate, inner } => Self::ChorusRate { rate, inner: f(inner) },
            Self::ChorusRatePattern { control, inner } => Self::ChorusRatePattern { control, inner: f(inner) },
            Self::PulseWidth { pulse_width, inner } => Self::PulseWidth { pulse_width, inner: f(inner) },
            Self::PulseWidthPattern { control, inner } => Self::PulseWidthPattern { control, inner: f(inner) },
            Self::Pan { amount, inner } => Self::Pan { amount, inner: f(inner) },
            Self::PanPattern { control, inner } => Self::PanPattern { control, inner: f(inner) },
            Self::Compressor { mix, inner } => Self::Compressor { mix, inner: f(inner) },
            Self::CompressorPattern { control, inner } => Self::CompressorPattern { control, inner: f(inner) },
            Self::CompressorThreshold { threshold, inner } => Self::CompressorThreshold { threshold, inner: f(inner) },
            Self::CompressorThresholdPattern { control, inner } => Self::CompressorThresholdPattern { control, inner: f(inner) },
            Self::CompressorRatio { ratio, inner } => Self::CompressorRatio { ratio, inner: f(inner) },
            Self::CompressorRatioPattern { control, inner } => Self::CompressorRatioPattern { control, inner: f(inner) },
            Self::Rate { factor, inner } => Self::Rate { factor, inner: f(inner) },
            Self::RatePattern { control, inner } => Self::RatePattern { control, inner: f(inner) },
            Self::Onset { onset_index, inner } => Self::Onset { onset_index, inner: f(inner) },
            Self::OnsetPattern { control, inner } => Self::OnsetPattern { control, inner: f(inner) },
            Self::Slice { start, end, inner } => Self::Slice { start, end, inner: f(inner) },
            Self::SlicePattern { start_control, end_control, inner } => Self::SlicePattern { start_control, end_control, inner: f(inner) },
            Self::SliceIdxPattern { control, segments, inner } => Self::SliceIdxPattern { control, segments, inner: f(inner) },
            Self::Pedal { pedal_program, inner } => Self::Pedal { pedal_program, inner: f(inner) },
            other => other,
        }
    }

    fn absolute_cycle(&self, cycle: i128) -> Result<i128, EvalError> {
        match self {
            Self::ExplicitCycle { origin_cycle, .. } => origin_cycle
                .checked_add(cycle)
                .ok_or_else(|| EvalError::new("cycle index overflowed while localizing a pattern")),
            Self::Stack(layers) => layers
                .first()
                .map_or(Ok(cycle), |layer| layer.absolute_cycle(cycle)),
            Self::Cycle(_) | Self::Stream(_) | Self::Rand { .. } => Ok(cycle),
            Self::Every { inner, .. }
            | Self::When { inner, .. }
            | Self::Sometimes { inner, .. }
            | Self::Within { inner, .. }
            | Self::Mask { inner, .. }
            | Self::Chaos { inner, .. }
            | Self::Roll { inner, .. }
            | Self::Strum { inner }
            | Self::Arp { inner, .. }
            | Self::Invert { inner, .. }
            | Self::Drop { inner, .. }
            | Self::Degrees { inner, .. }
            | Self::Transpose { inner, .. }
            | Self::TransposePattern { inner, .. }
            | Self::Fast { inner, .. }
            | Self::Slow { inner, .. }
            | Self::Shift { inner, .. }
            | Self::Rev { inner }
            | Self::Gain { inner, .. }
            | Self::GainPattern { inner, .. }
            | Self::Delay { inner, .. }
            | Self::DelayPattern { inner, .. }
            | Self::DelayTime { inner, .. }
            | Self::DelayTimePattern { inner, .. }
            | Self::DelayFeedback { inner, .. }
            | Self::DelayFeedbackPattern { inner, .. }
            | Self::Hpf { inner, .. }
            | Self::HpfPattern { inner, .. }
            | Self::Lpf { inner, .. }
            | Self::LpfPattern { inner, .. }
            | Self::Reverb { inner, .. }
            | Self::ReverbPattern { inner, .. }
            | Self::ReverbRoom { inner, .. }
            | Self::ReverbRoomPattern { inner, .. }
            | Self::ReverbDamp { inner, .. }
            | Self::ReverbDampPattern { inner, .. }
            | Self::Res { inner, .. }
            | Self::ResPattern { inner, .. }
            | Self::Drive { inner, .. }
            | Self::DrivePattern { inner, .. }
            | Self::Chorus { inner, .. }
            | Self::ChorusPattern { inner, .. }
            | Self::ChorusDepth { inner, .. }
            | Self::ChorusDepthPattern { inner, .. }
            | Self::ChorusRate { inner, .. }
            | Self::ChorusRatePattern { inner, .. }
            | Self::PulseWidth { inner, .. }
            | Self::PulseWidthPattern { inner, .. }
            | Self::Pan { inner, .. }
            | Self::PanPattern { inner, .. }
            | Self::Compressor { inner, .. }
            | Self::CompressorPattern { inner, .. }
            | Self::CompressorThreshold { inner, .. }
            | Self::CompressorThresholdPattern { inner, .. }
            | Self::CompressorRatio { inner, .. }
            | Self::CompressorRatioPattern { inner, .. }
            | Self::Pitch { inner, .. }
            | Self::PitchPattern { inner, .. }
            | Self::TunedPitch { inner, .. }
            | Self::TunedPitchPattern { inner, .. }
            | Self::Rate { inner, .. }
            | Self::RatePattern { inner, .. }
            | Self::Onset { inner, .. }
            | Self::OnsetPattern { inner, .. }
            | Self::Slice { inner, .. }
            | Self::SlicePattern { inner, .. }
            | Self::SliceIdxPattern { inner, .. }
            | Self::Pedal { inner, .. } => inner.absolute_cycle(cycle),
        }
    }
}

#[derive(Clone, Debug)]
enum GatePatternRuntime {
    Sample(Box<PatternRuntime<SampleEvent>>),
    Number(Box<PatternRuntime<f64>>),
}

impl GatePatternValue {
    fn into_runtime(self) -> GatePatternRuntime {
        match self {
            Self::Sample(pattern) => GatePatternRuntime::Sample(Box::new(pattern.pattern)),
            Self::Number(pattern) => GatePatternRuntime::Number(Box::new(pattern.pattern)),
        }
    }
}

impl GatePatternRuntime {
    fn query_open_spans(&self, span: &TimeSpan) -> Result<Vec<TimeSpan>, EvalError> {
        match self {
            Self::Sample(pattern) => {
                merge_open_spans(pattern.try_query(span)?.into_iter().map(|event| event.part))
            }
            Self::Number(pattern) => {
                merge_open_spans(pattern.try_query(span)?.into_iter().map(|event| event.part))
            }
        }
    }
}

impl<T> PatternRuntime<T>
where
    T: PatternRuntimeValue,
{
    fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Cycle(pattern) => pattern.try_query(span).map_err(Into::into),
            Self::Stream(stream) => stream.try_query(span).map_err(Into::into),
            Self::ExplicitCycle { stream, .. } => query_explicit_cycle(stream, span),
            Self::Stack(layers) => query_stack(layers, span),
            Self::Every {
                period,
                transform,
                inner,
            } => query_every(inner, transform, *period, span),
            Self::When {
                period,
                offset,
                transform,
                inner,
            } => query_when(inner, transform, *period, *offset, span),
            Self::Sometimes {
                site_salt,
                transform,
                inner,
            } => query_sometimes(inner, transform, *site_salt, span),
            Self::Within {
                start,
                end,
                transform,
                inner,
            } => query_within(inner, start, end, transform, span),
            Self::Mask { gate, inner } => query_mask(inner, gate, span),
            Self::Chaos { site_salt, inner } => query_chaos(inner, *site_salt, span),
            _ => self.try_query_transform(span),
        }
    }

    fn try_query_transform(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Roll { steps, inner } => T::roll_events(inner.try_query(span)?, *steps),
            Self::Strum { inner } => T::strum_events(inner.try_query(span)?),
            Self::Arp {
                steps,
                direction,
                inner,
            } => T::arp_events(inner.try_query(span)?, *steps, *direction),
            Self::Invert { count, inner } => T::invert_events(inner.try_query(span)?, *count),
            Self::Drop { count, inner } => T::drop_events(inner.try_query(span)?, *count),
            Self::Degrees { collection, inner } => {
                apply_value_transform(inner, span, |value| value.map_degrees(collection))
            }
            Self::Transpose { semitones, inner } => {
                apply_value_transform(inner, span, |value| value.transpose_semitones(*semitones))
            }
            Self::TransposePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Transpose)
            }
            Self::Fast { factor, inner } => query_fast(inner, *factor, span),
            Self::Slow { factor, inner } => query_slow(inner, *factor, span),
            Self::Shift { offset, inner } => query_shift(inner, offset, span),
            Self::Rev { inner } => query_rev(inner, span),
            Self::Gain { factor, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_gain(*factor))
            }
            Self::GainPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Gain)
            }
            Self::Pitch { semitones, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_rate(semitones_to_rate_multiplier(*semitones));
            }),
            Self::PitchPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pitch)
            }
            Self::TunedPitch {
                semitones,
                tuning,
                inner,
            } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_rate(semitones_to_tuned_rate(*semitones, tuning));
            }),
            Self::TunedPitchPattern {
                control,
                tuning,
                inner,
            } => apply_tuned_pitch_pattern(inner, control, span, tuning),
            Self::Rate { factor, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_rate(*factor))
            }
            Self::RatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Rate)
            }
            Self::Onset { onset_index, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_onset(*onset_index);
            }),
            Self::OnsetPattern { control, inner } => apply_onset_pattern(inner, control, span),
            Self::Slice { start, end, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_slice(*start, *end);
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
            Self::Pedal {
                pedal_program,
                inner,
            } => apply_value_mutation(inner, span, |value| {
                *value = value.attach_pedal_program(pedal_program);
            }),
            Self::Rand { site_salt } => query_rand(*site_salt, span),
            _ => self.try_query_audio_effect(span),
        }
    }

    fn try_query_audio_effect(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Delay { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_delay_mix(*mix))
            }
            Self::DelayPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayMix)
            }
            Self::DelayTime { time, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_delay_time(*time))
            }
            Self::DelayTimePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayTime)
            }
            Self::DelayFeedback { feedback, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_delay_feedback(*feedback);
            }),
            Self::DelayFeedbackPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayFeedback)
            }
            Self::Hpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_hpf(*cutoff_hz);
            }),
            Self::HpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Hpf)
            }
            Self::Lpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_lpf(*cutoff_hz);
            }),
            Self::LpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Lpf)
            }
            Self::Reverb { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_reverb_mix(*mix))
            }
            Self::ReverbPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbMix)
            }
            Self::ReverbRoom { room, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_reverb_room(*room);
            }),
            Self::ReverbRoomPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbRoom)
            }
            Self::ReverbDamp { damp, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_reverb_damp(*damp);
            }),
            Self::ReverbDampPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbDamp)
            }
            Self::Res { resonance, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_resonance(*resonance);
            }),
            Self::ResPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Res)
            }
            Self::Drive { drive, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_drive(*drive);
            }),
            Self::DrivePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Drive)
            }
            _ => self.try_query_modulation_effect(span),
        }
    }

    fn try_query_modulation_effect(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Chorus { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_chorus_mix(*mix))
            }
            Self::ChorusPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusMix)
            }
            Self::ChorusDepth { depth, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_chorus_depth(*depth);
            }),
            Self::ChorusDepthPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusDepth)
            }
            Self::ChorusRate { rate, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_chorus_rate(*rate);
            }),
            Self::ChorusRatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusRate)
            }
            Self::PulseWidth { pulse_width, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_pulse_width(*pulse_width);
            }),
            Self::PulseWidthPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::PulseWidth)
            }
            Self::Pan { amount, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_pan(*amount))
            }
            Self::PanPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pan)
            }
            Self::Compressor { mix, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_compressor_mix(*mix);
            }),
            Self::CompressorPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::CompressorMix)
            }
            Self::CompressorThreshold { threshold, inner } => {
                apply_value_mutation(inner, span, |value| {
                    *value = value.adjust_compressor_threshold(*threshold);
                })
            }
            Self::CompressorThresholdPattern { control, inner } => apply_control_pattern(
                inner,
                control,
                span,
                ControlPatternKind::CompressorThreshold,
            ),
            Self::CompressorRatio { ratio, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_compressor_ratio(*ratio);
            }),
            Self::CompressorRatioPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::CompressorRatio)
            }

            _ => unreachable!("handled in previous try_query stages"),
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

fn apply_value_transform<T, F>(
    inner: &PatternRuntime<T>,
    span: &TimeSpan,
    mut transform: F,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&T) -> Result<T, EvalError>,
{
    let mut events = inner.try_query(span)?;
    for event in &mut events {
        event.value = transform(&event.value)?;
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
        let layer_events = layer.try_query(span)?;
        if events.len() + layer_events.len() > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }
        events.extend(layer_events);
    }
    sort_events(&mut events);
    Ok(events)
}

fn query_mask<T>(
    inner: &PatternRuntime<T>,
    gate: &GatePatternRuntime,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_events = inner.try_query(span)?;
    let gate_spans = gate.query_open_spans(span)?;
    if source_events.is_empty() || gate_spans.is_empty() {
        return Ok(Vec::new());
    }

    let mut masked = Vec::with_capacity(source_events.len());

    for event in &source_events {
        let Some(boundaries) = compute_event_fragment_boundaries(&event.part, gate_spans.iter())
        else {
            continue;
        };

        for window in boundaries.windows(2) {
            let [start, end] = window else {
                continue;
            };
            if *start >= *end {
                continue;
            }

            let part = build_span(**start, **end)?;
            if gate_spans
                .iter()
                .any(|gate_span| spans_overlap(gate_span, &part))
            {
                masked.push(Event {
                    whole: None,
                    part,
                    value: event.value.clone(),
                });
            }
        }
    }

    sort_events(&mut masked);
    Ok(masked)
}

fn invert_event_cluster(cluster: &mut [Event<f64>], count: u32) -> Result<(), EvalError> {
    cluster.sort_by(|left, right| left.value.total_cmp(&right.value));
    for _ in 0..count {
        if cluster.len() <= 1 {
            break;
        }

        cluster[0].value += 12.0;
        if !cluster[0].value.is_finite() {
            return Err(EvalError::new(
                "`invert` produced a non-finite numeric value",
            ));
        }
        cluster.sort_by(|left, right| left.value.total_cmp(&right.value));
    }
    Ok(())
}

fn roll_event_cluster<T: Clone>(
    cluster: &[Event<T>],
    steps: u32,
) -> Result<Vec<Event<T>>, EvalError> {
    if cluster.is_empty() || cluster.len() == 1 && steps == 1 {
        return Ok(cluster.to_vec());
    }
    if steps == 0 {
        return Err(EvalError::new(
            "`roll` requires a positive whole number of steps",
        ));
    }

    let span = cluster[0].part;
    let width = window_width(&span)?;
    if width == Rational::zero() || steps == 1 {
        return Ok(cluster.to_vec());
    }

    let step_count = i128::from(steps);
    let step = rational_mul(
        &width,
        &rational_reciprocal(&rational_from_parts(step_count, 1)?)?,
    )?;

    let cluster_len = cluster.len();
    let steps_usize = usize::try_from(steps)
        .map_err(|_| EvalError::new("`roll` exceeded the supported evaluator range"))?;
    let capacity = cluster_len
        .checked_mul(steps_usize)
        .ok_or_else(|| EvalError::new("`roll` exceeded the supported evaluator range"))?;
    let mut rolled = Vec::with_capacity(capacity);

    for index in 0..steps {
        let offset = rational_mul_parts(&step, i64::from(index), 1)?;
        let start = rational_add(span.start(), &offset)?;
        let end = rational_add(&start, &step)?;
        let part = TimeSpan::new(start, end).map_err(EvalError::from)?;
        for event in cluster {
            rolled.push(Event {
                whole: None,
                part,
                value: event.value.clone(),
            });
        }
    }

    Ok(rolled)
}

fn strum_event_cluster(cluster: &mut [Event<f64>]) -> Result<(), EvalError> {
    if cluster.len() <= 1 {
        return Ok(());
    }

    cluster.sort_by(|left, right| left.value.total_cmp(&right.value));
    let span = cluster[0].part;
    let width = window_width(&span)?;
    if width == Rational::zero() {
        return Ok(());
    }
    let count = i128::try_from(cluster.len())
        .map_err(|_| EvalError::new("`strum` exceeded the supported evaluator range"))?;
    let step = rational_mul(
        &width,
        &rational_reciprocal(&rational_from_parts(count, 1)?)?,
    )?;

    for (index, event) in cluster.iter_mut().enumerate() {
        let index = i64::try_from(index)
            .map_err(|_| EvalError::new("`strum` exceeded the supported evaluator range"))?;
        let offset = rational_mul_parts(&step, index, 1)?;
        let start = rational_add(span.start(), &offset)?;
        let end = rational_add(&start, &step)?;
        event.part = TimeSpan::new(start, end).map_err(EvalError::from)?;
        event.whole = None;
    }

    Ok(())
}

fn arp_event_cluster(
    cluster: &mut [Event<f64>],
    steps: u32,
    direction: ArpDirectionValue,
) -> Result<Vec<Event<f64>>, EvalError> {
    if cluster.is_empty() {
        return Ok(cluster.to_vec());
    }

    if steps == 0 {
        return Err(EvalError::new(
            "`arp` requires a positive whole number of steps",
        ));
    }

    cluster.sort_by(|left, right| left.value.total_cmp(&right.value));
    let span = cluster[0].part;
    let width = window_width(&span)?;
    if width == Rational::zero() {
        return Ok(cluster.to_vec());
    }

    let step_count = i128::from(steps);
    let step = rational_mul(
        &width,
        &rational_reciprocal(&rational_from_parts(step_count, 1)?)?,
    )?;
    let len = cluster.len();
    let capacity = usize::try_from(steps)
        .map_err(|_| EvalError::new("`arp` exceeded the supported evaluator range"))?;
    let mut arped = Vec::with_capacity(capacity);

    for index in 0..steps {
        let offset_index = i64::from(index);
        let offset = rational_mul_parts(&step, offset_index, 1)?;
        let start = rational_add(span.start(), &offset)?;
        let end = rational_add(&start, &step)?;
        let slot = usize::try_from(index)
            .map_err(|_| EvalError::new("`arp` exceeded the supported evaluator range"))?
            % len;
        let selected = match direction {
            ArpDirectionValue::Up => slot,
            ArpDirectionValue::Down => len - 1 - slot,
            ArpDirectionValue::PingPong => {
                if len <= 1 {
                    0
                } else {
                    let cycle_len = len * 2 - 2;
                    let cycle_slot = usize::try_from(index).map_err(|_| {
                        EvalError::new("`arp` exceeded the supported evaluator range")
                    })? % cycle_len;
                    if cycle_slot < len {
                        cycle_slot
                    } else {
                        cycle_len - cycle_slot
                    }
                }
            }
        };
        arped.push(Event {
            whole: None,
            part: TimeSpan::new(start, end).map_err(EvalError::from)?,
            value: cluster[selected].value,
        });
    }

    Ok(arped)
}

fn drop_event_cluster(cluster: &mut [Event<f64>], count: u32) -> Result<(), EvalError> {
    cluster.sort_by(|left, right| left.value.total_cmp(&right.value));
    let len = cluster.len();
    let count = usize::try_from(count)
        .map_err(|_| EvalError::new("`drop` exceeded the supported evaluator range"))?;
    if len < count {
        return Ok(());
    }

    let target_index = len - count;
    cluster[target_index].value -= 12.0;
    if !cluster[target_index].value.is_finite() {
        return Err(EvalError::new("`drop` produced a non-finite numeric value"));
    }
    cluster.sort_by(|left, right| left.value.total_cmp(&right.value));
    Ok(())
}

fn process_event_clusters<T, F, R>(
    events: &[Event<T>],
    context: &str,
    mut process_cluster: F,
) -> Result<Vec<R>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&[Event<T>]) -> Result<Vec<R>, EvalError>,
{
    let mut result = Vec::with_capacity(events.len());
    let mut index = 0;

    while index < events.len() {
        let start_index = index;
        let span = &events[start_index].part;
        while index < events.len() && &events[index].part == span {
            if !events[index].value.is_finite_numeric() {
                return Err(EvalError::new(format!(
                    "`{context}` requires finite numeric values"
                )));
            }
            index += 1;
        }

        let cluster_result = process_cluster(&events[start_index..index])?;
        if result.len() + cluster_result.len() > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }
        result.extend(cluster_result);
    }

    Ok(result)
}

fn mutate_event_clusters<T, F>(
    events: &mut [Event<T>],
    context: &str,
    mut mutate_cluster: F,
) -> Result<(), EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&mut [Event<T>]) -> Result<(), EvalError>,
{
    sort_events(events);
    let mut index = 0;

    while index < events.len() {
        let start_index = index;
        let span = &events[start_index].part;
        while index < events.len() && &events[index].part == span {
            if !events[index].value.is_finite_numeric() {
                return Err(EvalError::new(format!(
                    "`{context}` requires finite numeric values"
                )));
            }
            index += 1;
        }

        mutate_cluster(&mut events[start_index..index])?;
    }

    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ControlPatternKind {
    Gain,
    DelayMix,
    DelayTime,
    DelayFeedback,
    Hpf,
    Lpf,
    ReverbMix,
    ReverbRoom,
    ReverbDamp,
    Res,
    Drive,
    ChorusMix,
    ChorusDepth,
    ChorusRate,
    PulseWidth,
    Pan,
    CompressorMix,
    CompressorThreshold,
    CompressorRatio,
    Pitch,
    Rate,
    Transpose,
}

impl ControlPatternKind {
    fn apply<T: PatternRuntimeValue>(self, value: &T, control_val: f64) -> Result<T, EvalError> {
        match self {
            Self::Gain => Ok(value.adjust_gain(control_val)),
            Self::DelayMix => Ok(value.adjust_delay_mix(control_val)),
            Self::DelayTime => Ok(value.adjust_delay_time(control_val)),
            Self::DelayFeedback => Ok(value.adjust_delay_feedback(control_val)),
            Self::Hpf => Ok(value.adjust_hpf(control_val)),
            Self::Lpf => Ok(value.adjust_lpf(control_val)),
            Self::ReverbMix => Ok(value.adjust_reverb_mix(control_val)),
            Self::ReverbRoom => Ok(value.adjust_reverb_room(control_val)),
            Self::ReverbDamp => Ok(value.adjust_reverb_damp(control_val)),
            Self::Res => Ok(value.adjust_resonance(control_val)),
            Self::Drive => Ok(value.adjust_drive(control_val)),
            Self::ChorusMix => Ok(value.adjust_chorus_mix(control_val)),
            Self::ChorusDepth => Ok(value.adjust_chorus_depth(control_val)),
            Self::ChorusRate => Ok(value.adjust_chorus_rate(control_val)),
            Self::PulseWidth => Ok(value.adjust_pulse_width(control_val)),
            Self::Pan => Ok(value.adjust_pan(control_val)),
            Self::CompressorMix => Ok(value.adjust_compressor_mix(control_val)),
            Self::CompressorThreshold => Ok(value.adjust_compressor_threshold(control_val)),
            Self::CompressorRatio => Ok(value.adjust_compressor_ratio(control_val)),
            Self::Pitch => Ok(value.adjust_rate(semitones_to_rate_multiplier(control_val))),
            Self::Rate => Ok(value.adjust_rate(control_val)),
            Self::Transpose => value.transpose_semitones(control_val),
        }
    }

    fn validate(self, value: f64) -> Result<(), EvalError> {
        match self {
            Self::Gain => Self::validate_gain(value),
            Self::DelayMix => Self::validate_delay_mix(value),
            Self::DelayTime => Self::validate_delay_time(value),
            Self::DelayFeedback => Self::validate_delay_feedback(value),
            Self::Hpf => Self::validate_hpf(value),
            Self::Lpf => Self::validate_lpf(value),
            Self::ReverbMix => Self::validate_reverb_mix(value),
            Self::ReverbRoom => Self::validate_reverb_room(value),
            Self::ReverbDamp => Self::validate_reverb_damp(value),
            Self::Res => Self::validate_res(value),
            Self::Drive => Self::validate_drive(value),
            _ => self.validate_fx(value),
        }
    }

    fn validate_fx(self, value: f64) -> Result<(), EvalError> {
        match self {
            Self::ChorusMix => Self::validate_chorus_mix(value),
            Self::ChorusDepth => Self::validate_chorus_depth(value),
            Self::ChorusRate => Self::validate_chorus_rate(value),
            Self::PulseWidth => Self::validate_pulse_width(value),
            Self::Pan => Self::validate_pan(value),
            Self::CompressorMix => Self::validate_compressor_mix(value),
            Self::CompressorThreshold => Self::validate_compressor_threshold(value),
            Self::CompressorRatio => Self::validate_compressor_ratio(value),
            Self::Pitch => Self::validate_pitch(value),
            Self::Rate => Self::validate_rate(value),
            Self::Transpose => Self::validate_transpose(value),
            _ => unreachable!("handled in validate"),
        }
    }

    fn validate_gain(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() {
            return Err(EvalError::new(
                "`gain` requires finite numeric control values",
            ));
        }
        Ok(())
    }

    fn validate_delay_mix(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`delay` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_delay_time(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value <= f64::EPSILON || value > 1.0 {
            return Err(EvalError::new(
                "`delay_time` requires positive finite control values within (0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_delay_feedback(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`delay_feedback` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_hpf(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value <= f64::EPSILON {
            return Err(EvalError::new(
                "`hpf` requires positive finite control values",
            ));
        }
        Ok(())
    }

    fn validate_lpf(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value <= f64::EPSILON {
            return Err(EvalError::new(
                "`lpf` requires positive finite control values",
            ));
        }
        Ok(())
    }

    fn validate_reverb_mix(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`reverb` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_reverb_room(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`reverb_room` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_reverb_damp(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`reverb_damp` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_res(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`res` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_drive(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value < 0.0 {
            return Err(EvalError::new(
                "`drive` requires finite non-negative control values",
            ));
        }
        Ok(())
    }

    fn validate_chorus_mix(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`chorus` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_chorus_depth(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`chorus_depth` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_chorus_rate(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value <= f64::EPSILON {
            return Err(EvalError::new(
                "`chorus_rate` requires positive finite control values",
            ));
        }
        Ok(())
    }

    fn validate_pulse_width(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..1.0).contains(&value) {
            return Err(EvalError::new(
                "`pw` requires finite control values in the open interval (0, 1)",
            ));
        }
        Ok(())
    }

    fn validate_pan(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`pan` requires finite control values within [-1, 1]",
            ));
        }
        Ok(())
    }

    fn validate_compressor_mix(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`compressor` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_compressor_threshold(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvalError::new(
                "`compressor_threshold` requires finite control values within [0, 1]",
            ));
        }
        Ok(())
    }

    fn validate_compressor_ratio(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value < 1.0 {
            return Err(EvalError::new(
                "`compressor_ratio` requires finite control values >= 1",
            ));
        }
        Ok(())
    }

    fn validate_pitch(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() {
            return Err(EvalError::new(
                "`pitch` requires finite numeric control values",
            ));
        }
        Ok(())
    }

    fn validate_rate(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() || value.abs() <= f64::EPSILON {
            return Err(EvalError::new(
                "`rate` requires finite non-zero control values",
            ));
        }
        Ok(())
    }

    fn validate_transpose(value: f64) -> Result<(), EvalError> {
        if !value.is_finite() {
            return Err(EvalError::new(
                "`transpose` requires finite numeric control values",
            ));
        }
        Ok(())
    }
}

#[allow(clippy::needless_pass_by_value)]
fn apply_event_fragments<'a, T, F, I>(
    source_events: &'a [Event<T>],
    control_parts: I,
    mut process_fragment: F,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&TimeSpan, &T) -> Result<Option<T>, EvalError>,
    I: Iterator<Item = &'a TimeSpan> + Clone,
{
    let mut composed = Vec::with_capacity(source_events.len());
    for event in source_events {
        let Some(boundaries) =
            compute_event_fragment_boundaries(&event.part, control_parts.clone())
        else {
            composed.push(event.clone());
            continue;
        };

        for window in boundaries.windows(2) {
            let [start, end] = window else {
                continue;
            };
            if *start >= *end {
                continue;
            }

            let part = build_span(**start, **end)?;
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
    for event in &control_events {
        kind.validate(event.value)?;
    }
    if control_events.is_empty() {
        return Ok(source_events);
    }

    apply_event_fragments(
        &source_events,
        control_events.iter().map(|e| &e.part),
        |part, value| {
            let mut new_value = value.clone();
            for control_event in &control_events {
                if spans_overlap(&control_event.part, part) {
                    new_value = kind.apply(&new_value, control_event.value)?;
                }
            }
            Ok(Some(new_value))
        },
    )
}

fn semitones_to_rate_multiplier(semitones: f64) -> f64 {
    (semitones / 12.0).exp2()
}

#[allow(clippy::match_same_arms)]
fn rewrite_pitch_with_tuning<T: Clone>(
    pattern: PatternRuntime<T>,
    table: &TuningTable,
) -> PatternRuntime<T> {
    match pattern {
        PatternRuntime::Pitch { semitones, inner } => PatternRuntime::TunedPitch {
            semitones,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::PitchPattern { control, inner } => PatternRuntime::TunedPitchPattern {
            control,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::TunedPitch {
            semitones, inner, ..
        } => PatternRuntime::TunedPitch {
            semitones,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::TunedPitchPattern { control, inner, .. } => PatternRuntime::TunedPitchPattern {
            control,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::Stack(layers) => PatternRuntime::Stack(
            layers
                .into_iter()
                .map(|layer| rewrite_pitch_with_tuning(layer, table))
                .collect(),
        ),
        other => other.map_inner(|inner| Box::new(rewrite_pitch_with_tuning(*inner, table))),
    }
}

/// Maps an integer semitone step onto a ratio from `table`, wrapping beyond one
/// octave by multiplying through `period`. Fractional semitones round to the
/// nearest step in Phase 1.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn semitones_to_tuned_rate(semitones: f64, table: &TuningTable) -> f64 {
    let ratios = table.ratios.as_ref();
    debug_assert!(!ratios.is_empty(), "tuning tables are non-empty");
    // Scale sizes are far below i32::MAX in practice (tens or low hundreds);
    // the wrap warning is suppressed to keep the single-multiply fast path.
    let n = ratios.len() as i32;
    let step = (semitones.round() as i32).saturating_sub(table.ref_semitone);
    let idx = step.rem_euclid(n) as usize;
    let octaves = step.div_euclid(n);
    ratios[idx] * table.period.powi(octaves)
}

fn apply_tuned_pitch_pattern<T>(
    inner: &PatternRuntime<T>,
    control: &PatternRuntime<f64>,
    span: &TimeSpan,
    tuning: &TuningTable,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_events = inner.try_query(span)?;
    let control_events = control.try_query(span)?;
    for event in &control_events {
        ControlPatternKind::validate_pitch(event.value)?;
    }
    if control_events.is_empty() {
        return Ok(source_events);
    }

    apply_event_fragments(
        &source_events,
        control_events.iter().map(|e| &e.part),
        |part, value| {
            let mut new_value = value.clone();
            for control_event in &control_events {
                if spans_overlap(&control_event.part, part) {
                    new_value =
                        new_value.adjust_rate(semitones_to_tuned_rate(control_event.value, tuning));
                }
            }
            Ok(Some(new_value))
        },
    )
}

fn whole_number_from_degree_value(value: f64) -> Result<i32, EvalError> {
    if !value.is_finite() || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(
            "`degrees` requires whole-number degree values",
        ));
    }

    #[allow(clippy::cast_possible_truncation)]
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        Err(EvalError::new(
            "`degrees` degree exceeded the supported evaluator range",
        ))
    } else {
        #[allow(clippy::cast_possible_truncation)]
        let degree = rounded as i32;
        Ok(degree)
    }
}

fn map_degree_to_semitones(degree: i32, collection: &PitchClassSetValue) -> Result<f64, EvalError> {
    let intervals = collection.intervals();
    let scale_len = i32::try_from(intervals.len()).map_err(|_| {
        EvalError::new("`degrees` scale length exceeded the supported evaluator range")
    })?;
    if scale_len == 0 {
        return Err(EvalError::new(
            "`degrees` requires a non-empty pitch class set",
        ));
    }
    let octave = degree.div_euclid(scale_len);
    let index = usize::try_from(degree.rem_euclid(scale_len)).map_err(|_| {
        EvalError::new("`degrees` scale index exceeded the supported evaluator range")
    })?;
    let semitones = octave
        .checked_mul(12)
        .and_then(|value| value.checked_add(intervals[index]))
        .ok_or_else(|| EvalError::new("`degrees` exceeded the supported evaluator range"))?;
    Ok(f64::from(semitones))
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
        &source_events,
        start_events
            .iter()
            .map(|e| &e.part)
            .chain(end_events.iter().map(|e| &e.part)),
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

            Ok(Some(
                value.clone().adjust_slice(relative_start, relative_end),
            ))
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

    apply_event_fragments(
        &source_events,
        control_events.iter().map(|e| &e.part),
        |part, value| {
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
                    new_value = new_value.adjust_slice(slice_start, slice_end);
                }
            }
            Ok(Some(new_value))
        },
    )
}

fn apply_onset_pattern<T>(
    inner: &PatternRuntime<T>,
    control: &PatternRuntime<f64>,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    let source_events = inner.try_query(span)?;
    let control_events = control.try_query(span)?;
    validate_onset_control_events(&control_events)?;
    if control_events.is_empty() {
        return Ok(source_events);
    }

    apply_event_fragments(
        &source_events,
        control_events.iter().map(|e| &e.part),
        |part, value| {
            let mut new_value = value.clone();
            for control_event in &control_events {
                if spans_overlap(&control_event.part, part) {
                    new_value =
                        new_value.adjust_onset(whole_number_from_onset_value(control_event.value)?);
                }
            }
            Ok(Some(new_value))
        },
    )
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

fn validate_onset_control_events(control_events: &[Event<f64>]) -> Result<(), EvalError> {
    for event in control_events {
        whole_number_from_onset_value(event.value)?;
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

fn whole_number_from_onset_value(value: f64) -> Result<u32, EvalError> {
    if !value.is_finite() || value < 0.0 || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(
            "`onset` requires whole-number control values",
        ));
    }

    format!("{value:.0}")
        .parse::<u32>()
        .map_err(|_| EvalError::new("`onset` control value exceeded the supported evaluator range"))
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
        part: *span,
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
        let mut cycle_events = stream.try_query(&local_query)?;
        shift_events(&mut cycle_events, &cycle_offset)?;
        if events.len() + cycle_events.len() > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }
        events.extend(cycle_events);
    }

    sort_events(&mut events);
    Ok(events)
}

fn query_every<T>(
    inner: &PatternRuntime<T>,
    transform: &FunctionValue,
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

fn query_when<T>(
    inner: &PatternRuntime<T>,
    transform: &FunctionValue,
    period: i64,
    offset: i64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    let period = i128::from(period);
    let offset = i128::from(offset);
    query_transform_cycles(inner, transform, span, |cycle| {
        cycle.rem_euclid(period) == offset
    })
}

fn query_sometimes<T>(
    inner: &PatternRuntime<T>,
    transform: &FunctionValue,
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

fn query_chaos<T>(
    inner: &PatternRuntime<T>,
    site_salt: u64,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    let mut events = Vec::with_capacity(8);
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    for cycle in start_cycle..end_cycle {
        let cycle_span = cycle_span(cycle)?;
        let Some(query_slice) = clip_span(&cycle_span, span)? else {
            continue;
        };

        // Query the underlying events for this whole cycle to shuffle them accurately
        let mut cycle_events = inner.try_query(&cycle_span)?;
        if cycle_events.is_empty() {
            continue;
        }

        // Shuffle using a deterministic RNG seeded by site_salt and cycle index
        let mut rng_state = deterministic_prng(site_salt, cycle);
        let len = cycle_events.len();
        for i in (1..len).rev() {
            // LCG for next random number
            rng_state = rng_state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = usize::try_from(rng_state).unwrap_or(usize::MAX) % (i + 1);
            if i != j {
                // ⚡ Bolt: Swap values in-place without allocating an intermediate `Vec` or deep cloning strings.
                // We use `split_at_mut` to get two disjoint mutable slices, guaranteeing safety.
                let (left, right) = cycle_events.split_at_mut(i.max(j));
                std::mem::swap(&mut left[i.min(j)].value, &mut right[0].value);
            }
        }

        // Apply shuffled values back to the original timing structure and clip to the query slice

        for event in cycle_events {
            if spans_overlap(&event.part, &query_slice) {
                // If it partially overlaps, we need to clip it
                if let Some(clipped_part) = clip_span(&event.part, &query_slice)? {
                    // Update the `whole` span if it was clipped, or preserve it
                    let whole = if clipped_part == event.part {
                        event.whole
                    } else {
                        Some(event.whole.unwrap_or(event.part))
                    };
                    events.push(Event {
                        whole,
                        part: clipped_part,
                        value: event.value,
                    });
                }
            }
        }
    }

    sort_events(&mut events);
    Ok(events)
}

fn query_transform_cycles<T, F>(
    inner: &PatternRuntime<T>,
    transform: &FunctionValue,
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

        let absolute_cycle = inner.absolute_cycle(cycle)?;
        if should_transform(absolute_cycle) {
            let cycle_offset = rational_from_parts(cycle, 1)?;
            let local_offset = rational_sub(&Rational::zero(), &cycle_offset)?;
            let local_query = translate_span(&query_slice, &local_offset)?;
            let localized = localize_cycle_runtime(inner, cycle, absolute_cycle)?;
            let mut transformed_events =
                apply_unary_transform(transform, localized)?.try_query(&local_query)?;
            shift_events(&mut transformed_events, &cycle_offset)?;
            if events.len() + transformed_events.len() > 100_000 {
                return Err(EvalError::new(
                    "evaluation exceeded the maximum allowed event limit",
                ));
            }
            events.extend(transformed_events);
        } else {
            let slice_events = inner.try_query(&query_slice)?;
            if events.len() + slice_events.len() > 100_000 {
                return Err(EvalError::new(
                    "evaluation exceeded the maximum allowed event limit",
                ));
            }
            events.extend(slice_events);
        }
    }

    sort_events(&mut events);
    Ok(events)
}

fn query_within<T>(
    inner: &PatternRuntime<T>,
    start: &Rational,
    end: &Rational,
    transform: &FunctionValue,
    span: &TimeSpan,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
{
    if span.is_empty() {
        return Ok(Vec::new());
    }

    let mut events = Vec::with_capacity(8);
    let start_cycle = floor_rational(span.start());
    let end_cycle = ceil_rational(span.end());

    for cycle in start_cycle..end_cycle {
        let cycle_span = cycle_span(cycle)?;
        let Some(query_slice) = clip_span(&cycle_span, span)? else {
            continue;
        };
        let window_span = within_window_span(cycle, start, end)?;
        let Some(window_query) = clip_span(&window_span, &query_slice)? else {
            let slice_events = inner.try_query(&query_slice)?;
            if events.len() + slice_events.len() > 100_000 {
                return Err(EvalError::new(
                    "evaluation exceeded the maximum allowed event limit",
                ));
            }
            events.extend(slice_events);
            continue;
        };

        if let Some(before_window) =
            clip_between(&query_slice, cycle_span.start(), window_span.start())?
        {
            let slice_events = inner.try_query(&before_window)?;
            if events.len() + slice_events.len() > 100_000 {
                return Err(EvalError::new(
                    "evaluation exceeded the maximum allowed event limit",
                ));
            }
            events.extend(slice_events);
        }

        let absolute_cycle = inner.absolute_cycle(cycle)?;
        let localized = localize_window_runtime(inner, &window_span, absolute_cycle)?;
        let local_query = localize_span_to_window(&window_query, &window_span)?;
        let mut transformed_events =
            apply_unary_transform(transform, localized)?.try_query(&local_query)?;
        restore_window_localized_events(&mut transformed_events, &window_span)?;
        if events.len() + transformed_events.len() > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }
        events.extend(transformed_events);

        if let Some(after_window) = clip_between(&query_slice, window_span.end(), cycle_span.end())?
        {
            let slice_events = inner.try_query(&after_window)?;
            if events.len() + slice_events.len() > 100_000 {
                return Err(EvalError::new(
                    "evaluation exceeded the maximum allowed event limit",
                ));
            }
            events.extend(slice_events);
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
        if events.len() + mirrored_events.len() > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
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

fn localize_window_runtime<T>(
    inner: &PatternRuntime<T>,
    window_span: &TimeSpan,
    origin_cycle: i128,
) -> Result<PatternRuntime<T>, EvalError>
where
    T: PatternRuntimeValue,
{
    let window_start = *window_span.start();
    let width = window_width(window_span)?;
    let normalize_factor = rational_reciprocal(&width)?;
    let local_offset = rational_sub(&Rational::zero(), &window_start)?;
    let mut localized_events = inner.try_query(window_span)?;

    for event in &mut localized_events {
        event.part = translate_span(&event.part, &local_offset)?;
        event.part = scale_span_by_rational(&event.part, &normalize_factor)?;
        event.whole = None;
    }

    Ok(PatternRuntime::ExplicitCycle {
        origin_cycle,
        stream: EventStream::new(localized_events),
    })
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

    build_span(*start, *end).map(Some)
}

fn merge_open_spans<I: Iterator<Item = TimeSpan>>(mut iter: I) -> Result<Vec<TimeSpan>, EvalError> {
    let Some(mut current) = iter.next() else {
        return Ok(Vec::new());
    };

    // ⚡ Bolt: Pre-allocate vector using the iterator's size hint as the maximum bound
    // to reduce heap reallocations during merge operations.
    let mut merged = Vec::with_capacity(iter.size_hint().0.saturating_add(1));

    for span in iter {
        if span.start() <= current.end() {
            let merged_end = if span.end() > current.end() {
                *span.end()
            } else {
                *current.end()
            };
            current = build_span(*current.start(), merged_end)?;
        } else {
            merged.push(current);
            current = span;
        }
    }

    merged.push(current);
    Ok(merged)
}

fn spans_overlap(a: &TimeSpan, b: &TimeSpan) -> bool {
    max(a.start(), b.start()) < min(a.end(), b.end())
}

fn compute_event_fragment_boundaries<'a, 'b, I>(
    source_span: &'a TimeSpan,
    control_parts: I,
) -> Option<Vec<&'a Rational>>
where
    'b: 'a,
    I: Iterator<Item = &'b TimeSpan>,
{
    let (lower, upper) = control_parts.size_hint();
    let capacity_estimate = 2_usize
        .saturating_add(upper.unwrap_or(lower).saturating_mul(2))
        .min(1024);
    // PRE-ALLOCATE: prevents heap reallocations when collecting span boundaries.
    let mut boundaries = Vec::with_capacity(capacity_estimate);
    boundaries.push(source_span.start());
    boundaries.push(source_span.end());
    let mut has_overlap = false;

    for control_part in control_parts {
        let start = max(control_part.start(), source_span.start());
        let end = min(control_part.end(), source_span.end());
        if start < end {
            has_overlap = true;
            boundaries.push(start);
            boundaries.push(end);
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

fn scale_span_by_rational(span: &TimeSpan, factor: &Rational) -> Result<TimeSpan, EvalError> {
    build_span(
        rational_mul(span.start(), factor)?,
        rational_mul(span.end(), factor)?,
    )
}

fn translate_span(span: &TimeSpan, offset: &Rational) -> Result<TimeSpan, EvalError> {
    build_span(
        rational_add(span.start(), offset)?,
        rational_add(span.end(), offset)?,
    )
}

fn clip_between(
    query: &TimeSpan,
    start: &Rational,
    end: &Rational,
) -> Result<Option<TimeSpan>, EvalError> {
    if start >= end {
        return Ok(None);
    }
    clip_span(&build_span(*start, *end)?, query)
}

fn within_window_span(
    cycle: i128,
    start: &Rational,
    end: &Rational,
) -> Result<TimeSpan, EvalError> {
    let cycle_start = rational_from_parts(cycle, 1)?;
    build_span(
        rational_add(&cycle_start, start)?,
        rational_add(&cycle_start, end)?,
    )
}

fn window_width(window_span: &TimeSpan) -> Result<Rational, EvalError> {
    rational_sub(window_span.end(), window_span.start())
}

fn localize_span_to_window(span: &TimeSpan, window_span: &TimeSpan) -> Result<TimeSpan, EvalError> {
    let width = window_width(window_span)?;
    let normalize_factor = rational_reciprocal(&width)?;
    let local_offset = rational_sub(&Rational::zero(), window_span.start())?;
    let translated = translate_span(span, &local_offset)?;
    scale_span_by_rational(&translated, &normalize_factor)
}

fn restore_window_localized_events<T>(
    events: &mut [Event<T>],
    window_span: &TimeSpan,
) -> Result<(), EvalError> {
    let width = window_width(window_span)?;
    for event in &mut *events {
        event.part = scale_span_by_rational(&event.part, &width)?;
        event.part = translate_span(&event.part, window_span.start())?;
        if let Some(whole) = event.whole.take() {
            let scaled = scale_span_by_rational(&whole, &width)?;
            event.whole = Some(translate_span(&scaled, window_span.start())?);
        }
    }

    sort_events(events);
    Ok(())
}

fn build_span(start: Rational, end: Rational) -> Result<TimeSpan, EvalError> {
    Ok(TimeSpan::new(start, end)?)
}

fn rational_add(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    Ok(left.checked_add(right)?)
}

fn rational_sub(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    Ok(left.checked_sub(right)?)
}

fn rational_mul(left: &Rational, right: &Rational) -> Result<Rational, EvalError> {
    left.checked_mul(right).map_err(Into::into)
}

fn rational_reciprocal(value: &Rational) -> Result<Rational, EvalError> {
    Rational::checked_from_parts(value.denominator(), value.numerator()).map_err(Into::into)
}

fn rational_mul_parts(
    value: &Rational,
    numerator: i64,
    denominator: i64,
) -> Result<Rational, EvalError> {
    let factor = Rational::checked_from_parts(i128::from(numerator), i128::from(denominator))?;
    Ok(value.checked_mul(&factor)?)
}

fn rational_from_parts(numerator: i128, denominator: i128) -> Result<Rational, EvalError> {
    Ok(Rational::checked_from_parts(numerator, denominator)?)
}

fn apply_unary_transform<T>(
    transform: &FunctionValue,
    localized: PatternRuntime<T>,
) -> Result<PatternRuntime<T>, EvalError>
where
    T: PatternRuntimeValue,
{
    let transformed =
        apply_function_value(transform.clone(), vec![T::into_runtime_value(localized)])?;
    T::try_from_runtime_value(transformed)
}

/// Calculates a deterministic, pseudorandom Boolean flag indicating if the
/// `sometimes` built-in function should apply its transformation to a given cycle.
///
/// This relies on the absolute cycle number and a lexical "site salt" to ensure
/// multiple usages of `sometimes` do not synchronize their coin flips.
#[doc(hidden)]
pub const fn sometimes_applies_on_cycle(cycle: i128, site_salt: u64) -> bool {
    (deterministic_prng(site_salt, cycle) & 1) != 0
}

const fn deterministic_prng(site_salt: u64, cycle: i128) -> u64 {
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
    state
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
        ArpDirectionValue, BuiltinFn, BuiltinKind, FunctionValue, NumberPatternValue, SampleEvent,
        SamplePatternValue, Value, arp_event_cluster, cycle_span, roll_event_cluster,
        sometimes_applies_on_cycle, strum_event_cluster,
    };
    use orpheus_pattern::{Event, PatternNode, Rational, TimeSpan};

    fn unary_transform(kind: BuiltinKind, args: Vec<Value>) -> FunctionValue {
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
    ) -> FunctionValue {
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
    fn number_pattern_query_unit_degrades_gracefully_on_overflow() {
        // Create an invalid Rational state that guarantees an arithmetic overflow during querying.
        // `query_unit()` checks bounded spans over `[0, 1)`. If we shift a pattern by an offset
        // whose parts cause `Rational::checked_add` to fail when it evaluates, we get an EvalError.
        // The maximum possible rational before bounds failure involves i128::MAX.
        let base = NumberPatternValue::constant(1.0);
        let max_rational = Rational::checked_from_parts(i128::MAX, 1).unwrap();

        // We nest shifts so that they compound inside `try_query_unit`
        let pattern = base.shift(max_rational).shift(max_rational);

        assert!(
            pattern.try_query_unit().is_err(),
            "Expected try_query_unit to fail with bounded arithmetic overflow"
        );

        let query_events = pattern.query_unit();
        assert!(
            query_events.is_empty(),
            "query_unit should degrade to an empty vector on error"
        );
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

    #[test]
    fn strum_leaves_zero_width_number_clusters_unchanged() {
        let zero = Rational::new(1, 2).unwrap();
        let span = TimeSpan::new(zero, zero).unwrap();
        let cluster = vec![
            Event {
                whole: None,
                part: span,
                value: 60.0,
            },
            Event {
                whole: None,
                part: span,
                value: 67.0,
            },
        ];

        let mut events = cluster.clone();
        strum_event_cluster(&mut events).unwrap();
        assert_eq!(events, cluster);
    }

    #[test]
    fn arp_leaves_zero_width_number_clusters_unchanged() {
        let zero = Rational::new(1, 2).unwrap();
        let span = TimeSpan::new(zero, zero).unwrap();
        let cluster = vec![
            Event {
                whole: None,
                part: span,
                value: 60.0,
            },
            Event {
                whole: None,
                part: span,
                value: 67.0,
            },
        ];

        let mut events = cluster.clone();
        let events = arp_event_cluster(&mut events, 5, ArpDirectionValue::Up).unwrap();
        assert_eq!(events, cluster);
    }

    #[test]
    fn chaos_shuffles_events_deterministically() {
        let source = "a = chaos(bd sn cp hh)";
        let module = crate::eval_module(source, crate::ReplMode::Loose).unwrap();
        let pattern = module.get("a").unwrap().as_sample_pattern().unwrap();

        let span = TimeSpan::new(Rational::zero(), Rational::new(2, 1).unwrap()).unwrap();
        let events = pattern.try_query(&span).unwrap();

        assert_eq!(events.len(), 8); // 4 events per cycle

        let c0_names: Vec<_> = events[0..4].iter().map(|e| e.value.sample()).collect();
        let c1_names: Vec<_> = events[4..8].iter().map(|e| e.value.sample()).collect();

        // Timing should be intact
        assert_eq!(events[0].part.start(), &Rational::zero());
        assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
        assert_eq!(events[4].part.start(), &Rational::one());
        assert_eq!(events[4].part.end(), &Rational::new(5, 4).unwrap());

        // Should contain all elements
        let mut c0_sorted = c0_names.clone();
        c0_sorted.sort_unstable();
        assert_eq!(c0_sorted, vec!["bd", "cp", "hh", "sn"]);

        let mut c1_sorted = c1_names.clone();
        c1_sorted.sort_unstable();
        assert_eq!(c1_sorted, vec!["bd", "cp", "hh", "sn"]);

        // C0 and C1 should likely be different permutations
        assert_ne!(c0_names, vec!["bd", "sn", "cp", "hh"]);
        assert_ne!(c0_names, c1_names);
    }

    #[test]
    fn roll_leaves_zero_width_clusters_unchanged() {
        let zero = Rational::new(1, 2).unwrap();
        let span = TimeSpan::new(zero, zero).unwrap();
        let cluster = vec![
            Event {
                whole: None,
                part: span,
                value: SampleEvent::named("sn"),
            },
            Event {
                whole: None,
                part: span,
                value: SampleEvent::named("hh"),
            },
        ];

        let events = roll_event_cluster(&cluster, 5).unwrap();
        assert_eq!(events, cluster);
    }

    /// 👺 Havoc: Prevent massive memory allocation failures from iterators
    /// lying or accurately reporting enormous upper bounds in `size_hint`.
    #[test]
    fn test_havoc_capacity_estimate_oom() {
        struct MaliciousIterator;
        impl Iterator for MaliciousIterator {
            type Item = &'static TimeSpan;
            fn next(&mut self) -> Option<Self::Item> {
                None
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                // Simulate an enormous upper bound
                (0, Some(usize::MAX))
            }
        }

        let zero = Rational::zero();
        let span = TimeSpan::new(zero, Rational::one()).unwrap();

        // This used to cause `memory allocation of ... bytes failed` (SIGABRT)
        let _ = super::compute_event_fragment_boundaries(&span, MaliciousIterator);
    }
}
