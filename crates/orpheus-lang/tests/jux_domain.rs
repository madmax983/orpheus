//! `jux` pattern-kind coverage (PR 2 of 3 gap-fixes).
//!
//! `jux` juxtaposes: it duplicates an audio-producing pattern, pans the
//! original hard-left and the transform's output hard-right, and stacks the
//! two copies. Panning requires events that carry a pan channel and reach an
//! audio bus. In Orpheus both drum-sample tokens and `voice { ... }` tokens
//! lower to `Value::SamplePattern` (a `SampleEvent` carries `pan`), so `jux`
//! works on either. A bare number / pitch / degree pattern is a control signal
//! (`Value::NumberPattern`, `f64` events) with no pan channel that never
//! reaches an audio bus on its own, so `jux` cannot apply to it — and the
//! error must say so precisely. See docs/adr/0014-jux-pattern-kind-domain.md.

use orpheus_lang::{ReplMode, eval_module};

/// Asserts evaluating `source` fails and the message mentions every fragment.
fn assert_eval_error_contains(source: &str, mode: ReplMode, expected_fragments: &[&str]) {
    let message = eval_module(source, mode).unwrap_err().to_string();
    assert!(
        !message.trim().is_empty(),
        "eval error should not be empty for `{source}`"
    );
    for fragment in expected_fragments {
        assert!(
            message.contains(fragment),
            "eval error `{message}` did not mention required fragment `{fragment}`"
        );
    }
}

#[test]
fn jux_applies_to_voice_token_pattern() {
    // A voice token behaves like a sample token in a pattern, so `jux` pans a
    // left copy and a reversed right copy just as it does for drum samples.
    let source = "acid = voice { sine(freq) * ar(gate, 0.001, 0.08) }\n\
                  line = acid acid ~ |> jux(rev)";
    let module = eval_module(source, ReplMode::Strict).unwrap();
    let pattern = module.get("line").unwrap().as_sample_pattern().unwrap();
    let events = pattern.query_unit().unwrap();

    // Two audible notes per copy, one copy panned left and one panned right.
    let left: Vec<_> = events.iter().filter(|e| e.value.pan() < 0.0).collect();
    let right: Vec<_> = events.iter().filter(|e| e.value.pan() > 0.0).collect();
    assert_eq!(left.len(), 2, "left (original) copy should keep both notes");
    assert_eq!(
        right.len(),
        2,
        "right (reversed) copy should keep both notes"
    );
    assert!(
        left.iter()
            .chain(right.iter())
            .all(|e| e.value.sample() == "acid"),
        "every juxtaposed event should still route to the `acid` voice token"
    );
    #[allow(clippy::float_cmp)] // hard-panned constants pass through unchanged
    {
        assert!(left.iter().all(|e| e.value.pan() == -1.0));
        assert!(right.iter().all(|e| e.value.pan() == 1.0));
    }
}

#[test]
fn jux_on_number_pattern_names_supported_kinds() {
    // A bare number pattern is a control with no pan channel; the error must
    // name the supported kinds (sample and voice patterns) and point at the
    // fix rather than the terse legacy "only applies to sample patterns".
    assert_eval_error_contains(
        "line = 0 3 7 |> jux(rev)",
        ReplMode::Loose,
        &["jux", "pan", "sample", "voice", "control"],
    );
}

#[test]
fn jux_on_pitch_pattern_names_supported_kinds() {
    // Named pitch literals (`c3 e3 g3`) lower to a number pattern too, so they
    // hit the same actionable error.
    assert_eval_error_contains(
        "line = c3 e3 g3 |> jux(rev)",
        ReplMode::Strict,
        &["jux", "pan", "sample", "voice", "control"],
    );
}
