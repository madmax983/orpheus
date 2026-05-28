use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;
use std::panic;

proptest! {
    // Fuzz boundary floats for timing primitives
    #[test]
    fn test_havoc_timing_nan_infinity(param in prop::num::f64::ANY) {
        let sources = [
            format!("notes = delay({})", param),
            format!("notes = chorus_depth({})", param),
            format!("notes = chorus_rate({})", param),
            format!("notes = res({})", param),
            format!("notes = shift({}, bd)", param),
            format!("notes = bd |> slice(0, {})", param),
            format!("notes = bd |> slice_idx({}, 1)", param),
            format!("notes = bd |> pitch({})", param),
        ];

        for source in sources {
            let result = panic::catch_unwind(|| {
                let env = eval_module(&source, ReplMode::Loose);
                if let Ok(mut values) = env {
                    if let Some(val) = values.remove("notes") {
                        if let Some(pat) = val.as_number_pattern() {
                            let _ = pat.query_unit();
                        } else if let Some(pat) = val.as_sample_pattern() {
                            let _ = pat.query_unit();
                        }
                    }
                }
            });
            assert!(result.is_ok(), "Panicked on source: {}", source);
        }
    }

    // Fuzz boundary integers for rhythm/degrees primitives
    #[test]
    fn test_havoc_integers(param in -2147483648..=2147483647i32) {
        let sources = [
            format!("notes = degrees({}, 0 2 4 5 7 9 11)", param),
            format!("notes = bd |> every({}, rev)", param),
            format!("notes = bd |> when({}, 0, rev)", param),
            format!("notes = bd |> when(1, {}, rev)", param),
            format!("notes = bd |> fast({})", param),
            format!("notes = bd |> slow({})", param),
            format!("notes = bd |> roll({})", param),
            format!("notes = bd |> arp({}, up)", param),
            format!("notes = bd |> drop_voice({})", param),
        ];

        for source in sources {
            let result = panic::catch_unwind(|| {
                let env = eval_module(&source, ReplMode::Loose);
                if let Ok(mut values) = env {
                    if let Some(val) = values.remove("notes") {
                        if let Some(pat) = val.as_number_pattern() {
                            let _ = pat.query_unit();
                        } else if let Some(pat) = val.as_sample_pattern() {
                            let _ = pat.query_unit();
                        }
                    }
                }
            });
            assert!(result.is_ok(), "Panicked on source: {}", source);
        }
    }
}

#[test]
fn test_havoc_empty_collections_and_zeros() {
    let sources = vec![
        "notes = degrees(0, [])",
        "t = make_tuning(2.0, 60, [1.0])\nnotes = sample(\"bd\") |> pitch(0) |> with_tuning(t)",
        "notes = sample(\"bd\") |> pitch(0) |> with_tuning(make_tuning(2.0, 60, []))",
        "notes = bd |> fast(0)",
        "notes = bd |> slow(0)",
        "notes = every(0, rev, bd sn)",
        "notes = when(0, 0, rev, bd sn)",
    ];

    for source in sources {
        let result = panic::catch_unwind(|| {
            let env = eval_module(source, ReplMode::Loose);
            if let Ok(mut values) = env {
                if let Some(val) = values.remove("notes") {
                    if let Some(pat) = val.as_sample_pattern() {
                        let _ = pat.query_unit();
                    } else if let Some(pat) = val.as_number_pattern() {
                        let _ = pat.query_unit();
                    }
                }
            }
        });
        assert!(result.is_ok(), "Panicked on source: {}", source);
    }
}
