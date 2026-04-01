use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;
use std::panic;

/// Generate float-literal strings, including cases with extremely long scientific-notation exponents.
fn float_literal_strategy() -> impl Strategy<Value = String> {
    // Strategy for "extreme" scientific-notation literals.
    let extreme = (
        // Optional sign.
        prop_oneof![
            Just(String::new()),
            Just("+".to_string()),
            Just("-".to_string()),
        ],
        // Integer part: 1–10 digits.
        proptest::collection::vec(proptest::char::range('0', '9'), 1..=4)
            .prop_map(|digits| digits.into_iter().collect::<String>()),
        // Optional fractional part: "" or "." followed by 1–10 digits.
        prop_oneof![
            Just(String::new()),
            proptest::collection::vec(proptest::char::range('0', '9'), 1..=4).prop_map(|digits| {
                let frac: String = digits.into_iter().collect();
                format!(".{frac}")
            }),
        ],
        // Optional exponent part, with very long digit sequences (1–1000 digits).
        prop_oneof![
            Just(String::new()),
            (
                prop_oneof![Just("e".to_string()), Just("E".to_string()),],
                prop_oneof![
                    Just(String::new()),
                    Just("+".to_string()),
                    Just("-".to_string()),
                ],
                proptest::collection::vec(proptest::char::range('0', '9'), 1..=4)
                    .prop_map(|digits| digits.into_iter().collect::<String>()),
            )
                .prop_map(|(e, sign, digits)| format!("{e}{sign}{digits}")),
        ],
    )
        .prop_map(|(sign, int_part, frac_part, exp_part)| {
            format!("{sign}{int_part}{frac_part}{exp_part}")
        });

    // Also include "normal" float literals derived from random f64 values.
    let normal = any::<f64>().prop_map(|v| v.to_string());

    prop_oneof![extreme, normal]
}

proptest! {
    #[test]
    fn query_unit_sample_does_not_panic(s in float_literal_strategy()) {
        let source = format!("a = shift({s}, fast({s}, bd))");

        // This is strict: any panic triggered inside `eval_module` or `query_unit`
        // will cause the test to fail. `eval_module` doesn't evaluate the pattern span itself,
        // so we must do it manually via `query_unit()`.
        let result = panic::catch_unwind(|| {
            let Ok(mut values) = eval_module(&source, ReplMode::Loose) else {
                return; // parse errors and eval errors on fuzz strings are expected
            };
            let Some(val) = values.remove("a") else {
                return;
            };
            let Some(pat) = val.as_sample_pattern() else {
                return;
            };
            let _ = pat.query_unit(); // query_unit for SamplePatternValue returns a Result so we ignore it
        });

        // If there was a panic, this assertion will fail.
        assert!(result.is_ok(), "query_unit panicked for input: {s}");
    }

    #[test]
    fn query_unit_number_does_not_panic(s in float_literal_strategy()) {
        // use an expression that heavily multiplies limits
        let source = format!("a = shift({s}, slow({s}, fast({s}, every({s}, rev, sine))))");

        let result = panic::catch_unwind(|| {
            let Ok(mut values) = eval_module(&source, ReplMode::Loose) else {
                return; // parse errors and eval errors on fuzz strings are expected
            };
            let Some(val) = values.remove("a") else {
                return;
            };
            let Some(pat) = val.as_number_pattern() else {
                return;
            };
            // NumberPatternValue::query_unit() panics on internal evaluation errors
            let _ = pat.query_unit();
        });

        // If there was a panic, this assertion will fail.
        assert!(result.is_ok(), "query_unit panicked for input: {s}");
    }
}

#[test]
fn havoc_deeply_nested_group_returns_error_instead_of_stack_overflow() {
    let mut source = "a = ".to_string();
    for _ in 0..1000 {
        source.push('(');
    }
    source.push_str("bd");
    for _ in 0..1000 {
        source.push(')');
    }
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err(), "Expected error from deep recursion");
    assert!(res.unwrap_err().to_string().contains("recursion"));
}

#[test]
fn havoc_deeply_nested_pipe_returns_error_instead_of_stack_overflow() {
    let mut source = "a = ".to_string();
    source.push_str("bd");
    for _ in 0..1000 {
        source.push_str(" |> rev");
    }
    let res = eval_module(&source, ReplMode::Loose);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("recursion"));
}
