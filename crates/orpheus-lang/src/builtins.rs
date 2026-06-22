#![allow(unreachable_patterns)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::match_same_arms)]
//! The `builtins` module implements the standard library of pattern transformations and controls.
//!
//! This module houses the execution logic for all primitive functions available in the
//! Orpheus language, mapping parsed AST function calls to operations on `Pattern`s
//! (e.g., `fast`, `jux`, `rev`, `gain`). It handles argument extraction, error reporting
//! for mismatched arity, and the application of polymorphic types during evaluation.
//!
//! Note that functions like `stack_values` handles variadic list processing, whereas types like
//! `BuiltinFn` encapsulate fixed-arity curried transformations.

use orpheus_pattern::{Rational, TimeSpan};

use crate::eval::{EvalError, apply_function_value, f64_to_rational};
use crate::explain::Explain;
use crate::midi_input;
use crate::value::{
    ArpDirectionValue, BuiltinFn, BuiltinKind, FunctionValue, GatePatternValue, NumberPatternValue,
    PitchClassSetValue, PluginPatternValue, SamplePatternValue, Value,
};

/// Checks if an identifier string corresponds to a known built-in audio sample.
///
/// This is used during evaluation to differentiate between function calls,
/// bound variables, and raw sample triggers without needing explicit quotes
/// around sample names in the language syntax.
///
/// # Parameters
/// - `name`: The bare identifier string to check (e.g., `"bd"`).
///
/// # Examples
///
/// ```
/// use orpheus_lang::is_sample_identifier;
///
/// assert!(is_sample_identifier("bd"));
/// assert!(is_sample_identifier("sn"));
/// assert!(!is_sample_identifier("fast")); // This is a function
/// assert!(!is_sample_identifier("foo"));  // Unknown/user variable
/// ```
#[must_use]
pub fn is_sample_identifier(name: &str) -> bool {
    matches!(
        name,
        "bd" | "sn" | "cp" | "hh" | "saw" | "pulse" | "tri" | "noise"
    )
}

const fn builtin_function_value(kind: BuiltinKind) -> Value {
    Value::Function(FunctionValue::Builtin(BuiltinFn::new(kind)))
}

const fn builtin_pitch_class_set_value(value: PitchClassSetValue) -> Value {
    Value::PitchClassSet(value)
}

/// Resolves a string identifier to its built-in [`Value`].
///
/// This provides the base environment of Orpheus, populating the execution
/// context with all standard library functions (like `fast`, `rev`, `stack`)
/// and global constants (like musical scales or raw sample patterns).
///
/// # Parameters
/// - `name`: The variable or function name to look up.
///
/// Returns `Some(Value)` if the name corresponds to a built-in, otherwise `None`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::builtin_value;
///
/// assert!(builtin_value("fast").is_some());
/// assert!(builtin_value("bd").is_some());
/// assert!(builtin_value("unknown_user_func").is_none());
/// ```
#[must_use]
pub fn builtin_value(name: &str) -> Option<Value> {
    lookup_pattern_transform(name)
        .or_else(|| lookup_scale(name))
        .or_else(|| lookup_effect(name))
}

fn lookup_pattern_transform(name: &str) -> Option<Value> {
    match name {
        "bd" | "sn" | "cp" | "hh" | "saw" | "pulse" | "tri" | "noise" => {
            Some(Value::SamplePattern(SamplePatternValue::atom(name)))
        }
        "every" => Some(builtin_function_value(BuiltinKind::Every)),
        "when" => Some(builtin_function_value(BuiltinKind::When)),
        "sometimes" => Some(builtin_function_value(BuiltinKind::Sometimes)),
        "within" => Some(builtin_function_value(BuiltinKind::Within)),
        "mask" => Some(builtin_function_value(BuiltinKind::Mask)),
        "strum" => Some(builtin_function_value(BuiltinKind::Strum)),
        "roll" => Some(builtin_function_value(BuiltinKind::Roll)),
        "arp" => Some(builtin_function_value(BuiltinKind::Arp)),
        "up" => Some(Value::ArpDirection(ArpDirectionValue::Up)),
        "down" => Some(Value::ArpDirection(ArpDirectionValue::Down)),
        "pingpong" | "updown" => Some(Value::ArpDirection(ArpDirectionValue::PingPong)),
        "invert" => Some(builtin_function_value(BuiltinKind::Invert)),
        "drop" => Some(builtin_function_value(BuiltinKind::Drop)),
        "chord" => Some(builtin_function_value(BuiltinKind::Chord)),
        "euclid" => Some(builtin_function_value(BuiltinKind::Euclid)),
        "lsystem" => Some(builtin_function_value(BuiltinKind::Lsystem)),
        "wolfram" => Some(builtin_function_value(BuiltinKind::Wolfram)),
        "pitch_class_set" => Some(builtin_function_value(BuiltinKind::PitchClassSet)),
        "degrees" => Some(builtin_function_value(BuiltinKind::Degrees)),
        "tuning" => Some(builtin_function_value(BuiltinKind::Tuning)),
        "load_scl" => Some(builtin_function_value(BuiltinKind::LoadScl)),
        "tune" => Some(builtin_function_value(BuiltinKind::Tune)),
        "vst" => Some(builtin_function_value(BuiltinKind::Vst)),
        "au" => Some(builtin_function_value(BuiltinKind::Au)),
        "notes" => Some(builtin_function_value(BuiltinKind::Notes)),
        "hex" => Some(builtin_function_value(BuiltinKind::Hex)),
        "bin" => Some(builtin_function_value(BuiltinKind::Bin)),
        _ => None,
    }
}

fn lookup_scale(name: &str) -> Option<Value> {
    match name {
        "ionian" => Some(builtin_pitch_class_set_value(PitchClassSetValue::ionian())),
        "dorian" => Some(builtin_pitch_class_set_value(PitchClassSetValue::dorian())),
        "phrygian" => Some(builtin_pitch_class_set_value(PitchClassSetValue::phrygian())),
        "mixolydian" => Some(builtin_pitch_class_set_value(
            PitchClassSetValue::mixolydian(),
        )),
        "aeolian" => Some(builtin_pitch_class_set_value(PitchClassSetValue::aeolian())),
        "minor_pentatonic" => Some(builtin_pitch_class_set_value(
            PitchClassSetValue::minor_pentatonic(),
        )),
        _ => None,
    }
}

fn lookup_effect(name: &str) -> Option<Value> {
    match name {
        "fast" => Some(builtin_function_value(BuiltinKind::Fast)),
        "slow" => Some(builtin_function_value(BuiltinKind::Slow)),
        "shift" => Some(builtin_function_value(BuiltinKind::Shift)),
        "rev" => Some(builtin_function_value(BuiltinKind::Rev)),
        "gain" => Some(builtin_function_value(BuiltinKind::Gain)),
        "delay" => Some(builtin_function_value(BuiltinKind::Delay)),
        "delay_time" => Some(builtin_function_value(BuiltinKind::DelayTime)),
        "delay_feedback" => Some(builtin_function_value(BuiltinKind::DelayFeedback)),
        "hpf" => Some(builtin_function_value(BuiltinKind::Hpf)),
        "lpf" => Some(builtin_function_value(BuiltinKind::Lpf)),
        "reverb" => Some(builtin_function_value(BuiltinKind::Reverb)),
        "reverb_room" => Some(builtin_function_value(BuiltinKind::ReverbRoom)),
        "reverb_damp" => Some(builtin_function_value(BuiltinKind::ReverbDamp)),
        "cutoff" => Some(builtin_function_value(BuiltinKind::Cutoff)),
        "chorus" => Some(builtin_function_value(BuiltinKind::Chorus)),
        "chorus_depth" => Some(builtin_function_value(BuiltinKind::ChorusDepth)),
        "chorus_rate" => Some(builtin_function_value(BuiltinKind::ChorusRate)),
        "compressor" => Some(builtin_function_value(BuiltinKind::Compressor)),
        "compressor_threshold" => Some(builtin_function_value(BuiltinKind::CompressorThreshold)),
        "compressor_ratio" => Some(builtin_function_value(BuiltinKind::CompressorRatio)),
        "res" => Some(builtin_function_value(BuiltinKind::Res)),
        "drive" => Some(builtin_function_value(BuiltinKind::Drive)),
        "pw" => Some(builtin_function_value(BuiltinKind::Pw)),
        "pan" => Some(builtin_function_value(BuiltinKind::Pan)),
        "pitch" => Some(builtin_function_value(BuiltinKind::Pitch)),
        "transpose" => Some(builtin_function_value(BuiltinKind::Transpose)),
        "sample" => Some(builtin_function_value(BuiltinKind::Sample)),
        "onset" => Some(builtin_function_value(BuiltinKind::Onset)),
        "rate" => Some(builtin_function_value(BuiltinKind::Rate)),
        "slice" => Some(builtin_function_value(BuiltinKind::Slice)),
        "slice_idx" => Some(builtin_function_value(BuiltinKind::SliceIdx)),
        "rand" => Some(builtin_function_value(BuiltinKind::Rand)),
        "jux" => Some(builtin_function_value(BuiltinKind::Jux)),
        "through" => Some(builtin_function_value(BuiltinKind::Through)),
        "cc" | "midi_cc" => Some(builtin_function_value(BuiltinKind::MidiCc)),
        "p" | "param" => Some(builtin_function_value(BuiltinKind::PluginParam)),
        "chaos" => Some(builtin_function_value(BuiltinKind::Chaos)),
        "palindrome" => Some(builtin_function_value(BuiltinKind::Palindrome)),
        _ => None,
    }
}

/// Evaluates a list of values into a stacked sequence pattern.
///
/// The `stack` operation takes multiple patterns and plays them simultaneously,
/// layering them on top of each other while sharing the exact same timeframe.
///
/// # Parameters
/// - `values`: A `Vec` of evaluated [`Value`]s. All values must be of the same
///   pattern type (e.g., all `SamplePatternValue`s or all `NumberPatternValue`s).
///
/// # Errors
///
/// Returns [`EvalError`] if the input vector is empty, or if the values contain
/// mismatched or incompatible types (e.g., trying to stack a number with an audio sample).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{Value, builtin_value, stack_values};
///
/// // Evaluates `stack(bd, sn)` conceptually:
/// let bd = builtin_value("bd").unwrap();
/// let sn = builtin_value("sn").unwrap();
/// let stacked = stack_values(vec![bd, sn]).unwrap();
///
/// assert!(matches!(stacked, Value::SamplePattern(_)));
/// ```
pub fn stack_values(values: Vec<Value>) -> Result<Value, EvalError> {
    if values.is_empty() {
        return Err(EvalError::new("`stack` requires at least one layer"));
    }

    if values
        .iter()
        .all(|value| matches!(value, Value::SamplePattern(_)))
    {
        let patterns = values
            .into_iter()
            .map(|value| match value {
                Value::SamplePattern(pattern) => pattern,
                Value::NumberPattern(_)
                | Value::ArpDirection(_)
                | Value::PitchClassSet(_)
                | Value::Function(_)
                | Value::String(_)
                | Value::Tuning(_)
                | Value::PluginPattern(_)
                | Value::Pedal(_) => unreachable!(),
            })
            .collect();
        return Ok(Value::SamplePattern(SamplePatternValue::stack(patterns)));
    }

    if values
        .iter()
        .all(|value| matches!(value, Value::NumberPattern(_)))
    {
        let patterns = values
            .into_iter()
            .map(|value| match value {
                Value::NumberPattern(pattern) => pattern,
                Value::SamplePattern(_)
                | Value::ArpDirection(_)
                | Value::PitchClassSet(_)
                | Value::Function(_)
                | Value::String(_)
                | Value::Tuning(_)
                | Value::PluginPattern(_)
                | Value::Pedal(_) => unreachable!(),
            })
            .collect();
        return Ok(Value::NumberPattern(
            crate::value::NumberPatternValue::stack(patterns),
        ));
    }

    Err(EvalError::new(
        "`stack` requires all layers to be the same pattern kind",
    ))
}

