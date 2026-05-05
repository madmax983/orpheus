use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_tiny_float_meter_panic() {
    let source = "notes = meter(0.00000000000000001, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "meter beat count must be a positive integer"
    );
}

#[test]
fn test_havoc_f64_to_rational_out_of_bounds() {
    let source = format!("notes = meter({}, 4, at(beat(0), bd))", 1e100);
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err());
}

#[test]
fn test_havoc_float_nan() {
    let source = format!("notes = {}", f64::NAN);
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err());
}

#[test]
fn test_havoc_pitch_out_of_bounds() {
    use orpheus_lang::parse_named_pitch_literal;
    let res = parse_named_pitch_literal("c99999999999999999999999999999999999999999999999999999");
    assert!(res.is_err());
}

#[test]
fn test_havoc_pitch_accidental_out_of_bounds() {
    use orpheus_lang::parse_named_pitch_literal;
    let res = parse_named_pitch_literal("cs999999999999999999999999999999999999999999999999999999999999999" );
    assert!(res.is_err());
}

#[test]
fn test_havoc_pitch_negative() {
    use orpheus_lang::parse_named_pitch_literal;
    let res = parse_named_pitch_literal("c-4");
    assert!(res.is_err() || res.unwrap().is_none());
}

#[test]
fn test_havoc_negative_time() {
    let source = "notes = shift(-1, bd)";
    let res = eval_module(source, ReplMode::Loose);
    // Is it returning a result with a negative part time?
    let notes = res.unwrap();
    let pat = notes.get("notes").unwrap().as_sample_pattern().unwrap();
    // This calls query but handles it internally
    let res = orpheus_lang::export_sample_pattern_to_csv(pat, "/dev/null", 1);
    assert!(res.is_ok());
}

#[test]
fn test_havoc_export_number_oom() {
    let source = "pattern = 1 2 3";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

    let result = orpheus_lang::export_number_pattern_to_csv(pattern, "test_havoc.csv", u64::MAX);
    assert!(result.is_err());
}
