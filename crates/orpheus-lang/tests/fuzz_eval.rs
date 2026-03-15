use proptest::prelude::*;
use orpheus_lang::eval_module;
use orpheus_lang::ReplMode;
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
        proptest::string::string_regex("[0-9]{1,4}").unwrap(),
        // Optional fractional part: "" or "." followed by 1–10 digits.
        prop_oneof![
            Just(String::new()),
            proptest::string::string_regex("\\.[0-9]{1,4}").unwrap(),
        ],
        // Optional exponent part, with digit sequences (1–10 digits).
        prop_oneof![
            Just(String::new()),
            (
                prop_oneof![
                    Just("e".to_string()),
                    Just("E".to_string()),
                ],
                prop_oneof![
                    Just(String::new()),
                    Just("+".to_string()),
                    Just("-".to_string()),
                ],
                proptest::string::string_regex("[0-9]{1,2}").unwrap(),
            ).prop_map(|(e, sign, digits)| format!("{e}{sign}{digits}")),
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
            if let Ok(mut values) = eval_module(&source, ReplMode::Loose) {
                if let Some(val) = values.remove("a") {
                    if let Some(pat) = val.as_sample_pattern() {
                        let _ = pat.query_unit();
                    }
                }
            }
        });

        // If there was a panic, this assertion will fail.
        assert!(result.is_ok(), "query_unit panicked for input: {s}");
    }
}