impl BuiltinFn {
    #[must_use]
    /// Instantiates a raw primitive function descriptor for the language.
    ///
    /// This constructor is primarily used by the parser and AST evaluator to
    /// bind standard library transformations (like `every` or `fast`) into
    /// the runtime environment before execution begins.
    ///
    /// # Examples
    ///
    /// ```rust
    /// // test placeholder
    ///
    /// // Create a representation of the `fast` function.
    ///
    ///
    /// ```
    pub const fn new(kind: BuiltinKind) -> Self {
        Self {
            kind,
            bound_args: Vec::new(),
            site_salt: None,
        }
    }

    #[must_use]
    /// Injects a stable cryptographic salt based on the AST call site.
    ///
    /// Generative operations (like `sometimes` or `rand`) require deterministic
    /// behavior across identical code execution. By salting the builtin with
    /// its syntax tree location, we ensure that random streams are reproducible
    /// and distinct per-call, even if the same builtin is invoked multiple times.
    ///
    /// # Examples
    ///
    /// ```rust
    /// // test placeholder
    ///
    ///
    /// ```
    pub const fn with_site_salt(mut self, site_salt: u64) -> Self {
        self.site_salt = Some(site_salt);
        self
    }

    pub(crate) fn apply(self, args: Vec<Value>) -> Result<Value, EvalError> {
        apply_builtin_function(&self, args)
    }
}

/// Applies arguments to a built-in primitive function.
///
/// This handles the dispatch logic for all standard library functions
/// (like `fast(2, bd)`). It unifies previously bound arguments with
/// newly provided arguments. If the arguments satisfy the function's
/// required arity, it executes the operation and returns the resulting
/// `Value` (usually a transformed `SamplePatternValue`).
///
/// If there are too few arguments, it automatically returns a new
/// curried `BuiltinFn` waiting for the remainder.
///
/// # Parameters
/// - `function`: The built-in function to invoke.
/// - `args`: The list of newly applied values.
///
/// # Errors
///
/// Returns an [`EvalError`] if there is a type mismatch (e.g., passing
/// an audio pattern where a number is expected) or if too many arguments
/// are provided.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{FunctionValue, Value, apply_builtin_function, builtin_value};
///
/// let fast_func = builtin_value("fast").unwrap();
/// let bd = builtin_value("bd").unwrap();
///
/// if let Value::Function(FunctionValue::Builtin(func)) = fast_func {
///     // `fast` takes 2 arguments: a rate and a pattern.
///     // Applying only 1 argument (the rate) returns a new curried function.
///     let curried = apply_builtin_function(&func, vec![bd]).unwrap();
///     assert!(matches!(curried, Value::Function(_)));
/// }
/// ```
pub fn apply_builtin_function(function: &BuiltinFn, args: Vec<Value>) -> Result<Value, EvalError> {
    let kind = function.kind;
    // PRE-ALLOCATE: avoids extra heap allocations when combining bound arguments and explicit arguments.
    let mut combined = Vec::with_capacity(function.bound_args.len() + args.len());
    combined.extend_from_slice(&function.bound_args);
    combined.extend(args);

    if combined.len() < kind.arity() {
        return Ok(Value::Function(FunctionValue::Builtin(BuiltinFn {
            kind,
            bound_args: combined,
            site_salt: function.site_salt,
        })));
    }

    if combined.len() > kind.arity() {
        return Err(EvalError::new(format!(
            "`{}` expected {} argument(s), got {}",
            kind.name(),
            kind.arity(),
            combined.len()
        )));
    }

    kind.execute(function, combined)
}

impl BuiltinKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Every => "every",
            Self::When => "when",
            Self::Sometimes => "sometimes",
            Self::Within => "within",
            Self::Mask => "mask",
            Self::Strum => "strum",
            Self::Roll => "roll",
            Self::Arp => "arp",
            Self::Invert => "invert",
            Self::Drop => "drop",
            Self::Chord => "chord",
            Self::Euclid => "euclid",
            Self::Lsystem => "lsystem",
            Self::Wolfram => "wolfram",
            Self::PitchClassSet => "pitch_class_set",
            Self::Degrees => "degrees",
            Self::Fast => "fast",
            Self::Slow => "slow",
            Self::Shift => "shift",
            Self::Rev => "rev",
            Self::Gain => "gain",
            Self::Delay => "delay",
            Self::DelayTime => "delay_time",
            Self::DelayFeedback => "delay_feedback",
            Self::Hpf => "hpf",
            Self::Lpf => "lpf",
            Self::Reverb => "reverb",
            Self::ReverbRoom => "reverb_room",
            Self::ReverbDamp => "reverb_damp",
            Self::Cutoff => "cutoff",
            Self::Chorus => "chorus",
            Self::ChorusDepth => "chorus_depth",
            Self::ChorusRate => "chorus_rate",
            Self::Compressor => "compressor",
            Self::CompressorThreshold => "compressor_threshold",
            Self::CompressorRatio => "compressor_ratio",
            Self::Res => "res",
            Self::Drive => "drive",
            Self::Pw => "pw",
            Self::Pan => "pan",
            Self::Pitch => "pitch",
            Self::Transpose => "transpose",
            Self::Sample => "sample",
            Self::Onset => "onset",
            Self::Rate => "rate",
            Self::Slice => "slice",
            Self::SliceIdx => "slice_idx",
            Self::Rand => "rand",
            Self::Jux => "jux",
            Self::Through => "through",
            Self::MidiCc => "midi_cc",
            Self::Chaos => "chaos",
            Self::Palindrome => "palindrome",
            Self::Tuning => "tuning",
            Self::LoadScl => "load_scl",
            Self::Tune => "tune",
            Self::Vst => "vst",
            Self::Au => "au",
            Self::Notes => "notes",
            Self::Hex => "hex",
            Self::Bin => "bin",
            Self::PluginParam => "p",
        }
    }

    const fn arity(self) -> usize {
        match self {
            Self::Every | Self::Arp | Self::Slice | Self::SliceIdx | Self::Lsystem => 3,
            Self::When | Self::Within => 4,
            Self::PitchClassSet
            | Self::Rev
            | Self::Sample
            | Self::Strum
            | Self::Chaos
            | Self::Palindrome
            | Self::Tuning
            | Self::LoadScl
            | Self::Vst
            | Self::Au
            | Self::MidiCc
            | Self::Hex
            | Self::Bin => 1,
            Self::Sometimes
            | Self::Mask
            | Self::Roll
            | Self::Invert
            | Self::Drop
            | Self::Chord
            | Self::Euclid
            | Self::Wolfram
            | Self::Degrees
            | Self::Fast
            | Self::Slow
            | Self::Shift
            | Self::Gain
            | Self::Delay
            | Self::DelayTime
            | Self::DelayFeedback
            | Self::Hpf
            | Self::Lpf
            | Self::Reverb
            | Self::ReverbRoom
            | Self::ReverbDamp
            | Self::Cutoff
            | Self::Chorus
            | Self::ChorusDepth
            | Self::ChorusRate
            | Self::Compressor
            | Self::CompressorThreshold
            | Self::CompressorRatio
            | Self::Res
            | Self::Drive
            | Self::Pw
            | Self::Pan
            | Self::Pitch
            | Self::Transpose
            | Self::Onset
            | Self::Rate
            | Self::Jux
            | Self::Through
            | Self::MidiCc
            | Self::Tune
            | Self::Notes => 2,
            Self::PluginParam => 3,
            Self::Rand => 0,
        }
    }

    fn execute(self, function: &BuiltinFn, args: Vec<Value>) -> Result<Value, EvalError> {
        match self {
            Self::Every => apply_every(args),
            Self::When => apply_when(args),
            Self::Sometimes => apply_sometimes(args, function.site_salt.unwrap_or_default()),
            Self::Within => apply_within(args),
            Self::Mask => apply_mask(args),
            Self::Strum => apply_strum(args),
            Self::Roll => apply_roll(args),
            Self::Arp => apply_arp(args),
            Self::Invert => apply_invert(args),
            Self::Drop => apply_drop(args),
            Self::Chord => apply_chord(args),
            Self::Euclid => apply_euclid(args),
            Self::Lsystem => apply_lsystem(args),
            Self::Wolfram => apply_wolfram(args),
            Self::PitchClassSet => apply_pitch_class_set(args),
            Self::Degrees => apply_degrees(args),
            Self::Fast => apply_fast(args),
            Self::Slow => apply_slow(args),
            Self::Shift => apply_shift(args),
            Self::Rev => apply_rev(args),
            Self::Gain => apply_gain(args),
            Self::Delay => apply_delay(args),
            Self::DelayTime => apply_delay_time(args),
            Self::DelayFeedback => apply_delay_feedback(args),
            Self::Hpf => apply_hpf(args),
            Self::Lpf => apply_lpf(args),
            Self::Reverb => apply_reverb(args),
            Self::ReverbRoom => apply_reverb_room(args),
            Self::ReverbDamp => apply_reverb_damp(args),
            Self::Cutoff => apply_cutoff(args),
            Self::Chorus => apply_chorus(args),
            Self::ChorusDepth => apply_chorus_depth(args),
            Self::ChorusRate => apply_chorus_rate(args),
            Self::Compressor => apply_compressor(args),
            Self::CompressorThreshold => apply_compressor_threshold(args),
            Self::CompressorRatio => apply_compressor_ratio(args),
            Self::Res => apply_res(args),
            Self::Drive => apply_drive(args),
            Self::Pw => apply_pw(args),
            Self::Pan => apply_pan(args),
            Self::Pitch => apply_pitch(args),
            Self::Transpose => apply_transpose(args),
            Self::Sample => apply_sample(args),
            Self::Onset => apply_onset(args),
            Self::Rate => apply_rate(args),
            Self::Slice => apply_slice(args),
            Self::SliceIdx => apply_slice_idx(args),
            Self::Rand => apply_rand(args, function.site_salt.unwrap_or_default()),
            Self::Jux => apply_jux(args),
            Self::Through => apply_through(args),
            Self::MidiCc => apply_midi_cc(args),
            Self::Chaos => apply_chaos(args, function.site_salt.unwrap_or_default()),
            Self::Palindrome => apply_palindrome(args),
            Self::Tuning => apply_tuning(args),
            Self::LoadScl => apply_load_scl(args),
            Self::Tune => apply_tune(args),
            Self::Vst => apply_vst(args),
            Self::Au => apply_au(args),
            Self::Notes => apply_plugin_notes(args),
            Self::Hex => apply_hex(args),
            Self::Bin => apply_bin(args),
            Self::PluginParam => apply_plugin_param(args),
        }
    }
}

