use orpheus_pattern::{Rational, TimeSpan};

use crate::eval::{EvalError, f64_to_rational};
use crate::value::{BuiltinFn, BuiltinKind, NumberPatternValue, SamplePatternValue, Value};

pub fn is_sample_identifier(name: &str) -> bool {
    matches!(name, "bd" | "sn" | "cp" | "hh")
}

pub fn builtin_value(name: &str) -> Option<Value> {
    match name {
        "bd" | "sn" | "cp" | "hh" => Some(Value::SamplePattern(SamplePatternValue::atom(name))),
        "every" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Every))),
        "sometimes" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Sometimes))),
        "fast" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Fast))),
        "slow" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Slow))),
        "shift" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Shift))),
        "rev" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Rev))),
        "gain" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Gain))),
        "hpf" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Hpf))),
        "lpf" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Lpf))),
        "pan" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Pan))),
        "pitch" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Pitch))),
        "sample" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Sample))),
        "rate" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Rate))),
        "slice" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Slice))),
        "slice_idx" => Some(Value::Function(BuiltinFn::new(BuiltinKind::SliceIdx))),
        "rand" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Rand))),
        "jux" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Jux))),
        _ => None,
    }
}

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
                Value::NumberPattern(_) | Value::Function(_) | Value::String(_) => unreachable!(),
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
                Value::SamplePattern(_) | Value::Function(_) | Value::String(_) => unreachable!(),
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
    pub const fn new(kind: BuiltinKind) -> Self {
        Self {
            kind,
            bound_args: Vec::new(),
            site_salt: None,
        }
    }

    #[must_use]
    pub const fn with_site_salt(mut self, site_salt: u64) -> Self {
        self.site_salt = Some(site_salt);
        self
    }

    pub(crate) fn apply(self, args: Vec<Value>) -> Result<Value, EvalError> {
        apply_builtin_function(&self, args)
    }
}

pub fn apply_builtin_function(function: &BuiltinFn, args: Vec<Value>) -> Result<Value, EvalError> {
    let kind = function.kind;
    // PRE-ALLOCATE: avoids extra heap allocations when combining bound arguments and explicit arguments.
    let mut combined = Vec::with_capacity(function.bound_args.len() + args.len());
    combined.extend_from_slice(&function.bound_args);
    combined.extend(args);

    if combined.len() < kind.arity() {
        return Ok(Value::Function(BuiltinFn {
            kind,
            bound_args: combined,
            site_salt: function.site_salt,
        }));
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
            Self::Sometimes => "sometimes",
            Self::Fast => "fast",
            Self::Slow => "slow",
            Self::Shift => "shift",
            Self::Rev => "rev",
            Self::Gain => "gain",
            Self::Hpf => "hpf",
            Self::Lpf => "lpf",
            Self::Pan => "pan",
            Self::Pitch => "pitch",
            Self::Sample => "sample",
            Self::Rate => "rate",
            Self::Slice => "slice",
            Self::SliceIdx => "slice_idx",
            Self::Rand => "rand",
            Self::Jux => "jux",
        }
    }

    const fn arity(self) -> usize {
        match self {
            Self::Every | Self::Slice | Self::SliceIdx => 3,
            Self::Sometimes
            | Self::Fast
            | Self::Slow
            | Self::Shift
            | Self::Gain
            | Self::Hpf
            | Self::Lpf
            | Self::Pan
            | Self::Pitch
            | Self::Rate
            | Self::Jux => 2,
            Self::Rev | Self::Sample => 1,
            Self::Rand => 0,
        }
    }

    fn execute(self, function: &BuiltinFn, args: Vec<Value>) -> Result<Value, EvalError> {
        match self {
            Self::Every => apply_every(args),
            Self::Sometimes => apply_sometimes(args, function.site_salt.unwrap_or_default()),
            Self::Fast => apply_fast(args),
            Self::Slow => apply_slow(args),
            Self::Shift => apply_shift(args),
            Self::Rev => apply_rev(args),
            Self::Gain => apply_gain(args),
            Self::Hpf => apply_hpf(args),
            Self::Lpf => apply_lpf(args),
            Self::Pan => apply_pan(args),
            Self::Pitch => apply_pitch(args),
            Self::Sample => apply_sample(args),
            Self::Rate => apply_rate(args),
            Self::Slice => apply_slice(args),
            Self::SliceIdx => apply_slice_idx(args),
            Self::Rand => apply_rand(args, function.site_salt.unwrap_or_default()),
            Self::Jux => apply_jux(args),
        }
    }
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
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`every` expected a pattern as its final argument",
        )),
    }
}

