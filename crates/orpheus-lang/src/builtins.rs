use crate::eval::EvalError;
use crate::value::{BuiltinFn, BuiltinKind, SamplePatternValue, Value};

pub fn is_sample_identifier(name: &str) -> bool {
    matches!(name, "bd" | "sn" | "cp" | "hh")
}

pub fn builtin_value(name: &str) -> Option<Value> {
    match name {
        "bd" | "sn" | "cp" | "hh" => Some(Value::SamplePattern(SamplePatternValue::atom(name))),
        "fast" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Fast))),
        "slow" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Slow))),
        "rev" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Rev))),
        "gain" => Some(Value::Function(BuiltinFn::new(BuiltinKind::Gain))),
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
                Value::NumberPattern(_) | Value::Function(_) => unreachable!(),
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
                Value::SamplePattern(_) | Value::Function(_) => unreachable!(),
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
        }
    }

    pub(crate) fn apply(self, args: Vec<Value>) -> Result<Value, EvalError> {
        let mut combined = self.bound_args;
        combined.extend(args);

        if combined.len() < self.kind.arity() {
            return Ok(Value::Function(Self {
                kind: self.kind,
                bound_args: combined,
            }));
        }

        if combined.len() > self.kind.arity() {
            return Err(EvalError::new(format!(
                "`{}` expected {} argument(s), got {}",
                self.kind.name(),
                self.kind.arity(),
                combined.len()
            )));
        }

        self.kind.execute(combined)
    }
}

impl BuiltinKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Slow => "slow",
            Self::Rev => "rev",
            Self::Gain => "gain",
        }
    }

    const fn arity(self) -> usize {
        match self {
            Self::Fast | Self::Slow | Self::Gain => 2,
            Self::Rev => 1,
        }
    }

    fn execute(self, args: Vec<Value>) -> Result<Value, EvalError> {
        match self {
            Self::Fast => apply_fast(args),
            Self::Slow => apply_slow(args),
            Self::Rev => apply_rev(args),
            Self::Gain => apply_gain(args),
        }
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
        Value::Function(_) => Err(EvalError::new(
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
        Value::Function(_) => Err(EvalError::new(
            "`slow` expected a pattern as its final argument",
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
        Value::Function(_) => Err(EvalError::new("`rev` expected a pattern argument")),
    }
}

fn apply_gain(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let gain = extract_gain(
        args.next()
            .ok_or_else(|| EvalError::new("`gain` requires a gain argument"))?,
    )?;
    let pattern = args
        .next()
        .ok_or_else(|| EvalError::new("`gain` requires a pattern argument"))?;

    match pattern {
        Value::SamplePattern(pattern) => Ok(Value::SamplePattern(pattern.gain(gain))),
        Value::NumberPattern(_) => Err(EvalError::new(
            "`gain` only applies to sample patterns in Task 5",
        )),
        Value::Function(_) => Err(EvalError::new(
            "`gain` expected a sample pattern as its final argument",
        )),
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

    Ok(integer)
}

fn extract_gain(value: Value) -> Result<f64, EvalError> {
    let gain = extract_constant_number(value, "gain")?;

    if !gain.is_finite() {
        return Err(EvalError::new("`gain` requires a finite numeric value"));
    }

    Ok(gain)
}

fn extract_constant_number(value: Value, builtin_name: &str) -> Result<f64, EvalError> {
    match value {
        Value::NumberPattern(pattern) => pattern.constant_value(),
        Value::SamplePattern(_) | Value::Function(_) => Err(EvalError::new(format!(
            "`{builtin_name}` requires a constant number argument"
        ))),
    }
}