fn apply_midi_cc(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let controller = extract_constant_number(
        args.next()
            .ok_or_else(|| EvalError::new("`midi_cc` requires a controller argument"))?,
        "midi_cc",
    )?;
    if controller.fract() != 0.0 || !(0.0..=127.0).contains(&controller) {
        return Err(EvalError::new(
            "`midi_cc` requires an integer controller index within [0, 127]",
        ));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let value = midi_input::cc_normalized(controller as u8);
    Ok(Value::NumberPattern(NumberPatternValue::constant(value)))
}

fn apply_through(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let pedal = extract_pedal(
        args.next()
            .ok_or_else(|| EvalError::new("`through` requires a pedal argument"))?,
        "through",
    )?;
    let pattern = extract_sample_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`through` requires a sample pattern argument"))?,
        "through",
    )?;
    let pedal_program = std::sync::Arc::new(orpheus_dsp::PedalProgram::new(
        pedal.format_source(),
        pedal.explain("through"),
    ));

    Ok(Value::SamplePattern(pattern.through(pedal_program)))
}

fn apply_every(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let period = extract_positive_integer_factor(
        args.next()
            .ok_or_else(|| EvalError::new("`every` requires a cycle count argument"))?,
        "every",
    )?;
    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`every` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`every` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => {
            let transform = extract_unary_pattern_transform(transform, "every", "second")?;
            Ok(Value::SamplePattern(pattern.every(period, transform)))
        }
        Value::NumberPattern(pattern) => {
            let transform = extract_unary_pattern_transform(transform, "every", "second")?;
            Ok(Value::NumberPattern(pattern.every(period, transform)))
        }
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::String(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_) => Err(EvalError::new(
            "`every` expected a pattern as its final argument",
        )),
    }
}

fn apply_when(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let period = extract_positive_integer_factor(
        args.next()
            .ok_or_else(|| EvalError::new("`when` requires a cycle period argument"))?,
        "when",
    )?;
    let offset = i64::from(extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`when` requires a cycle offset argument"))?,
        "`when` offset",
        false,
    )?);
    if offset >= period {
        return Err(EvalError::new(
            "`when` requires offset less than the period",
        ));
    }

    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`when` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`when` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| {
            Ok(Value::SamplePattern(p.when(
                period,
                offset,
                extract_unary_pattern_transform(transform.clone(), "when", "third")?,
            )))
        },
        |p| {
            Ok(Value::NumberPattern(p.when(
                period,
                offset,
                extract_unary_pattern_transform(transform.clone(), "when", "third")?,
            )))
        },
        "when",
    )
}

fn apply_jux(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`jux` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`jux` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |pattern_val| {
            let transform_fn = extract_unary_pattern_transform(transform.clone(), "jux", "first")?;
            let transformed_val = apply_function_value(
                transform_fn,
                vec![Value::SamplePattern(pattern_val.clone())],
            )?;
            let Value::SamplePattern(transformed_pattern_val) = transformed_val else {
                return Err(EvalError::new(
                    "`jux` transform must return a sample pattern",
                ));
            };

            let left = pattern_val.pan(-1.0);
            let right = transformed_pattern_val.pan(1.0);
            Ok(Value::SamplePattern(SamplePatternValue::stack(vec![
                left, right,
            ])))
        },
        |_| Err(EvalError::new("`jux` only applies to sample patterns")),
        "jux",
    )
}

fn apply_sometimes(args: Vec<Value>, site_salt: u64) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`sometimes` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`sometimes` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| {
            Ok(Value::SamplePattern(p.sometimes_with_site_salt(
                extract_unary_pattern_transform(transform.clone(), "sometimes", "first")?,
                site_salt,
            )))
        },
        |p| {
            Ok(Value::NumberPattern(p.sometimes_with_site_salt(
                extract_unary_pattern_transform(transform.clone(), "sometimes", "first")?,
                site_salt,
            )))
        },
        "sometimes",
    )
}

fn apply_within(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let start = extract_unit_interval_boundary(
        args.next()
            .ok_or_else(|| EvalError::new("`within` requires a start argument"))?,
        "start",
    )?;
    let end = extract_unit_interval_boundary(
        args.next()
            .ok_or_else(|| EvalError::new("`within` requires an end argument"))?,
        "end",
    )?;
    if start >= end {
        return Err(EvalError::new("`within` requires start < end"));
    }

    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`within` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`within` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| {
            Ok(Value::SamplePattern(p.within(
                start,
                end,
                extract_unary_pattern_transform(transform.clone(), "within", "third")?,
            )))
        },
        |p| {
            Ok(Value::NumberPattern(p.within(
                start,
                end,
                extract_unary_pattern_transform(transform.clone(), "within", "third")?,
            )))
        },
        "within",
    )
}

fn apply_mask(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let gate = extract_pattern_gate(
        args.next()
            .ok_or_else(|| EvalError::new("`mask` requires a gate argument"))?,
        "mask",
        "first",
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`mask` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.mask(gate.clone()))),
        |p| Ok(Value::NumberPattern(p.mask(gate.clone()))),
        "mask",
    )
}

fn apply_euclid(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let pulses = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`euclid` requires a pulses argument"))?,
        "`euclid` pulses",
        false,
    )?;
    let steps = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`euclid` requires a steps argument"))?,
        "`euclid` steps",
        true,
    )?;

    if pulses > steps {
        return Err(EvalError::new(
            "`euclid` requires pulses less than or equal to steps",
        ));
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(
        build_euclid_nodes(pulses, steps),
    )))
}

fn apply_chord(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let root = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`chord` requires a root pattern argument"))?,
        "chord",
    )?;
    let intervals = extract_interval_set(
        args.next()
            .ok_or_else(|| EvalError::new("`chord` requires an interval-set argument"))?,
    )?;

    Ok(Value::NumberPattern(root.chord(&intervals)))
}

fn apply_strum(args: Vec<Value>) -> Result<Value, EvalError> {
    let pattern = extract_number_pattern(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new("`strum` requires a number pattern argument"))?,
        "strum",
    )?;

    Ok(Value::NumberPattern(pattern.strum()))
}

fn apply_roll(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let steps = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`roll` requires a step-count argument"))?,
        "roll",
        true,
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`roll` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.roll(steps))),
        |p| Ok(Value::NumberPattern(p.roll(steps))),
        "roll",
    )
}

fn apply_arp(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let steps = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`arp` requires a step-count argument"))?,
        "arp",
        true,
    )?;
    let direction_value = args
        .next()
        .ok_or_else(|| EvalError::new("`arp` requires a direction argument"))?;
    let direction = extract_arp_direction(&direction_value)?;
    let pattern = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`arp` requires a number pattern argument"))?,
        "arp",
    )?;

    Ok(Value::NumberPattern(pattern.arp(steps, direction)))
}

fn apply_invert(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let count = extract_inversion_count(
        args.next()
            .ok_or_else(|| EvalError::new("`invert` requires an inversion-count argument"))?,
    )?;
    let pattern = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`invert` requires a number pattern argument"))?,
        "invert",
    )?;

    Ok(Value::NumberPattern(pattern.invert(count)))
}

fn apply_drop(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let count = extract_drop_count(
        args.next()
            .ok_or_else(|| EvalError::new("`drop` requires a drop-count argument"))?,
    )?;
    let pattern = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`drop` requires a number pattern argument"))?,
        "drop",
    )?;

    Ok(Value::NumberPattern(pattern.drop_voice(count)))
}

fn apply_pitch_class_set(args: Vec<Value>) -> Result<Value, EvalError> {
    let pattern = extract_number_pattern(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new("`pitch_class_set` requires a pattern argument"))?,
        "pitch_class_set",
    )?;
    let pitch_classes = extract_pitch_class_values(&pattern)?;

    Ok(Value::PitchClassSet(PitchClassSetValue::new(
        pitch_classes,
    )?))
}

fn apply_degrees(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let collection = extract_pitch_class_set(
        args.next()
            .ok_or_else(|| EvalError::new("`degrees` requires a pitch class set argument"))?,
    )?;
    let pattern = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`degrees` requires a pattern argument"))?,
        "degrees",
    )?;
    validate_degree_pattern(&pattern)?;

    Ok(Value::NumberPattern(pattern.degrees(collection)))
}

fn apply_fast(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let factor = extract_positive_integer_factor(
        args.next()
            .ok_or_else(|| EvalError::new("`fast` requires a factor argument"))?,
        "fast",
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`fast` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.fast(factor))),
        |p| Ok(Value::NumberPattern(p.fast(factor))),
        "fast",
    )
}

fn apply_slow(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let factor = extract_positive_integer_factor(
        args.next()
            .ok_or_else(|| EvalError::new("`slow` requires a factor argument"))?,
        "slow",
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`slow` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.slow(factor))),
        |p| Ok(Value::NumberPattern(p.slow(factor))),
        "slow",
    )
}

fn apply_shift(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let offset = extract_constant_rational_offset(
        args.next()
            .ok_or_else(|| EvalError::new("`shift` requires an offset argument"))?,
        "shift",
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`shift` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.shift(offset))),
        |p| Ok(Value::NumberPattern(p.shift(offset))),
        "shift",
    )
}

fn apply_rev(args: Vec<Value>) -> Result<Value, EvalError> {
    let pattern = args
        .into_iter()
        .next()
        .ok_or_else(|| EvalError::new("`rev` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.rev())),
        |p| Ok(Value::NumberPattern(p.rev())),
        "rev",
    )
}

fn apply_chaos(args: Vec<Value>, site_salt: u64) -> Result<Value, EvalError> {
    let pattern = args
        .into_iter()
        .next()
        .ok_or_else(|| EvalError::new("`chaos` requires a pattern argument"))?;

    apply_pattern_transform(
        pattern,
        |p| Ok(Value::SamplePattern(p.chaos_with_site_salt(site_salt))),
        |p| Ok(Value::NumberPattern(p.chaos_with_site_salt(site_salt))),
        "chaos",
    )
}

fn apply_gain(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "gain",
        "gain",
        extract_gain_control,
        SamplePatternValue::gain,
        SamplePatternValue::gain_pattern,
    )
}

fn apply_delay(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "delay",
        "mix",
        |value| extract_unit_interval_control(value, "delay"),
        SamplePatternValue::delay,
        SamplePatternValue::delay_pattern,
    )
}

