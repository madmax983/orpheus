use orpheus_lang::{ReplMode, Value, eval_module};

fn sample_names(value: &Value) -> Vec<String> {
    value
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .into_iter()
        .map(|event| event.value.sample().to_owned())
        .collect()
}

fn assert_eval_error_contains(source: &str, mode: ReplMode, expected_fragments: &[&str]) {
    let error = eval_module(source, mode).unwrap_err();
    let message = error.to_string();

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
fn evaluating_sequence_produces_sample_pattern() {
    let module = eval_module("drums = bd sn cp sn", ReplMode::Loose).unwrap();
    match module.get("drums").unwrap() {
        Value::SamplePattern(pattern) => {
            let events = pattern.query_unit();
            assert_eq!(events.len(), 4);
            assert_eq!(
                sample_names(module.get("drums").unwrap()),
                ["bd", "sn", "cp", "sn"]
            );
        }
        other => panic!("expected sample pattern, got {other:?}"),
    }
}

#[test]
fn evaluating_hh_resolves_to_a_sample_pattern() {
    let module = eval_module("hats = hh", ReplMode::Loose).unwrap();
    assert_eq!(sample_names(module.get("hats").unwrap()), ["hh"]);
}

#[test]
fn stack_merges_parallel_layers() {
    let module = eval_module("drums = stack(bd ~, ~ sn)", ReplMode::Loose).unwrap();
    let pattern = module.get("drums").unwrap().as_sample_pattern().unwrap();
    assert_eq!(pattern.query_unit().len(), 2);
    assert_eq!(sample_names(module.get("drums").unwrap()), ["bd", "sn"]);
}

#[test]
fn fast_pipe_repeats_the_pattern_within_the_cycle() {
    let module = eval_module("drums = bd sn |> fast(2)", ReplMode::Loose).unwrap();
    assert_eq!(
        sample_names(module.get("drums").unwrap()),
        ["bd", "sn", "bd", "sn"]
    );
}

#[test]
fn slow_pipe_stretches_the_pattern() {
    let module = eval_module("drums = bd sn |> slow(2)", ReplMode::Loose).unwrap();
    assert_eq!(sample_names(module.get("drums").unwrap()), ["bd"]);
}

#[test]
fn rev_reverses_events_within_each_cycle() {
    let module = eval_module("drums = bd sn cp sn |> rev", ReplMode::Loose).unwrap();
    assert_eq!(
        sample_names(module.get("drums").unwrap()),
        ["sn", "cp", "sn", "bd"]
    );
}

#[test]
fn direct_call_matches_pipe_application_for_fast() {
    let direct = eval_module("drums = fast(2, bd sn)", ReplMode::Loose).unwrap();
    let piped = eval_module("drums = bd sn |> fast(2)", ReplMode::Loose).unwrap();

    assert_eq!(
        sample_names(direct.get("drums").unwrap()),
        sample_names(piped.get("drums").unwrap())
    );
}

#[test]
fn gain_updates_sample_event_amplitude() {
    let module = eval_module("drums = bd |> gain(0.8)", ReplMode::Loose).unwrap();
    let event = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit();
    assert_eq!(event.len(), 1);
    assert!((event[0].value.gain() - 0.8).abs() < f64::EPSILON);
}

#[test]
fn loose_mode_still_rejects_unresolved_identifiers_until_placeholders_exist() {
    assert_eval_error_contains(
        "drums = mystery",
        ReplMode::Loose,
        &[
            "unresolved identifier `mystery`",
            "placeholder playback is not implemented",
        ],
    );
}

#[test]
fn strict_mode_rejects_unresolved_identifiers() {
    assert_eval_error_contains(
        "drums = mystery",
        ReplMode::Strict,
        &["unresolved identifier `mystery`"],
    );
}

#[test]
fn builtin_type_errors_report_which_argument_shape_is_required() {
    assert_eval_error_contains(
        "drums = fast(bd, bd sn)",
        ReplMode::Loose,
        &["`fast` requires a constant number argument"],
    );
}

#[test]
fn transform_calls_inside_sequences_report_specific_guidance() {
    assert_eval_error_contains(
        "drums = bd fast(2)",
        ReplMode::Loose,
        &[
            "function call `fast` cannot appear inside a pattern sequence",
            "pipe",
        ],
    );
}