fn apply_jux(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`jux` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`jux` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern_val) => {
            let transform_fn = extract_unary_pattern_transform(transform, "jux", "first")?;
            let transformed_val =
                transform_fn.apply(vec![Value::SamplePattern(pattern_val.clone())])?;
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
        }
        Value::NumberPattern(_) => Err(EvalError::new("`jux` only applies to sample patterns")),
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`jux` expected a sample pattern as its final argument",
        )),
    }
}

fn apply_sometimes(args: Vec<Value>, site_salt: u64) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let transform = args
        .next()
        .ok_or_else(|| EvalError::new("`sometimes` requires a transform argument"))?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`sometimes` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => {
            let transform = extract_unary_pattern_transform(transform, "sometimes", "first")?;
            Ok(Value::SamplePattern(
                pattern.sometimes_with_site_salt(transform, site_salt),
            ))
        }
        Value::NumberPattern(pattern) => {
            let transform = extract_unary_pattern_transform(transform, "sometimes", "first")?;
            Ok(Value::NumberPattern(
                pattern.sometimes_with_site_salt(transform, site_salt),
            ))
        }
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`sometimes` expected a pattern as its final argument",
        )),
    }
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

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(pattern.fast(factor))),
        Value::NumberPattern(pattern) => Ok(Value::NumberPattern(pattern.fast(factor))),
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`fast` expected a pattern as its final argument",
        )),
    }
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

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(pattern.slow(factor))),
        Value::NumberPattern(pattern) => Ok(Value::NumberPattern(pattern.slow(factor))),
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`slow` expected a pattern as its final argument",
        )),
    }
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

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(pattern.shift(offset))),
        Value::NumberPattern(pattern) => Ok(Value::NumberPattern(pattern.shift(offset))),
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`shift` expected a pattern as its final argument",
        )),
    }
}