fn apply_delay_time(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "delay_time",
        "time",
        |value| extract_delay_time_control(value, "delay_time"),
        SamplePatternValue::delay_time,
        SamplePatternValue::delay_time_pattern,
    )
}

fn apply_delay_feedback(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "delay_feedback",
        "feedback",
        |value| extract_unit_interval_control(value, "delay_feedback"),
        SamplePatternValue::delay_feedback,
        SamplePatternValue::delay_feedback_pattern,
    )
}

fn apply_hpf(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "hpf",
        "cutoff",
        |val| extract_filter_cutoff_control(val, "hpf"),
        SamplePatternValue::hpf,
        SamplePatternValue::hpf_pattern,
    )
}

fn apply_lpf(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "lpf",
        "cutoff",
        |val| extract_filter_cutoff_control(val, "lpf"),
        SamplePatternValue::lpf,
        SamplePatternValue::lpf_pattern,
    )
}

fn apply_reverb(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "reverb",
        "mix",
        |value| extract_unit_interval_control(value, "reverb"),
        SamplePatternValue::reverb,
        SamplePatternValue::reverb_pattern,
    )
}

fn apply_reverb_room(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "reverb_room",
        "room",
        |value| extract_unit_interval_control(value, "reverb_room"),
        SamplePatternValue::reverb_room,
        SamplePatternValue::reverb_room_pattern,
    )
}

fn apply_reverb_damp(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "reverb_damp",
        "damp",
        |value| extract_unit_interval_control(value, "reverb_damp"),
        SamplePatternValue::reverb_damp,
        SamplePatternValue::reverb_damp_pattern,
    )
}

fn apply_cutoff(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "cutoff",
        "cutoff",
        |val| extract_filter_cutoff_control(val, "cutoff"),
        SamplePatternValue::cutoff,
        SamplePatternValue::cutoff_pattern,
    )
}

fn apply_res(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "res",
        "resonance",
        extract_resonance_control,
        SamplePatternValue::res,
        SamplePatternValue::res_pattern,
    )
}

fn apply_drive(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "drive",
        "drive",
        extract_drive_control,
        SamplePatternValue::drive,
        SamplePatternValue::drive_pattern,
    )
}

fn apply_chorus(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "chorus",
        "mix",
        |value| extract_unit_interval_control(value, "chorus"),
        SamplePatternValue::chorus,
        SamplePatternValue::chorus_pattern,
    )
}

fn apply_chorus_depth(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "chorus_depth",
        "depth",
        |value| extract_unit_interval_control(value, "chorus_depth"),
        SamplePatternValue::chorus_depth,
        SamplePatternValue::chorus_depth_pattern,
    )
}

fn apply_chorus_rate(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "chorus_rate",
        "rate",
        |value| extract_positive_finite_control(value, "chorus_rate"),
        SamplePatternValue::chorus_rate,
        SamplePatternValue::chorus_rate_pattern,
    )
}

fn apply_pw(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "pw",
        "pulse width",
        extract_pulse_width_control,
        SamplePatternValue::pulse_width,
        SamplePatternValue::pulse_width_pattern,
    )
}

fn apply_pan(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "pan",
        "pan",
        extract_pan_control,
        SamplePatternValue::pan,
        SamplePatternValue::pan_pattern,
    )
}

fn apply_compressor(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "compressor",
        "mix",
        |value| extract_unit_interval_control(value, "compressor"),
        SamplePatternValue::compressor,
        SamplePatternValue::compressor_pattern,
    )
}

fn apply_compressor_threshold(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "compressor_threshold",
        "threshold",
        |value| extract_unit_interval_control(value, "compressor_threshold"),
        SamplePatternValue::compressor_threshold,
        SamplePatternValue::compressor_threshold_pattern,
    )
}

fn apply_compressor_ratio(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "compressor_ratio",
        "ratio",
        |value| extract_compressor_ratio_control(value, "compressor_ratio"),
        SamplePatternValue::compressor_ratio,
        SamplePatternValue::compressor_ratio_pattern,
    )
}

fn apply_pitch(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "pitch",
        "semitone",
        extract_pitch_control,
        SamplePatternValue::pitch,
        SamplePatternValue::pitch_pattern,
    )
}

fn apply_transpose(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let control = extract_transpose_control(
        args.next()
            .ok_or_else(|| EvalError::new("`transpose` requires a semitone argument"))?,
    )?;
    let pattern = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`transpose` requires a pattern argument"))?,
        "transpose",
    )?;

    Ok(Value::NumberPattern(match control {
        NumericControl::Constant(semitones) => pattern.transpose(semitones),
        NumericControl::Pattern(control) => pattern.transpose_pattern(control),
    }))
}

fn apply_tuning(args: Vec<Value>) -> Result<Value, EvalError> {
    let pattern = extract_number_pattern(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new("`tuning` requires a ratio list argument"))?,
        "tuning",
    )?;
    let events = pattern.try_query(&TimeSpan::unit())?;
    if events.is_empty() {
        return Err(EvalError::new("`tuning` requires at least one ratio value"));
    }
    let mut ratios = Vec::with_capacity(events.len());
    for event in events {
        if !event.value.is_finite() {
            return Err(EvalError::new("`tuning` requires finite ratio values"));
        }
        ratios.push(event.value);
    }
    // Convention: the list's final value is the period (e.g. 2.0 for octave).
    let period = ratios
        .pop()
        .ok_or_else(|| EvalError::new("`tuning` requires a period as the last ratio"))?;

    Ok(Value::Tuning(crate::value::TuningValue::new(
        "tuning", ratios, period,
    )?))
}

fn apply_load_scl(args: Vec<Value>) -> Result<Value, EvalError> {
    let path = extract_string(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new("`load_scl` requires a path string argument"))?,
        "load_scl",
    )?;
    let tuning = crate::scl::parse_scala_file(std::path::Path::new(&path))?;
    Ok(Value::Tuning(tuning))
}

fn apply_tune(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let tuning_value = args
        .next()
        .ok_or_else(|| EvalError::new("`tune` requires a tuning argument"))?;
    let tuning = extract_tuning(tuning_value)?;
    let pattern = extract_sample_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`tune` requires a sample pattern argument"))?,
        "tune",
    )?;
    Ok(Value::SamplePattern(pattern.tune(&tuning)))
}

fn apply_vst(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_plugin_descriptor(args, orpheus_dsp::PluginFormat::Vst3, "vst")
}

fn apply_au(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_plugin_descriptor(args, orpheus_dsp::PluginFormat::AudioUnit, "au")
}

fn apply_plugin_descriptor(
    args: Vec<Value>,
    format: orpheus_dsp::PluginFormat,
    builtin_name: &str,
) -> Result<Value, EvalError> {
    let identifier = extract_string(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new(format!("`{builtin_name}` requires a plugin name")))?,
        builtin_name,
    )?;
    let descriptor = orpheus_dsp::PluginDescriptor::try_new(format, identifier)
        .map_err(|error| EvalError::new(error.to_string()))?;
    Ok(Value::PluginPattern(PluginPatternValue::new(
        orpheus_dsp::PluginTrackSource::new(descriptor),
    )))
}

fn apply_plugin_notes(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let notes = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`notes` requires a note pattern argument"))?,
        "notes",
    )?;
    let plugin = extract_plugin_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`notes` requires a plugin argument"))?,
        "notes",
    )?;
    let note_events = notes.try_query_unit()?;
    let events = note_events
        .iter()
        .map(number_event_to_plugin_note)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::PluginPattern(
        plugin.with_notes(events.into_boxed_slice()),
    ))
}

fn apply_plugin_param(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let name = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`p` requires a parameter name argument"))?,
        "p",
    )?;
    let control = extract_number_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`p` requires a control pattern argument"))?,
        "p",
    )?;
    let plugin = extract_plugin_pattern(
        args.next()
            .ok_or_else(|| EvalError::new("`p` requires a plugin argument"))?,
        "p",
    )?;
    let control_events = control.try_query_unit()?;
    let events = control_events
        .iter()
        .map(number_event_to_plugin_parameter)
        .collect::<Result<Vec<_>, _>>()?;
    let lane = orpheus_dsp::PluginParameterLane::new(name, events.into_boxed_slice())
        .map_err(|error| EvalError::new(error.to_string()))?;
    Ok(Value::PluginPattern(plugin.with_parameter_lane(lane)))
}

fn number_event_to_plugin_note(
    event: &orpheus_pattern::Event<f64>,
) -> Result<orpheus_pattern::Event<orpheus_dsp::PluginNote>, EvalError> {
    if !event.value.is_finite()
        || event.value.fract().abs() > f64::EPSILON
        || !(0.0..=127.0).contains(&event.value)
    {
        return Err(EvalError::new(
            "`notes` requires integer MIDI note numbers within [0, 127]",
        ));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let note_number = event.value as u8;
    Ok(orpheus_pattern::Event {
        whole: event.whole,
        part: event.part,
        value: orpheus_dsp::PluginNote::new(note_number, 1.0)
            .map_err(|error| EvalError::new(error.to_string()))?,
    })
}

fn number_event_to_plugin_parameter(
    event: &orpheus_pattern::Event<f64>,
) -> Result<orpheus_pattern::Event<f32>, EvalError> {
    if !event.value.is_finite() || !(0.0..=1.0).contains(&event.value) {
        return Err(EvalError::new(
            "`p` requires normalized parameter values within [0, 1]",
        ));
    }
    #[allow(clippy::cast_possible_truncation)]
    Ok(orpheus_pattern::Event {
        whole: event.whole,
        part: event.part,
        value: event.value as f32,
    })
}

fn extract_tuning(value: Value) -> Result<crate::value::TuningValue, EvalError> {
    match value {
        Value::Tuning(tuning) => Ok(tuning),
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Pedal(_)
        | Value::PluginPattern(_)
        | Value::String(_) => Err(EvalError::new(
            "`tune` expected a tuning value; construct one via `tuning(...)` or `load_scl(\"...\")`",
        )),
    }
}

fn apply_sample(args: Vec<Value>) -> Result<Value, EvalError> {
    let token = extract_string(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new("`sample` requires a token argument"))?,
        "sample",
    )?;
    Ok(Value::SamplePattern(SamplePatternValue::atom(&token)))
}

fn apply_onset(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let index = extract_onset_index_control(
        args.next()
            .ok_or_else(|| EvalError::new("`onset` requires an index argument"))?,
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`onset` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(match index {
            OnsetIndexControl::Constant(index) => pattern.onset(index),
            OnsetIndexControl::Pattern(control) => pattern.onset_pattern(*control),
        })),
        Value::NumberPattern(_) => Err(EvalError::new("`onset` only applies to sample patterns")),
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(
            "`onset` expected a sample pattern as its final argument",
        )),
    }
}

fn apply_rate(args: Vec<Value>) -> Result<Value, EvalError> {
    apply_sample_numeric_control(
        args,
        "rate",
        "rate",
        extract_rate_control,
        SamplePatternValue::rate,
        SamplePatternValue::rate_pattern,
    )
}

