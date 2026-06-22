//! Chaos engineering tests validating the AST evaluator's recursion limits, ensuring deeply nested closures return an error instead of causing a stack overflow.
use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;

#[test]
fn test_havoc_stack_overflow() {
    let source = "f x = x(x)\nomega = f(f)";
    let result = eval_module(source, ReplMode::Loose);
    assert!(result.is_err(), "Expected an error but got: {result:?}");
    assert_eq!(
        result.unwrap_err().to_string(),
        "evaluation recursion limit exceeded"
    );
}

#[test]
fn test_havoc_stack_overflow_meter_eval() {
    let mut source = "bd".to_string();
    for _ in 0..300 {
        source = format!("({source})");
    }
    source = format!("notes = {source}");

    let result = eval_module(&source, ReplMode::Loose);
    // Either parse error or eval error, but not an abort
    if let Err(e) = result {
        assert!(
            e.to_string().contains("exceeded") || e.to_string().contains("limit"),
            "Unexpected error: {e}"
        );
    }
}

#[test]
fn test_havoc_stack_overflow_stream() {
    let mut pattern = "bd".to_string();
    for _ in 0..300 {
        pattern = format!("stream({pattern})");
    }
    let source = format!("x = {pattern}");
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.to_string().contains("exceeded") || err.to_string().contains("limit"),
        "Unexpected error: {err}"
    );
}

#[test]
fn test_havoc_stack_overflow_at() {
    let mut pattern = "bd".to_string();
    for _ in 0..300 {
        pattern = format!("at(0, {pattern})");
    }
    let source = format!("x = {pattern}");
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.to_string().contains("exceeded") || err.to_string().contains("limit"),
        "Unexpected error: {err}"
    );
}

#[test]
fn test_havoc_stack_overflow_meter() {
    let mut pattern = "bd".to_string();
    for _ in 0..300 {
        pattern = format!("meter(4, 4, {pattern})");
    }
    let source = format!("x = {pattern}");
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.to_string().contains("exceeded") || err.to_string().contains("limit"),
        "Unexpected error: {err}"
    );
}