fn apply_rev(args: Vec<Value>) -> Result<Value, EvalError> {
    let pattern = args
        .into_iter()
        .next()
        .ok_or_else(|| EvalError::new("`rev` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(pattern.rev())),
        Value::NumberPattern(pattern) => Ok(Value::NumberPattern(pattern.rev())),
        Value::Function(_) | Value::String(_) => {
            Err(EvalError::new("`rev` expected a pattern argument"))
        }
    }
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

fn apply_sample(args: Vec<Value>) -> Result<Value, EvalError> {
    let token = extract_string(
        args.into_iter()
            .next()
            .ok_or_else(|| EvalError::new("`sample` requires a token argument"))?,
        "sample",
    )?;
    Ok(Value::SamplePattern(SamplePatternValue::atom(&token)))
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
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
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
            SliceIndexControl::Pattern(control) => pattern.slice_idx_pattern(control, segments),
        })),
        Value::NumberPattern(_) => Err(EvalError::new(
            "`slice_idx` only applies to sample patterns",
        )),
        Value::Function(_) | Value::String(_) => Err(EvalError::new(
            "`slice_idx` expected a sample pattern as its final argument",
        )),
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
        Value::Function(_) | Value::String(_) => Err(EvalError::new(format!(
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

    let integer = format!("{number:.0}").parse::<i64>().map_err(|_| {
        EvalError::new(format!(
            "`{builtin_name}` factor exceeded the supported evaluator range"
        ))
    })?;

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
) -> Result<BuiltinFn, EvalError> {
    let message = format!(
        "`{builtin_name}` requires a unary pattern transform as its {argument_position} argument"
    );
    match transform {
        Value::Function(function) => {
            let remaining = function.kind.arity() - function.bound_args.len();
            if remaining == 1 {
                Ok(function)
            } else {
                Err(EvalError::new(message))
            }
        }
        Value::SamplePattern(_) | Value::NumberPattern(_) | Value::String(_) => {
            Err(EvalError::new(message))
        }
    }
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

    let integer = format!("{number:.0}").parse::<u32>().map_err(|_| {
        EvalError::new(format!(
            "`{context}` exceeded the supported evaluator range"
        ))
    })?;

    Ok(integer)
}

enum NumericControl {
    Constant(f64),
    Pattern(NumberPatternValue),
}

enum SliceIndexControl {
    Constant(u32),
    Pattern(NumberPatternValue),
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
    let pattern = extract_number_pattern(value, "pitch")?;
    if let Ok(semitones) = pattern.constant_value() {
        if !semitones.is_finite() {
            return Err(EvalError::new("`pitch` requires a finite numeric value"));
        }
        return Ok(NumericControl::Constant(semitones));
    }

    validate_numeric_control_pattern(&pattern, "pitch", |value| {
        if value.is_finite() {
            Ok(())
        } else {
            Err(EvalError::new(
                "`pitch` requires finite numeric control values",
            ))
        }
    })?;

    Ok(NumericControl::Pattern(pattern))
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

    Ok(SliceIndexControl::Pattern(pattern))
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

    let index = format!("{value:.0}")
        .parse::<u32>()
        .map_err(|_| EvalError::new("`slice_idx index` exceeded the supported evaluator range"))?;
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

fn validate_slice_control_patterns(
    start_pattern: &NumberPatternValue,
    end_pattern: &NumberPatternValue,
) -> Result<(), EvalError> {
    let unit = TimeSpan::unit();
    let start_events = start_pattern.try_query(&unit)?;
    let end_events = end_pattern.try_query(&unit)?;
    // PRE-ALLOCATE: prevents heap reallocations when collecting span boundaries.
    let mut boundaries = Vec::with_capacity(2 + (start_events.len() + end_events.len()) * 2);
    boundaries.push(unit.start().clone());
    boundaries.push(unit.end().clone());

    for event in &start_events {
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
            boundaries.push(start.clone());
            boundaries.push(end.clone());
        }
    }
    for event in &end_events {
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
            boundaries.push(start.clone());
            boundaries.push(end.clone());
        }
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
        let part = build_control_span(start.clone(), end.clone())?;
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
        Value::SamplePattern(_) | Value::Function(_) | Value::String(_) => Err(EvalError::new(
            format!("`{builtin_name}` requires a numeric pattern argument"),
        )),
    }
}

fn extract_constant_number(value: Value, builtin_name: &str) -> Result<f64, EvalError> {
    match value {
        Value::NumberPattern(pattern) => pattern.constant_value().map_err(|_| {
            EvalError::new(format!(
                "`{builtin_name}` requires a constant number argument"
            ))
        }),
        Value::SamplePattern(_) | Value::Function(_) | Value::String(_) => Err(EvalError::new(
            format!("`{builtin_name}` requires a constant number argument"),
        )),
    }
}

fn extract_string(value: Value, builtin_name: &str) -> Result<String, EvalError> {
    match value {
        Value::String(string) => Ok(string),
        Value::SamplePattern(_) | Value::NumberPattern(_) | Value::Function(_) => Err(
            EvalError::new(format!("`{builtin_name}` requires a string argument")),
        ),
    }
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
        left_events.sort_by(|a, b| a.part.start().cmp(b.part.start()));
        let mut right_events: Vec<_> = events.iter().filter(|e| e.value.pan() > 0.0).collect();
        right_events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

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
}