fn apply_slice(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let start = extract_slice_endpoint_control(
        args.next()
            .ok_or_else(|| EvalError::new("`slice` requires a start argument"))?,
        "slice start",
    )?;
    let end = extract_slice_endpoint_control(
        args.next()
            .ok_or_else(|| EvalError::new("`slice` requires an end argument"))?,
        "slice end",
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`slice` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(match (start, end) {
            (NumericControl::Constant(start), NumericControl::Constant(end)) => {
                if start >= end {
                    return Err(EvalError::new("`slice` requires start < end"));
                }
                pattern.slice(start, end)
            }
            (start, end) => {
                let start_pattern = numeric_control_to_pattern(start);
                let end_pattern = numeric_control_to_pattern(end);
                validate_slice_control_patterns(&start_pattern, &end_pattern)?;
                pattern.slice_pattern(start_pattern, end_pattern)
            }
        })),
        Value::NumberPattern(_) => Err(EvalError::new("`slice` only applies to sample patterns")),
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(
            "`slice` expected a sample pattern as its final argument",
        )),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn apply_rand(_args: Vec<Value>, site_salt: u64) -> Result<Value, EvalError> {
    Ok(Value::NumberPattern(NumberPatternValue::rand(site_salt)))
}

fn apply_slice_idx(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let index_arg = args
        .next()
        .ok_or_else(|| EvalError::new("`slice_idx` requires an index argument"))?;
    let segments = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`slice_idx` requires a segment count argument"))?,
        "slice_idx segments",
        true,
    )?;
    let index = extract_slice_idx_control(index_arg, segments)?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`slice_idx` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(match index {
            SliceIndexControl::Constant(index) => {
                let (start, end) = slice_idx_bounds(index, segments)?;
                pattern.slice(start, end)
            }
            SliceIndexControl::Pattern(control) => pattern.slice_idx_pattern(*control, segments),
        })),
        Value::NumberPattern(_) => Err(EvalError::new(
            "`slice_idx` only applies to sample patterns",
        )),
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(
            "`slice_idx` expected a sample pattern as its final argument",
        )),
    }
}

fn apply_pattern_transform(
    pattern: Value,
    mut apply_sample: impl FnMut(crate::value::SamplePatternValue) -> Result<Value, EvalError>,
    mut apply_number: impl FnMut(crate::value::NumberPatternValue) -> Result<Value, EvalError>,
    builtin_name: &str,
) -> Result<Value, EvalError> {
    match pattern {
        Value::SamplePattern(p) => apply_sample(p),
        Value::NumberPattern(p) => apply_number(p),
        _ => Err(EvalError::new(format!(
            "`{builtin_name}` expected a pattern argument"
        ))),
    }
}

fn apply_sample_numeric_control(
    args: Vec<Value>,
    builtin_name: &str,
    arg_name: &str,
    extract_control: impl FnOnce(Value) -> Result<NumericControl, EvalError>,
    apply_constant: impl FnOnce(SamplePatternValue, f64) -> SamplePatternValue,
    apply_pattern: impl FnOnce(SamplePatternValue, NumberPatternValue) -> SamplePatternValue,
) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let control_val = extract_control(args.next().ok_or_else(|| {
        EvalError::new(format!("`{builtin_name}` requires a {arg_name} argument"))
    })?)?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new(format!("`{builtin_name}` requires a pattern argument")))?;

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(match control_val {
            NumericControl::Constant(val) => apply_constant(pattern, val),
            NumericControl::Pattern(control) => apply_pattern(pattern, control),
        })),
        Value::NumberPattern(_) => Err(EvalError::new(if builtin_name == "gain" {
            format!("`{builtin_name}` only applies to sample patterns in Task 5")
        } else {
            format!("`{builtin_name}` only applies to sample patterns")
        })),
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(format!(
            "`{builtin_name}` expected a sample pattern as its final argument"
        ))),
    }
}

fn extract_positive_integer_factor(value: Value, builtin_name: &str) -> Result<i64, EvalError> {
    let number = extract_constant_number(value, builtin_name)?;

    if !number.is_finite() || number <= 0.0 || number.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(format!(
            "`{builtin_name}` requires a positive integer factor"
        )));
    }

    #[allow(clippy::cast_possible_truncation)]
    let integer = number.round() as i64;
    #[allow(clippy::cast_precision_loss)]
    let max_i64_as_f64 = i64::MAX as f64;
    if integer == i64::MAX && number > max_i64_as_f64 {
        return Err(EvalError::new(format!(
            "`{builtin_name}` factor exceeded the supported evaluator range"
        )));
    }

    if integer <= 0 {
        return Err(EvalError::new(format!(
            "`{builtin_name}` requires a positive integer factor"
        )));
    }

    if integer > 1024 {
        return Err(EvalError::new(format!(
            "`{builtin_name}` factor exceeded the maximum allowed bound of 1024"
        )));
    }

    Ok(integer)
}

fn extract_constant_rational_offset(
    value: Value,
    builtin_name: &str,
) -> Result<Rational, EvalError> {
    let number = extract_constant_number(value, builtin_name)?;
    f64_to_rational(number, &format!("`{builtin_name}` offset"))
}

fn extract_unary_pattern_transform(
    transform: Value,
    builtin_name: &str,
    argument_position: &str,
) -> Result<FunctionValue, EvalError> {
    let message = format!(
        "`{builtin_name}` requires a unary pattern transform as its {argument_position} argument"
    );
    match transform {
        Value::Function(FunctionValue::Builtin(function)) => {
            let remaining = function.kind.arity() - function.bound_args.len();
            if remaining == 1 {
                Ok(FunctionValue::Builtin(function))
            } else {
                Err(EvalError::new(message))
            }
        }
        Value::Function(FunctionValue::User(function)) => {
            if function.remaining_params.len() == 1 {
                Ok(FunctionValue::User(function))
            } else {
                Err(EvalError::new(message))
            }
        }
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::String(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_) => Err(EvalError::new(message)),
    }
}

fn extract_pattern_gate(
    gate: Value,
    builtin_name: &str,
    argument_position: &str,
) -> Result<GatePatternValue, EvalError> {
    let message =
        format!("`{builtin_name}` requires a pattern gate as its {argument_position} argument");
    match gate {
        Value::SamplePattern(pattern) => Ok(GatePatternValue::Sample(pattern)),
        Value::NumberPattern(pattern) => Ok(GatePatternValue::Number(pattern)),
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(message)),
    }
}

fn build_euclid_nodes(pulses: u32, steps: u32) -> Vec<orpheus_pattern::PatternNode<f64>> {
    let pattern = build_euclid_pattern(pulses, steps);
    pattern
        .into_iter()
        .map(|open| {
            if open {
                orpheus_pattern::PatternNode::atom(1.0)
            } else {
                orpheus_pattern::PatternNode::rest()
            }
        })
        .collect()
}

fn build_euclid_pattern(pulses: u32, steps: u32) -> Vec<bool> {
    if steps == 0 {
        return Vec::new();
    }
    if pulses == 0 {
        return vec![false; usize::try_from(steps).unwrap_or_default()];
    }
    if pulses >= steps {
        return vec![true; usize::try_from(steps).unwrap_or_default()];
    }

    let mut counts = Vec::new();
    let mut remainders = vec![usize::try_from(pulses).unwrap_or_default()];
    let mut divisor = usize::try_from(steps.saturating_sub(pulses)).unwrap_or_default();
    let mut level = 0_usize;

    while remainders[level] > 1 {
        counts.push(divisor / remainders[level]);
        remainders.push(divisor % remainders[level]);
        divisor = remainders[level];
        level += 1;
    }
    counts.push(divisor);

    let mut pattern = Vec::with_capacity(usize::try_from(steps).unwrap_or_default());
    build_euclid_level(
        isize::try_from(level).unwrap_or_default(),
        &counts,
        &remainders,
        &mut pattern,
    );

    if let Some(first_open) = pattern.iter().position(|step| *step) {
        pattern.rotate_left(first_open);
    }

    pattern
}

fn build_euclid_level(
    level: isize,
    counts: &[usize],
    remainders: &[usize],
    pattern: &mut Vec<bool>,
) {
    if level == -1 {
        pattern.push(false);
        return;
    }
    if level == -2 {
        pattern.push(true);
        return;
    }

    let level_index = usize::try_from(level).unwrap_or_default();
    for _ in 0..counts[level_index] {
        build_euclid_level(level - 1, counts, remainders, pattern);
    }
    if remainders[level_index] != 0 {
        build_euclid_level(level - 2, counts, remainders, pattern);
    }
}

fn extract_unit_interval_boundary(value: Value, label: &str) -> Result<Rational, EvalError> {
    let rendered = extract_constant_number(value, "within")?;
    let rational = f64_to_rational(rendered, &format!("`within` {label}"))?;
    if rational < Rational::zero() || rational > Rational::one() {
        return Err(EvalError::new(format!(
            "`within` requires {label} within [0, 1]"
        )));
    }
    Ok(rational)
}

