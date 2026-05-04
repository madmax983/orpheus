use orpheus_lang::EvalError;

#[test]
fn eval_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: EvalError = io_err.into();
    assert!(err.to_string().contains("file not found"));

    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
    let err: EvalError = io_err.into();
    assert!(err.to_string().contains("permission denied"));

    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "other error");
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
