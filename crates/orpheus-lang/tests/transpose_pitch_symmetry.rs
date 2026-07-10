//! `transpose` / `pitch` argument symmetry — gap-fix 3 of 3 from the
//! reference song (PR #1439).
//!
//! Authoring the reference song surfaced this asymmetry: "`transpose`
//! requires a number pattern (asymmetric with `pitch`, which accepts
//! sample/voice token patterns too); register moves on voices must use
//! `pitch`." Both builtins are the same operation — a semitone shift — so
//! the same call shape must work with either name on either pattern kind:
//!
//! - on a number pattern, the semitones are added to each value
//!   (`transpose`'s historical domain);
//! - on a sample/voice pattern, the playback pitch shifts by the same
//!   semitone amount via the rate multiplier (`pitch`'s historical domain).
//!
//! Constant and pattern-valued controls, pipe position (`p |> transpose(n)`)
//! and direct calls (`transpose(n, p)`) all behave identically for the two
//! names, and both keep typing as `Pattern<Number> -> Pattern<a> -> Pattern<a>`.

use orpheus_lang::{ReplMode, eval_module, infer_module};

/// Queries one cycle of a sample-pattern binding and returns each event's
/// playback-rate multiplier.
fn sample_rates(source: &str, name: &str) -> Vec<f64> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    module
        .get(name)
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap()
        .into_iter()
        .map(|event| event.value.rate())
        .collect()
}

/// Queries one cycle of a number-pattern binding and returns its values.
fn number_values(source: &str, name: &str) -> Vec<f64> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    module
        .get(name)
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit()
        .into_iter()
        .map(|event| event.value)
        .collect()
}

// --- `transpose` on sample/voice patterns (pitch's historical domain) ---

#[test]
fn transpose_shifts_sample_pattern_playback_rate_like_pitch() {
    let transposed = sample_rates(r#"lead = sample("vox_ah") |> transpose(12)"#, "lead");
    let pitched = sample_rates(r#"lead = sample("vox_ah") |> pitch(12)"#, "lead");

    assert_eq!(transposed, pitched);
    assert_eq!(transposed.len(), 1);
    assert!((transposed[0] - 2.0).abs() < f64::EPSILON);
}

#[test]
fn transpose_accepts_pattern_valued_controls_on_sample_patterns() {
    let transposed = sample_rates(r#"lead = sample("vox_ah") |> transpose(0 12)"#, "lead");
    let pitched = sample_rates(r#"lead = sample("vox_ah") |> pitch(0 12)"#, "lead");

    assert_eq!(transposed, pitched);
    assert_eq!(transposed, vec![1.0, 2.0]);
}

#[test]
fn transpose_direct_call_shape_accepts_sample_patterns() {
    let rates = sample_rates(r#"lead = transpose(-12, sample("vox_ah"))"#, "lead");

    assert_eq!(rates.len(), 1);
    assert!((rates[0] - 0.5).abs() < f64::EPSILON);
}

#[test]
fn transpose_moves_voice_pattern_register_like_pitch() {
    // The reference-song shape that motivated this fix: a register move on a
    // pattern of custom-voice tokens previously had to use `pitch`.
    let source = "kit = voice { sample(\"bd\") }\nline = kit kit |> transpose(12)";
    let transposed = sample_rates(source, "line");
    let pitched = sample_rates(
        "kit = voice { sample(\"bd\") }\nline = kit kit |> pitch(12)",
        "line",
    );

    assert_eq!(transposed, pitched);
    assert_eq!(transposed, vec![2.0, 2.0]);
}

// --- `pitch` on number patterns (transpose's historical domain) ---

#[test]
fn pitch_shifts_number_patterns_by_semitones_like_transpose() {
    let pitched = number_values("line = 0 3 7 |> pitch(12)", "line");
    let transposed = number_values("line = 0 3 7 |> transpose(12)", "line");

    assert_eq!(pitched, transposed);
    assert_eq!(pitched, vec![12.0, 15.0, 19.0]);
}

#[test]
fn pitch_accepts_pattern_valued_controls_on_number_patterns() {
    let pitched = number_values("line = degrees(aeolian, 0 2) |> pitch(12 -12)", "line");
    let transposed = number_values("line = degrees(aeolian, 0 2) |> transpose(12 -12)", "line");

    assert_eq!(pitched, transposed);
    assert_eq!(pitched, vec![12.0, -9.0]);
}

#[test]
fn pitch_direct_call_shape_accepts_number_patterns() {
    let values = number_values("melody = pitch(12, c4 e4 g4)", "melody");
    let reference = number_values("melody = transpose(12, c4 e4 g4)", "melody");

    assert_eq!(values, reference);
}

// --- typing: both names share the polymorphic semitone-shift scheme ---

#[test]
fn transpose_preserves_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") |> transpose(0 12 -12)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn pitch_preserves_number_pattern_types() {
    let typed = infer_module("line = 0 3 7 |> pitch(12)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("line").to_string(), "Pattern<Number>");
}

#[test]
fn transpose_and_pitch_still_type_their_historical_domains() {
    let transposed = infer_module(
        "line = degrees(aeolian, 0 2 4) |> transpose(45)",
        ReplMode::Strict,
    )
    .unwrap();
    let pitched = infer_module(
        r#"lead = sample("vox_ah") |> pitch(0 12)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(transposed.type_of("line").to_string(), "Pattern<Number>");
    assert_eq!(pitched.type_of("lead").to_string(), "Pattern<Sample>");
}

// --- both names keep rejecting non-pattern arguments ---

#[test]
fn transpose_and_pitch_reject_non_pattern_final_arguments() {
    for source in ["bad = transpose(1, up)", "bad = pitch(1, up)"] {
        let message = eval_module(source, ReplMode::Loose)
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("pattern"),
            "`{source}` should error mentioning `pattern`, got: {message}"
        );
    }
}