fn extract_whole_number(
    value: Value,
    context: &str,
    positive_only: bool,
) -> Result<u32, EvalError> {
    let number = extract_constant_number(value, context)?;
    let valid = number.is_finite()
        && number >= 0.0
        && number.fract().abs() <= f64::EPSILON
        && (!positive_only || number > 0.0);

    if !valid {
        let requirement = if positive_only {
            "a positive whole number"
        } else {
            "a whole number"
        };
        return Err(EvalError::new(format!(
            "`{context}` requires {requirement}"
        )));
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let integer = number.round() as u32;
    if integer == u32::MAX && number > f64::from(u32::MAX) {
        return Err(EvalError::new(format!(
            "`{context}` exceeded the supported evaluator range"
        )));
    }

    if integer > 1024 {
        return Err(EvalError::new(format!(
            "`{context}` exceeded the maximum allowed bound of 1024"
        )));
    }

    Ok(integer)
}

enum NumericControl {
    Constant(f64),
    Pattern(NumberPatternValue),
}

enum OnsetIndexControl {
    Constant(u32),
    Pattern(Box<NumberPatternValue>),
}

enum SliceIndexControl {
    Constant(u32),
    Pattern(Box<NumberPatternValue>),
}

fn extract_gain_control(value: Value) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, "gain")?;
    if let Ok(gain) = pattern.constant_value() {
        if !gain.is_finite() {
            return Err(EvalError::new("`gain` requires a finite numeric value"));
        }
        return Ok(NumericControl::Constant(gain));
    }

    validate_numeric_control_pattern(&pattern, "gain", |value| {
        if value.is_finite() {
            Ok(())
        } else {
            Err(EvalError::new(
                "`gain` requires finite numeric control values",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_unit_interval_control(
    value: Value,
    builtin_name: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, builtin_name)?;
    if let Ok(number) = pattern.constant_value() {
        if !number.is_finite() || !(0.0..=1.0).contains(&number) {
            return Err(EvalError::new(format!(
                "`{builtin_name}` requires a finite number within [0, 1]"
            )));
        }
        return Ok(NumericControl::Constant(number));
    }

    validate_numeric_control_pattern(&pattern, builtin_name, |value| {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{builtin_name}` requires finite control values within [0, 1]"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_positive_finite_control(
    value: Value,
    builtin_name: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, builtin_name)?;
    if let Ok(number) = pattern.constant_value() {
        if !number.is_finite() || number <= f64::EPSILON {
            return Err(EvalError::new(format!(
                "`{builtin_name}` requires a positive finite numeric value"
            )));
        }
        return Ok(NumericControl::Constant(number));
    }

    validate_numeric_control_pattern(&pattern, builtin_name, |value| {
        if value.is_finite() && value > f64::EPSILON {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{builtin_name}` requires positive finite control values"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_delay_time_control(
    value: Value,
    builtin_name: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, builtin_name)?;
    if let Ok(number) = pattern.constant_value() {
        if !number.is_finite() || number <= f64::EPSILON || number > 1.0 {
            return Err(EvalError::new(format!(
                "`{builtin_name}` requires a positive finite numeric value within (0, 1]"
            )));
        }
        return Ok(NumericControl::Constant(number));
    }

    validate_numeric_control_pattern(&pattern, builtin_name, |value| {
        if value.is_finite() && value > f64::EPSILON && value <= 1.0 {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{builtin_name}` requires positive finite control values within (0, 1]"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_compressor_ratio_control(
    value: Value,
    builtin_name: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, builtin_name)?;
    if let Ok(number) = pattern.constant_value() {
        if !number.is_finite() || number < 1.0 {
            return Err(EvalError::new(format!(
                "`{builtin_name}` requires a finite numeric value >= 1"
            )));
        }
        return Ok(NumericControl::Constant(number));
    }

    validate_numeric_control_pattern(&pattern, builtin_name, |value| {
        if value.is_finite() && value >= 1.0 {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{builtin_name}` requires finite control values >= 1"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_pan_control(value: Value) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, "pan")?;
    if let Ok(pan) = pattern.constant_value() {
        if !pan.is_finite() || !(-1.0..=1.0).contains(&pan) {
            return Err(EvalError::new(
                "`pan` requires a finite number within [-1, 1]",
            ));
        }
        return Ok(NumericControl::Constant(pan));
    }

    validate_numeric_control_pattern(&pattern, "pan", |value| {
        if value.is_finite() && (-1.0..=1.0).contains(&value) {
            Ok(())
        } else {
            Err(EvalError::new(
                "`pan` requires finite control values within [-1, 1]",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_filter_cutoff_control(
    value: Value,
    builtin_name: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, builtin_name)?;
    if let Ok(cutoff_hz) = pattern.constant_value() {
        if !cutoff_hz.is_finite() || cutoff_hz <= f64::EPSILON {
            return Err(EvalError::new(format!(
                "`{builtin_name}` requires a positive finite numeric value"
            )));
        }
        return Ok(NumericControl::Constant(cutoff_hz));
    }

    validate_numeric_control_pattern(&pattern, builtin_name, |value| {
        if value.is_finite() && value > f64::EPSILON {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{builtin_name}` requires positive finite control values"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_rate_control(value: Value) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, "rate")?;
    if let Ok(rate) = pattern.constant_value() {
        if !rate.is_finite() || rate.abs() <= f64::EPSILON {
            return Err(EvalError::new(
                "`rate` requires a finite non-zero numeric value",
            ));
        }
        return Ok(NumericControl::Constant(rate));
    }

    validate_numeric_control_pattern(&pattern, "rate", |value| {
        if value.is_finite() && value.abs() > f64::EPSILON {
            Ok(())
        } else {
            Err(EvalError::new(
                "`rate` requires finite non-zero control values",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_resonance_control(value: Value) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, "res")?;
    if let Ok(resonance) = pattern.constant_value() {
        if !resonance.is_finite() || !(0.0..=1.0).contains(&resonance) {
            return Err(EvalError::new(
                "`res` requires a finite number within [0, 1]",
            ));
        }
        return Ok(NumericControl::Constant(resonance));
    }

    validate_numeric_control_pattern(&pattern, "res", |value| {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(())
        } else {
            Err(EvalError::new(
                "`res` requires finite control values within [0, 1]",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_drive_control(value: Value) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, "drive")?;
    if let Ok(drive) = pattern.constant_value() {
        if !drive.is_finite() || drive < 0.0 {
            return Err(EvalError::new(
                "`drive` requires a finite non-negative numeric value",
            ));
        }
        return Ok(NumericControl::Constant(drive));
    }

    validate_numeric_control_pattern(&pattern, "drive", |value| {
        if value.is_finite() && value >= 0.0 {
            Ok(())
        } else {
            Err(EvalError::new(
                "`drive` requires finite non-negative control values",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_pulse_width_control(value: Value) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, "pw")?;
    if let Ok(pulse_width) = pattern.constant_value() {
        if !pulse_width.is_finite() || !(0.0..1.0).contains(&pulse_width) {
            return Err(EvalError::new(
                "`pw` requires a finite number in the open interval (0, 1)",
            ));
        }
        return Ok(NumericControl::Constant(pulse_width));
    }

    validate_numeric_control_pattern(&pattern, "pw", |value| {
        if value.is_finite() && (0.0..1.0).contains(&value) {
            Ok(())
        } else {
            Err(EvalError::new(
                "`pw` requires finite control values in the open interval (0, 1)",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_slice_endpoint_control(
    value: Value,
    context: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, context)?;
    if let Ok(number) = pattern.constant_value() {
        if !number.is_finite() || !(0.0..=1.0).contains(&number) {
            return Err(EvalError::new(format!(
                "`{context}` must be within the closed interval [0, 1]"
            )));
        }
        return Ok(NumericControl::Constant(number));
    }

    validate_numeric_control_pattern(&pattern, context, |value| {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{context}` requires finite control values within [0, 1]"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn extract_pitch_control(value: Value) -> Result<NumericControl, EvalError> {
    extract_finite_numeric_control(value, "pitch")
}

fn extract_transpose_control(value: Value) -> Result<NumericControl, EvalError> {
    extract_finite_numeric_control(value, "transpose")
}

fn extract_finite_numeric_control(
    value: Value,
    builtin_name: &str,
) -> Result<NumericControl, EvalError> {
    let pattern = extract_number_pattern(value, builtin_name)?;
    if let Ok(semitones) = pattern.constant_value() {
        if !semitones.is_finite() {
            return Err(EvalError::new(format!(
                "`{builtin_name}` requires a finite numeric value"
            )));
        }
        return Ok(NumericControl::Constant(semitones));
    }

    validate_numeric_control_pattern(&pattern, builtin_name, |value| {
        if value.is_finite() {
            Ok(())
        } else {
            Err(EvalError::new(format!(
                "`{builtin_name}` requires finite numeric control values"
            )))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
}

fn validate_degree_pattern(pattern: &NumberPatternValue) -> Result<(), EvalError> {
    let events = pattern.try_query(&TimeSpan::unit())?;
    for event in events {
        if !event.value.is_finite() || event.value.fract().abs() > f64::EPSILON {
            return Err(EvalError::new(
                "`degrees` requires whole-number degree values",
            ));
        }
    }
    Ok(())
}

fn extract_pitch_class_values(pattern: &NumberPatternValue) -> Result<Vec<i32>, EvalError> {
    let events = pattern.try_query(&TimeSpan::unit())?;
    let mut pitch_classes = Vec::with_capacity(events.len());
    for event in events {
        pitch_classes.push(whole_number_from_pitch_class_value(event.value)?);
    }
    Ok(pitch_classes)
}

fn extract_interval_set(value: Value) -> Result<Vec<f64>, EvalError> {
    let pattern = extract_number_pattern(value, "chord")?;
    let events = pattern.try_query(&TimeSpan::unit())?;
    let mut intervals = Vec::with_capacity(events.len());
    for event in events {
        if !event.value.is_finite() {
            return Err(EvalError::new(
                "`chord` requires finite numeric interval values",
            ));
        }
        intervals.push(event.value);
    }
    Ok(intervals)
}

fn extract_inversion_count(value: Value) -> Result<u32, EvalError> {
    let number = extract_constant_number(value, "invert")?;
    if !number.is_finite() {
        return Err(EvalError::new(
            "`invert` requires a finite non-negative whole number",
        ));
    }
    if number < 0.0 {
        return Err(EvalError::new(
            "`invert` requires a non-negative whole number",
        ));
    }
    if number.fract().abs() > f64::EPSILON {
        return Err(EvalError::new("`invert` requires a whole number"));
    }

    if number.is_nan() {
        return Err(EvalError::new("`invert` requires a valid number"));
    }

    if number < 0.0 {
        return Err(EvalError::new(
            "`invert` requires a non-negative whole number",
        ));
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let parsed = number.round() as u32;
    if number > f64::from(u32::MAX) {
        return Err(EvalError::new(
            "`invert` exceeded the supported evaluator range",
        ));
    }
    Ok(parsed)
}

fn extract_drop_count(value: Value) -> Result<u32, EvalError> {
    extract_whole_number(value, "drop", true)
}

fn whole_number_from_pitch_class_value(value: f64) -> Result<i32, EvalError> {
    if !value.is_finite() || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(
            "`pitch_class_set` requires whole number pitch classes",
        ));
    }

    if value.is_nan() {
        return Err(EvalError::new("`pitch_class_set` requires a valid number"));
    }

    #[allow(clippy::cast_possible_truncation)]
    let parsed = value.round() as i32;
    if value > f64::from(i32::MAX) || value < f64::from(i32::MIN) {
        return Err(EvalError::new(
            "`pitch_class_set` exceeded the supported evaluator range",
        ));
    }
    Ok(parsed)
}

fn extract_onset_index_control(value: Value) -> Result<OnsetIndexControl, EvalError> {
    let pattern = extract_number_pattern(value, "onset")?;
    if let Ok(index) = pattern.constant_value() {
        return Ok(OnsetIndexControl::Constant(validate_onset_index_constant(
            index,
        )?));
    }

    validate_numeric_control_pattern(&pattern, "onset", validate_onset_index_control_value)?;

    Ok(OnsetIndexControl::Pattern(Box::new(pattern)))
}

fn extract_slice_idx_control(value: Value, segments: u32) -> Result<SliceIndexControl, EvalError> {
    let pattern = extract_number_pattern(value, "slice_idx")?;
    if let Ok(index) = pattern.constant_value() {
        return Ok(SliceIndexControl::Constant(validate_slice_idx_constant(
            index, segments,
        )?));
    }

    validate_numeric_control_pattern(&pattern, "slice_idx", |value| {
        validate_slice_idx_control_value(value, segments)
    })?;

    Ok(SliceIndexControl::Pattern(Box::new(pattern)))
}

fn validate_numeric_control_pattern<F>(
    pattern: &NumberPatternValue,
    builtin_name: &str,
    validate: F,
) -> Result<(), EvalError>
where
    F: Fn(f64) -> Result<(), EvalError>,
{
    let events = pattern.try_query(&orpheus_pattern::TimeSpan::unit())?;
    if events.is_empty() {
        return Ok(());
    }
    for event in events {
        validate(event.value).map_err(|error| {
            EvalError::new(format!(
                "`{builtin_name}` control pattern is invalid: {error}"
            ))
        })?;
    }
    Ok(())
}

fn validate_slice_idx_constant(value: f64, segments: u32) -> Result<u32, EvalError> {
    if !value.is_finite() || value < 0.0 || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new("`slice_idx index` requires a whole number"));
    }

    if value.is_nan() {
        return Err(EvalError::new("`slice_idx index` requires a valid number"));
    }

    if value < 0.0 {
        return Err(EvalError::new(
            "`slice_idx index` requires a non-negative whole number",
        ));
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = value.round() as u32;
    if value > f64::from(u32::MAX) {
        return Err(EvalError::new(
            "`slice_idx index` exceeded the supported evaluator range",
        ));
    }
    if index >= segments {
        return Err(EvalError::new("`slice_idx` requires index < segments"));
    }

    Ok(index)
}

fn validate_slice_idx_control_value(value: f64, segments: u32) -> Result<(), EvalError> {
    if !value.is_finite() || value < 0.0 || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(
            "`slice_idx` requires whole-number control values",
        ));
    }
    if value >= f64::from(segments) {
        return Err(EvalError::new(
            "`slice_idx` requires control values with index < segments",
        ));
    }

    Ok(())
}

fn validate_onset_index_constant(value: f64) -> Result<u32, EvalError> {
    if !value.is_finite() || value < 0.0 || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new("`onset index` requires a whole number"));
    }

    if value.is_nan() {
        return Err(EvalError::new("`onset index` requires a valid number"));
    }

    if value < 0.0 {
        return Err(EvalError::new(
            "`onset index` requires a non-negative whole number",
        ));
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let parsed = value.round() as u32;
    if value > f64::from(u32::MAX) {
        return Err(EvalError::new(
            "`onset index` exceeded the supported evaluator range",
        ));
    }
    Ok(parsed)
}

fn validate_onset_index_control_value(value: f64) -> Result<(), EvalError> {
    if !value.is_finite() || value < 0.0 || value.fract().abs() > f64::EPSILON {
        return Err(EvalError::new(
            "`onset` requires whole-number control values",
        ));
    }

    Ok(())
}

fn slice_idx_bounds(index: u32, segments: u32) -> Result<(f64, f64), EvalError> {
    let start = f64::from(index) / f64::from(segments);
    let end = f64::from(index.checked_add(1).ok_or_else(|| {
        EvalError::new("`slice_idx index` exceeded the supported evaluator range")
    })?) / f64::from(segments);

    Ok((start, end))
}

fn numeric_control_to_pattern(control: NumericControl) -> NumberPatternValue {
    match control {
        NumericControl::Constant(value) => NumberPatternValue::constant(value),
        NumericControl::Pattern(pattern) => pattern,
    }
}

fn push_span_boundaries(
    events: &[orpheus_pattern::Event<f64>],
    unit: &TimeSpan,
    boundaries: &mut Vec<Rational>,
) {
    for event in events {
        let start = if event.part.start() > unit.start() {
            event.part.start()
        } else {
            unit.start()
        };
        let end = if event.part.end() < unit.end() {
            event.part.end()
        } else {
            unit.end()
        };
        if start < end {
            boundaries.push(*start);
            boundaries.push(*end);
        }
    }
}

fn validate_slice_control_patterns(
    start_pattern: &NumberPatternValue,
    end_pattern: &NumberPatternValue,
) -> Result<(), EvalError> {
    let unit = TimeSpan::unit();
    let start_events = start_pattern.try_query(&unit)?;
    let end_events = end_pattern.try_query(&unit)?;
    // PRE-ALLOCATE: prevents heap reallocations when collecting span boundaries.
    let mut boundaries = Vec::with_capacity(2 + (start_events.len() + end_events.len()) * 2);
    boundaries.push(*unit.start());
    boundaries.push(*unit.end());

    push_span_boundaries(&start_events, &unit, &mut boundaries);
    push_span_boundaries(&end_events, &unit, &mut boundaries);

    boundaries.sort();
    boundaries.dedup();

    for window in boundaries.windows(2) {
        let [start, end] = window else {
            continue;
        };
        if start >= end {
            continue;
        }
        let part = build_control_span(*start, *end)?;
        let mut current_start = 0.0;
        let mut current_end = 1.0;

        for event in &start_events {
            if control_spans_overlap(&event.part, &part) {
                current_start = event.value;
            }
        }
        for event in &end_events {
            if control_spans_overlap(&event.part, &part) {
                current_end = event.value;
            }
        }

        if current_start >= current_end {
            return Err(EvalError::new(
                "`slice` requires control values with start < end",
            ));
        }
    }

    Ok(())
}

fn control_spans_overlap(a: &TimeSpan, b: &TimeSpan) -> bool {
    let start = if a.start() > b.start() {
        a.start()
    } else {
        b.start()
    };
    let end = if a.end() < b.end() { a.end() } else { b.end() };
    start < end
}

fn build_control_span(start: Rational, end: Rational) -> Result<TimeSpan, EvalError> {
    TimeSpan::new(start, end)
        .map_err(|error| EvalError::new(format!("slice control span became invalid: {error}")))
}

fn extract_number_pattern(
    value: Value,
    builtin_name: &str,
) -> Result<NumberPatternValue, EvalError> {
    match value {
        Value::NumberPattern(pattern) => Ok(pattern),
        Value::SamplePattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(format!(
            "`{builtin_name}` requires a number pattern argument"
        ))),
    }
}

fn extract_sample_pattern(
    value: Value,
    builtin_name: &str,
) -> Result<SamplePatternValue, EvalError> {
    match value {
        Value::SamplePattern(pattern) => Ok(pattern),
        Value::NumberPattern(_) => Err(EvalError::new(format!(
            "`{builtin_name}` only applies to sample patterns"
        ))),
        Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(format!(
            "`{builtin_name}` expected a sample pattern argument"
        ))),
    }
}

fn extract_pedal(value: Value, builtin_name: &str) -> Result<crate::pedal::PedalValue, EvalError> {
    match value {
        Value::Pedal(pedal) => Ok(pedal),
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::PluginPattern(_)
        | Value::Tuning(_)
        | Value::String(_) => Err(EvalError::new(format!(
            "`{builtin_name}` requires a pedal argument"
        ))),
    }
}

fn extract_plugin_pattern(
    value: Value,
    builtin_name: &str,
) -> Result<PluginPatternValue, EvalError> {
    match value {
        Value::PluginPattern(plugin) => Ok(plugin),
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Pedal(_)
        | Value::Tuning(_)
        | Value::String(_) => Err(EvalError::new(format!(
            "`{builtin_name}` requires a plugin argument"
        ))),
    }
}

fn extract_constant_number(value: Value, builtin_name: &str) -> Result<f64, EvalError> {
    match value {
        Value::NumberPattern(pattern) => pattern.constant_value().map_err(|_| {
            EvalError::new(format!(
                "`{builtin_name}` requires a constant number argument"
            ))
        }),
        Value::SamplePattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(format!(
            "`{builtin_name}` requires a constant number argument"
        ))),
    }
}

fn extract_string(value: Value, builtin_name: &str) -> Result<String, EvalError> {
    match value {
        Value::String(string) => Ok(string.to_string()),
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::ArpDirection(_)
        | Value::PitchClassSet(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::Function(_) => Err(EvalError::new(format!(
            "`{builtin_name}` requires a string argument"
        ))),
    }
}

fn extract_arp_direction(value: &Value) -> Result<ArpDirectionValue, EvalError> {
    match value {
        Value::ArpDirection(direction) => Ok(*direction),
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::PitchClassSet(_)
        | Value::Function(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::String(_) => Err(EvalError::new(
            "`arp` requires a direction argument like `up`, `down`, `pingpong`, or `updown`",
        )),
    }
}

fn extract_pitch_class_set(value: Value) -> Result<PitchClassSetValue, EvalError> {
    match value {
        Value::PitchClassSet(pitch_class_set) => Ok(pitch_class_set),
        Value::String(name) => Err(EvalError::new(format!(
            "`degrees` now requires a pitch class set value, not a string; use `degrees({name}, ...)` for canonical builtins or `pitch_class_set(...)` for user-defined sets"
        ))),
        Value::SamplePattern(_)
        | Value::NumberPattern(_)
        | Value::ArpDirection(_)
        | Value::Tuning(_)
        | Value::PluginPattern(_)
        | Value::Pedal(_)
        | Value::Function(_) => Err(EvalError::new(
            "`degrees` requires a pitch class set as its first argument",
        )),
    }
}

fn apply_hex(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let text = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`hex` requires a string argument"))?,
        "`hex` string",
    )?;

    let mut nodes = Vec::new();
    for ch in text.chars() {
        if let Some(val) = ch.to_digit(16) {
            for i in (0..4).rev() {
                if (val & (1 << i)) != 0 {
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                } else {
                    nodes.push(orpheus_pattern::PatternNode::rest());
                }
            }
        }
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)))
}

fn apply_bin(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let text = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`bin` requires a string argument"))?,
        "`bin` string",
    )?;

    let mut nodes = Vec::new();
    for ch in text.chars() {
        if ch == '1' {
            nodes.push(orpheus_pattern::PatternNode::atom(1.0));
        } else if ch == '0' {
            nodes.push(orpheus_pattern::PatternNode::rest());
        }
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)))
}

#[cfg(test)]
mod tests {
    // use super::*
    use crate::{ReplMode, eval_module};

    #[test]
    fn jux_applies_transform_and_pans() {
        let source = "a = jux(rev, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("a").unwrap().as_sample_pattern().unwrap();

        let events = pattern.query_unit().unwrap();

        // original (bd sn) panned left
        // rev(bd sn) -> (sn bd) panned right
        // So we expect 4 events:
        // [0, 1/2]: bd (pan -1.0)
        // [0, 1/2]: sn (pan 1.0)
        // [1/2, 1]: sn (pan -1.0)
        // [1/2, 1]: bd (pan 1.0)

        assert_eq!(events.len(), 4);

        let mut left_events: Vec<_> = events.iter().filter(|e| e.value.pan() < 0.0).collect();
        left_events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));
        let mut right_events: Vec<_> = events.iter().filter(|e| e.value.pan() > 0.0).collect();
        right_events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

        assert_eq!(left_events.len(), 2);
        assert_eq!(right_events.len(), 2);

        assert_eq!(left_events[0].value.sample(), "bd");
        assert_eq!(left_events[1].value.sample(), "sn");

        assert_eq!(right_events[0].value.sample(), "sn");
        assert_eq!(right_events[1].value.sample(), "bd");
    }

    #[test]
    fn havoc_fast_and_slow_reject_huge_factors() {
        let source_fast = "a = fast(2048, bd)";
        let result_fast = eval_module(source_fast, ReplMode::Loose);
        assert!(
            result_fast.is_err(),
            "expected fast with huge factor to be rejected"
        );
        let err_msg = result_fast.unwrap_err().to_string();
        assert!(
            err_msg.contains("maximum allowed bound of 1024"),
            "unexpected error message: {err_msg}"
        );

        let source_slow = "b = slow(2048, bd)";
        let result_slow = eval_module(source_slow, ReplMode::Loose);
        assert!(
            result_slow.is_err(),
            "expected slow with huge factor to be rejected"
        );
        let err_msg = result_slow.unwrap_err().to_string();
        assert!(
            err_msg.contains("maximum allowed bound of 1024"),
            "unexpected error message: {err_msg}"
        );
    }

    #[test]
    fn chaos_rejects_invalid_types() {
        let test_cases = vec![
            "a = chaos(\"string\")",
            "a = chaos(rev)",
            "a = chaos(every)",
        ];

        for source in test_cases {
            let result = eval_module(source, ReplMode::Loose);
            assert!(
                result.is_err(),
                "expected chaos with invalid argument to be rejected: {source}"
            );
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("`chaos` expected a pattern argument"),
                "unexpected error message for {source}: {err_msg}"
            );
        }
    }

    #[test]
    fn midi_cc_evaluates_and_rejects_invalid() {
        let test_cases = vec![
            ("a = midi_cc(1)", true),
            ("a = cc(127)", true),
            ("a = cc(0)", true),
            ("a = cc(128)", false),
            ("a = cc(-1)", false),
            ("a = cc(1.5)", false),
        ];

        for (source, is_valid) in test_cases {
            let result = eval_module(source, ReplMode::Loose);
            if is_valid {
                assert!(
                    result.is_ok(),
                    "expected valid midi_cc evaluation: {source}, but got {result:?}"
                );
            } else {
                assert!(
                    result.is_err(),
                    "expected invalid midi_cc evaluation to fail: {source}"
                );
                let err_msg = result.unwrap_err().to_string();
                assert!(
                    err_msg
                        .contains("`midi_cc` requires an integer controller index within [0, 127]"),
                    "unexpected error message for {source}: {err_msg}"
                );
            }
        }
    }
}

fn apply_palindrome(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`palindrome` requires a pattern argument"))?;

    let Value::Function(rev_function) = builtin_function_value(BuiltinKind::Rev) else {
        unreachable!("BuiltinKind::Rev always returns a FunctionValue")
    };

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(pattern.every(2, rev_function))),
        Value::NumberPattern(pattern) => Ok(Value::NumberPattern(pattern.every(2, rev_function))),
        _ => Err(EvalError::new(
            "`palindrome` expects a sample or number pattern",
        )),
    }
}

#[cfg(test)]
mod test_nova {
    use crate::{ReplMode, eval_module};
    use orpheus_pattern::{Rational, TimeSpan};

    #[test]
    fn test_lsystem_builtin() {
        let source = "pat = lsystem(\"A\", 3, \"A:AB,B:A\")";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pat").unwrap().as_number_pattern().unwrap();

        let span = TimeSpan::new(Rational::zero(), Rational::one()).unwrap();
        let events = pattern.try_query(&span).unwrap();

        // A -> AB -> ABA -> ABAAB
        // A=0, B=1, A=0, A=0, B=1
        assert_eq!(events.len(), 5);
        assert!((events[0].value - 0.0).abs() < f64::EPSILON);
        assert!((events[1].value - 1.0).abs() < f64::EPSILON);
        assert!((events[2].value - 0.0).abs() < f64::EPSILON);
        assert!((events[3].value - 0.0).abs() < f64::EPSILON);
        assert!((events[4].value - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_palindrome_builtin() {
        let source = "pat = palindrome(bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pat").unwrap().as_sample_pattern().unwrap();

        // 2 cycles
        let span = TimeSpan::new(Rational::zero(), Rational::new(2, 1).unwrap()).unwrap();
        let events = pattern.try_query(&span).unwrap();

        assert_eq!(events.len(), 4);
        assert_eq!(events[0].value.sample(), "sn");
        assert_eq!(events[0].part.start(), &Rational::new(0, 1).unwrap());

        assert_eq!(events[1].value.sample(), "bd");
        assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());

        assert_eq!(events[2].value.sample(), "bd");
        assert_eq!(events[2].part.start(), &Rational::new(1, 1).unwrap());

        assert_eq!(events[3].value.sample(), "sn");
        assert_eq!(events[3].part.start(), &Rational::new(3, 2).unwrap());
    }
}

fn apply_wolfram(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let rule = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`wolfram` requires a rule argument"))?,
        "`wolfram` rule",
        false,
    )?;
    let steps = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`wolfram` requires a steps argument"))?,
        "`wolfram` steps",
        true,
    )?;
    #[allow(clippy::cast_possible_truncation)]
    let rule_num = rule as u8;
    let mut current_state = vec![false; steps as usize];
    if steps > 0 {
        current_state[steps as usize / 2] = true; // center pixel
    }

    // ⚡ Bolt: Pre-allocate capacity to avoid reallocation in nested loop
    let mut nodes = Vec::with_capacity((steps as usize).saturating_mul(steps as usize));
    for _ in 0..steps {
        // Record current state
        for cell in &current_state {
            if *cell {
                nodes.push(orpheus_pattern::PatternNode::atom(1.0));
            } else {
                nodes.push(orpheus_pattern::PatternNode::rest());
            }
        }

        // Calculate next state
        let mut next_state = vec![false; steps as usize];
        for i in 0..steps as usize {
            let left = if i == 0 { false } else { current_state[i - 1] };
            let center = current_state[i];
            let right = if i == steps as usize - 1 {
                false
            } else {
                current_state[i + 1]
            };

            let neighborhood = (u8::from(left) << 2) | (u8::from(center) << 1) | u8::from(right);
            next_state[i] = (rule_num & (1 << neighborhood)) != 0;
        }
        current_state = next_state;
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)))
}

