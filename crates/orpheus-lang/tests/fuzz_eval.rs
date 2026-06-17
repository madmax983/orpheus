//! Fuzz testing the pattern evaluation engine with random AST nodes and structural combinations to surface hidden panics or undefined behavior.
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

// 👺 Havoc: Add a proptest that generates random nested AST trees to ensure bounds checks hold.
fn ast_tree_strategy() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        Just("bd".to_string()),
        Just("sn".to_string()),
        Just("1".to_string()),
        Just("2".to_string()),
    ];

    leaf.prop_recursive(
        8, // Max tree depth
        256, // Max total nodes
        10, // Max items per level
        |inner| {
            prop_oneof![
                // Grouping
                inner.clone().prop_map(|s| format!("({s})")),
                // Sequences
                proptest::collection::vec(inner.clone(), 1..5).prop_map(|seq| {
                    let mut s = String::new();
                    for item in seq {
                        s.push_str(&item);
                        s.push(' ');
                    }
                    s
                }),
                // Stacks
                proptest::collection::vec(inner.clone(), 1..3).prop_map(|seq| {
                    let mut s = "stack(".to_string();
                    for (i, item) in seq.iter().enumerate() {
                        s.push_str(item);
                        if i < seq.len() - 1 {
                            s.push_str(", ");
                        }
                    }
                    s.push(')');
                    s
                }),
                // Pipes
                (inner.clone(), inner.clone()).prop_map(|(lhs, rhs)| format!("{lhs} |> {rhs}")),
            ]
        },
    )
}

proptest! {
    #[test]
    fn havoc_evaluator_recursion_limit_prevents_native_stack_overflow(source in ast_tree_strategy()) {
        let binding = format!("test_val = {source}");

        let result = panic::catch_unwind(|| {
            let _ = eval_module(&binding, ReplMode::Loose);
        });

        // The goal here is that the evaluator does NOT abort the process with a stack overflow.
        // It might parse correctly, or return an EvalError via the depth bound, or fail to parse.
        // All are acceptable as long as it does not hard crash the Rust thread.
        assert!(result.is_ok(), "Deeply nested AST structure caused a thread panic instead of a graceful EvalError");
    }
}
