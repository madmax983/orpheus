//! Verification that every runnable code snippet shown in the top-level
//! `README.md` is real, working Orpheus syntax.
//!
//! If a README example changes, update the matching case here (and vice versa)
//! so the README's "10-second taste" and feature highlights can never silently
//! drift away from the language the workspace actually implements.

use orpheus_lang::{ReplMode, Value, eval_module};

/// Collect the ordered sample names produced by one cycle of a sample pattern.
fn sample_names(value: &Value) -> Vec<String> {
    value
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap()
        .into_iter()
        .map(|event| event.value.sample().to_owned())
        .collect()
}

/// README "10-second taste", line 1: a bare whitespace sequence is a pattern of
/// four sample events in one cycle.
#[test]
fn taste_sequence_binds_four_events() {
    let module = eval_module("drums = bd sn cp sn", ReplMode::Loose).unwrap();
    assert_eq!(
        sample_names(module.get("drums").unwrap()),
        ["bd", "sn", "cp", "sn"]
    );
}

/// README "10-second taste", line 2: `*` speeds a token up and `|>` pipes the
/// pattern through a control (`gain`), leaving the sample name intact.
#[test]
fn taste_repeat_and_pipe_transform() {
    let module = eval_module("hats = hh*8 |> gain(0.6)", ReplMode::Loose).unwrap();
    let names = sample_names(module.get("hats").unwrap());
    assert_eq!(names.len(), 8);
    assert!(names.iter().all(|name| name == "hh"));
}

/// README "10-second taste", line 3: an inline `voice { }` block defines a
/// user instrument that compiles to a graph-voice value.
#[test]
fn taste_voice_definition_compiles() {
    let module = eval_module(
        "pluck = voice { saw(freq) * ar(gate, 0.001, 0.2) }",
        ReplMode::Loose,
    )
    .unwrap();
    assert!(
        matches!(module.get("pluck").unwrap(), Value::Voice(_)),
        "expected a voice value"
    );
}

/// README "10-second taste", line 4: a user-defined voice name is used as a
/// pattern token, so `bass = pluck ~ pluck ~` schedules the instrument on the
/// grid alongside `~` rests.
#[test]
fn taste_voice_token_plays_in_a_sequence() {
    let source = "pluck = voice { saw(freq) * ar(gate, 0.001, 0.2) }\nbass = pluck ~ pluck ~";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    assert_eq!(
        sample_names(module.get("bass").unwrap()),
        ["pluck", "pluck"]
    );
}

/// Feature highlight: `every(n, transform)` under `|>` applies a transform on a
/// cycle cadence — real curried-builtin syntax, pattern supplied last.
#[test]
fn highlight_every_transform_evaluates() {
    let module = eval_module("riff = bd sn cp sn |> every(2, rev)", ReplMode::Loose).unwrap();
    assert_eq!(sample_names(module.get("riff").unwrap()).len(), 4);
}