#[cfg(test)]
mod wolfram_tests {
    use crate::{ReplMode, eval_module};
    use orpheus_pattern::Rational;

    #[test]
    fn test_wolfram_rule_30() {
        let source = "w = wolfram(30, 3)";
        let result = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = result.get("w").unwrap().as_number_pattern().unwrap();

        let span = orpheus_pattern::TimeSpan::new(Rational::zero(), Rational::one()).unwrap();
        let events = pattern.try_query(&span).unwrap();

        assert_eq!(events.len(), 5);
    }
}

fn apply_lsystem(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let axiom = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`lsystem` requires an axiom argument"))?,
        "`lsystem` axiom",
    )?;
    let iterations = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`lsystem` requires an iterations argument"))?,
        "`lsystem` iterations",
        false,
    )?;
    let rules_str = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`lsystem` requires a rules argument"))?,
        "`lsystem` rules",
    )?;

    // Parse rules: "A:AB,B:A"
    let mut rules = std::collections::HashMap::new();
    for rule in rules_str.split(',') {
        let parts: Vec<&str> = rule.split(':').collect();
        if parts.len() == 2 {
            let key = parts[0].trim().chars().next().ok_or_else(|| {
                EvalError::new("`lsystem` rules must have a single character key")
            })?;
            rules.insert(key, parts[1].trim().to_string());
        } else if !rule.trim().is_empty() {
            return Err(EvalError::new(
                "`lsystem` rules must be formatted as 'A:AB,B:A'",
            ));
        }
    }

    let mut current = axiom;
    for _ in 0..iterations {
        let mut next = String::new();
        for c in current.chars() {
            if let Some(replacement) = rules.get(&c) {
                next.push_str(replacement);
            } else {
                next.push(c);
            }
        }
        current = next;
    }

    // Convert to nodes. A=0, B=1, C=2, etc. ~ or _ = rest.
    // ⚡ Bolt: Pre-allocate vector capacity to avoid heap reallocation based on exact final string length.
    let mut nodes = Vec::with_capacity(current.len());
    for c in current.chars() {
        if c == '~' || c == '_' {
            nodes.push(orpheus_pattern::PatternNode::rest());
        } else if c.is_ascii_alphabetic() {
            let val = if c.is_ascii_uppercase() {
                c as u8 - b'A'
            } else {
                c as u8 - b'a'
            };
            nodes.push(orpheus_pattern::PatternNode::atom(f64::from(val)));
        } else if let Some(digit) = c.to_digit(10) {
            nodes.push(orpheus_pattern::PatternNode::atom(f64::from(digit)));
        } else {
            nodes.push(orpheus_pattern::PatternNode::rest());
        }
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)))
}

