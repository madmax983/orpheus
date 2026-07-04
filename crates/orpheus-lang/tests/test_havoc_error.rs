//! Chaos tests verifying that deep IO errors, parser failures, and evaluation issues are correctly transformed into safe `EvalError`s instead of panics.
use orpheus_lang::EvalError;

#[test]
fn eval_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: EvalError = io_err.into();
    assert!(err.to_string().contains("file not found"));

    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
    let err: EvalError = io_err.into();
    assert!(err.to_string().contains("permission denied"));

    let io_err = std::io::Error::other("other error");
    let err: EvalError = io_err.into();
    assert!(err.to_string().contains("other error"));
}

#[test]
fn eval_error_from_fmt_error() {
    let fmt_err = std::fmt::Error;
    let err: EvalError = fmt_err.into();
    assert!(
        err.to_string()
            .contains("an error occurred when formatting an argument")
    );
}

#[test]
fn eval_error_from_scl_error() {
    use orpheus_lang::SclError;
    let scl_err = SclError::Header("missing header".into());
    let err: EvalError = scl_err.into();
    assert!(
        err.to_string()
            .contains("malformed Scala header: missing header")
    );
}

#[test]
fn eval_error_from_try_from_int_error() {
    let int_err = u8::try_from(256u16).unwrap_err();
    let err: EvalError = int_err.into();
    assert!(
        err.to_string()
            .contains("out of range integral type conversion attempted")
    );
}

#[test]
fn eval_error_from_parse_int_error() {
    let parse_err = "abc".parse::<u8>().unwrap_err();
    let err: EvalError = parse_err.into();
    assert!(err.to_string().contains("invalid digit found in string"));
}

#[test]
fn eval_error_from_pattern_error() {
    use orpheus_pattern::PatternError;
    let pattern_err = PatternError::InvalidDenominator { denominator: 0 };
    let err: EvalError = pattern_err.into();
    assert!(
        err.to_string()
            .contains("rational denominator cannot be zero")
    );
}

#[test]
fn eval_error_from_type_error() {
    use orpheus_lang::TypeError;
    let type_err = TypeError::new("mock type error");
    let err: EvalError = type_err.into();
    assert_eq!(err.to_string(), "mock type error");
}

#[test]
fn eval_error_from_load_error() {
    use orpheus_lang::LoadError;
    let load_err = LoadError::new("mock load error");
    let err: EvalError = load_err.into();
    assert_eq!(err.to_string(), "mock load error");
}

#[test]
fn eval_error_from_parse_error() {
    use orpheus_lang::ParseError;
    let parse_err = ParseError::new("mock parse error");
    let err: EvalError = parse_err.into();
    assert_eq!(err.to_string(), "mock parse error");
}

#[test]
fn type_error_from_parse_error() {
    use orpheus_lang::{ParseError, TypeError};
    let parse_err = ParseError::new("mock parse error");
    let type_err: TypeError = parse_err.into();
    assert_eq!(type_err.to_string(), "mock parse error");
}