#[cfg(test)]
mod hex_bin_tests {
    use super::*;

    fn assert_one(value: f64) {
        assert_eq!(value.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn test_hex_builtin() {
        let text = std::sync::Arc::from("89a");
        let result = apply_hex(vec![Value::String(text)]).unwrap();
        let pattern = result.as_number_pattern().unwrap();
        let span = orpheus_pattern::TimeSpan::new(
            orpheus_pattern::Rational::new(0, 1).unwrap(),
            orpheus_pattern::Rational::new(1, 1).unwrap(),
        )
        .unwrap();
        let events = pattern.try_query(&span).unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(
            events[0].part.start(),
            &orpheus_pattern::Rational::new(0, 12).unwrap()
        );
        assert_one(events[0].value);
        assert_eq!(
            events[1].part.start(),
            &orpheus_pattern::Rational::new(4, 12).unwrap()
        );
        assert_one(events[1].value);
        assert_eq!(
            events[2].part.start(),
            &orpheus_pattern::Rational::new(7, 12).unwrap()
        );
        assert_one(events[2].value);
        assert_eq!(
            events[3].part.start(),
            &orpheus_pattern::Rational::new(8, 12).unwrap()
        );
        assert_one(events[3].value);
        assert_eq!(
            events[4].part.start(),
            &orpheus_pattern::Rational::new(10, 12).unwrap()
        );
        assert_one(events[4].value);
    }

    #[test]
    fn test_bin_builtin() {
        let text = std::sync::Arc::from("101");
        let result = apply_bin(vec![Value::String(text)]).unwrap();
        let pattern = result.as_number_pattern().unwrap();
        let span = orpheus_pattern::TimeSpan::new(
            orpheus_pattern::Rational::new(0, 1).unwrap(),
            orpheus_pattern::Rational::new(1, 1).unwrap(),
        )
        .unwrap();
        let events = pattern.try_query(&span).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].part.start(),
            &orpheus_pattern::Rational::new(0, 3).unwrap()
        );
        assert_one(events[0].value);
        assert_eq!(
            events[1].part.start(),
            &orpheus_pattern::Rational::new(2, 3).unwrap()
        );
        assert_one(events[1].value);
    }
}

#[cfg(test)]
mod hex_bin_error_tests {
    use super::*;

    #[test]
    fn test_hex_builtin_missing_arg() {
        let result = apply_hex(vec![]);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`hex` requires a string argument"
        );
    }

    #[test]
    fn test_bin_builtin_missing_arg() {
        let result = apply_bin(vec![]);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "`bin` requires a string argument"
        );
    }
}
