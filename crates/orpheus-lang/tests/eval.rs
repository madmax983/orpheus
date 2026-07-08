//! Core integration tests for the language evaluator, verifying that expressions compile down to the correct temporal patterns and audio graphs.
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use orpheus_lang::{FunctionValue, ReplMode, Value, eval_module, export_sample_pattern_to_json};
use orpheus_pattern::{Rational, TimeSpan};
use serde_json::Value as JsonValue;

static EXPORT_COUNTER: AtomicU64 = AtomicU64::new(0);

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

fn exported_sample_events(value: &Value, cycle_count: u64) -> Vec<JsonValue> {
    let unique = EXPORT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "orpheus-lang-eval-export-{}-{unique}.json",
        std::process::id()
    ));
    export_sample_pattern_to_json(value.as_sample_pattern().unwrap(), &path, cycle_count).unwrap();
    let payload = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_file(&path);
    serde_json::from_str::<JsonValue>(&payload).unwrap()["events"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn evaluating_sequence_produces_sample_pattern() {
    let module = eval_module("drums = bd sn cp sn", ReplMode::Loose).unwrap();
    match module.get("drums").unwrap() {
        Value::SamplePattern(pattern) => {
            let events = pattern.query_unit().unwrap();
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
    assert_eq!(pattern.query_unit().unwrap().len(), 2);
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
fn every_applies_its_transform_on_cycle_zero() {
    let module = eval_module("drums = bd sn |> every(2, fast(2))", ReplMode::Loose).unwrap();
    assert_eq!(
        sample_names(module.get("drums").unwrap()),
        ["bd", "sn", "bd", "sn"]
    );
}

#[test]
fn sometimes_applies_its_transform_on_cycle_zero() {
    let module = eval_module("drums = bd sn |> sometimes(fast(2))", ReplMode::Loose).unwrap();
    assert_eq!(
        sample_names(module.get("drums").unwrap()),
        ["bd", "sn", "bd", "sn"]
    );
}

#[test]
fn when_applies_its_transform_only_on_matching_cycle_offsets() {
    let module = eval_module("drums = bd sn |> when(3, 1, rev)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 3);

    assert_eq!(
        events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["bd", "sn", "sn", "bd", "bd", "sn"]
    );
    assert_eq!(events[0]["start_num"].as_i64().unwrap(), 0);
    assert_eq!(events[0]["start_den"].as_i64().unwrap(), 1);
    assert_eq!(events[1]["start_num"].as_i64().unwrap(), 1);
    assert_eq!(events[1]["start_den"].as_i64().unwrap(), 2);
    assert_eq!(events[2]["start_num"].as_i64().unwrap(), 1);
    assert_eq!(events[2]["start_den"].as_i64().unwrap(), 1);
    assert_eq!(events[3]["start_num"].as_i64().unwrap(), 3);
    assert_eq!(events[3]["start_den"].as_i64().unwrap(), 2);
    assert_eq!(events[4]["start_num"].as_i64().unwrap(), 2);
    assert_eq!(events[4]["start_den"].as_i64().unwrap(), 1);
    assert_eq!(events[5]["start_num"].as_i64().unwrap(), 5);
    assert_eq!(events[5]["start_den"].as_i64().unwrap(), 2);
}

#[test]
fn when_applies_its_transform_on_cycles_one_and_four_in_a_five_cycle_query() {
    let module = eval_module("drums = bd sn |> when(3, 1, rev)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 5);

    assert_eq!(
        events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["bd", "sn", "sn", "bd", "bd", "sn", "bd", "sn", "sn", "bd"]
    );
    assert_eq!(events[0]["start_num"].as_i64().unwrap(), 0);
    assert_eq!(
        events
            .iter()
            .map(|event| {
                (
                    event["start_num"].as_i64().unwrap(),
                    event["start_den"].as_i64().unwrap(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, 1),
            (1, 2),
            (1, 1),
            (3, 2),
            (2, 1),
            (5, 2),
            (3, 1),
            (7, 2),
            (4, 1),
            (9, 2),
        ]
    );
}

#[test]
fn every_transforms_the_selected_cycle_in_isolation() {
    let module = eval_module(
        "drums = every(2, fast(2), every(3, rev, bd sn cp))",
        ReplMode::Loose,
    )
    .unwrap();
    assert_eq!(
        sample_names(module.get("drums").unwrap()),
        ["cp", "sn", "bd", "cp", "sn", "bd"]
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
fn shift_rotates_sample_events_forward_within_the_cycle() {
    let module = eval_module("drums = bd sn |> shift(0.25)", ReplMode::Loose).unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[0].value.sample(), "sn");
    assert_eq!(events[1].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[1].value.sample(), "bd");
    assert_eq!(events[2].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
    assert_eq!(events[2].value.sample(), "sn");
}

#[test]
fn shift_rotates_number_patterns_forward_within_the_cycle() {
    let module = eval_module("swing = shift(0.5, 1 2)", ReplMode::Loose).unwrap();
    let events = module
        .get("swing")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value - 2.0).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value - 1.0).abs() < f64::EPSILON);
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
fn direct_call_matches_pipe_application_for_every() {
    let direct = eval_module("drums = every(2, fast(2), bd sn)", ReplMode::Loose).unwrap();
    let piped = eval_module("drums = bd sn |> every(2, fast(2))", ReplMode::Loose).unwrap();

    assert_eq!(
        sample_names(direct.get("drums").unwrap()),
        sample_names(piped.get("drums").unwrap())
    );
}

#[test]
fn when_pipe_matches_direct_call() {
    let direct = eval_module("drums = when(3, 1, rev, bd sn)", ReplMode::Loose).unwrap();
    let piped = eval_module("drums = bd sn |> when(3, 1, rev)", ReplMode::Loose).unwrap();

    assert_eq!(
        exported_sample_events(direct.get("drums").unwrap(), 3),
        exported_sample_events(piped.get("drums").unwrap(), 3),
    );
}

#[test]
fn direct_call_matches_pipe_application_for_shift() {
    let direct = eval_module("drums = shift(0.25, bd sn)", ReplMode::Loose).unwrap();
    let piped = eval_module("drums = bd sn |> shift(0.25)", ReplMode::Loose).unwrap();

    let direct_events = direct
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let piped_events = piped
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(direct_events, piped_events);
}

#[test]
fn parameterized_binding_can_be_applied_curried() {
    let module = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         groove = swing(0.25)(bd sn)",
        ReplMode::Loose,
    )
    .unwrap();

    let events = module
        .get("groove")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events[0].value.sample(), "sn");
    assert_eq!(events[1].value.sample(), "bd");
    assert_eq!(events[2].value.sample(), "sn");
}

#[test]
fn parameterized_binding_can_be_used_from_pipe() {
    let direct = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         groove = swing(0.25)(bd sn)",
        ReplMode::Loose,
    )
    .unwrap();
    let piped = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         groove = bd sn |> swing(0.25)",
        ReplMode::Loose,
    )
    .unwrap();

    let direct_events = direct
        .get("groove")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let piped_events = piped
        .get("groove")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(direct_events, piped_events);
}

#[test]
fn pedal_graph_binding_evaluates_to_pedal_value() {
    let module = eval_module(
        "fx = graph { wet = input |> clip(model=silicon_hard); wet |> output }",
        ReplMode::Strict,
    )
    .unwrap();

    let pedal = module
        .get("fx")
        .unwrap()
        .as_pedal()
        .expect("expected pedal value");

    assert!(pedal.format_source().contains("clip(model=silicon_hard)"));
}

#[test]
fn pedal_graph_rejects_unbound_local_signal() {
    assert_eval_error_contains(
        "fx = graph { wet = dry |> output; wet |> output }",
        ReplMode::Strict,
        &["dry", "unbound local signal"],
    );
}

#[test]
fn pedal_graph_rejects_unbound_named_parameter_identifier() {
    assert_eval_error_contains(
        "fx = graph { wet = input |> clip(model=ghost); wet |> output }",
        ReplMode::Strict,
        &["ghost", "unbound local signal"],
    );
}

#[test]
fn pedal_graph_rejects_implicit_cycle() {
    assert_eval_error_contains(
        "fx = graph { wet = wet |> gain(0.5); wet |> output }",
        ReplMode::Strict,
        &["wet", "implicit cycle", "feedback"],
    );
}

#[test]
fn pedal_graph_accepts_explicit_feedback_node() {
    let module = eval_module(
        "fx = graph { wet = feedback(wet |> gain(0.5)); wet |> output }",
        ReplMode::Strict,
    )
    .unwrap();

    assert!(matches!(module.get("fx"), Some(Value::Pedal(_))));
}

#[test]
fn pedal_graph_rejects_output_misuse() {
    assert_eval_error_contains(
        "fx = graph { wet = output(input); wet |> output }",
        ReplMode::Strict,
        &["output", "final pipe target"],
    );
}

#[test]
fn through_direct_call_wraps_sample_pattern() {
    let module = eval_module(
        "drivebox = graph { wet = input |> clip(model=silicon_hard); wet |> output }\n\
         lead = through(drivebox, saw)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(sample_names(module.get("lead").unwrap()), ["saw"]);
}

#[test]
fn through_pipe_form_matches_direct_call() {
    let direct = eval_module(
        "drivebox = graph { wet = input |> clip(model=silicon_hard); wet |> output }\n\
         lead = through(drivebox, saw)",
        ReplMode::Strict,
    )
    .unwrap();
    let piped = eval_module(
        "drivebox = graph { wet = input |> clip(model=silicon_hard); wet |> output }\n\
         lead = saw |> through(drivebox)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(
        direct
            .get("lead")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
        piped
            .get("lead")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
    );
}

#[test]
fn through_rejects_non_sample_targets() {
    assert_eval_error_contains(
        "drivebox = graph { wet = input |> clip(model=silicon_hard); wet |> output }\n\
         bad = through(drivebox, 1.0)",
        ReplMode::Strict,
        &["through", "sample pattern"],
    );
}

#[test]
fn through_preserves_sample_pattern_type_and_attaches_pedal() {
    let module = eval_module(
        "drivebox = graph { wet = input |> clip(model=silicon_hard); wet |> output }\n\
         lead = through(drivebox, bd sn)",
        ReplMode::Strict,
    )
    .unwrap();

    let pattern = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .expect("through should keep a sample pattern");
    let events = pattern.query_unit().unwrap();

    assert_eq!(
        events
            .iter()
            .map(|event| event.value.sample())
            .collect::<Vec<_>>(),
        vec!["bd", "sn"]
    );

    let first_program = events[0]
        .value
        .pedal_program()
        .expect("first event should carry a pedal program");
    let second_program = events[1]
        .value
        .pedal_program()
        .expect("second event should carry a pedal program");

    assert!(std::sync::Arc::ptr_eq(first_program, second_program));
    assert!(
        first_program
            .explain()
            .contains("clip(input, model=silicon_hard)")
    );
}

#[test]
fn self_recursive_parameterized_binding_is_rejected() {
    assert_eval_error_contains(
        "loop pat = loop(pat)",
        ReplMode::Strict,
        &["self-reference", "loop"],
    );
}

#[test]
fn within_reverses_only_the_selected_window() {
    let module = eval_module(
        "drums = bd sn cp hh |> within(0, 0.5, rev)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(
        events
            .iter()
            .map(|event| event.value.sample())
            .collect::<Vec<_>>(),
        vec!["sn", "bd", "cp", "hh"]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.end(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
}

#[test]
fn within_pipe_matches_direct_call() {
    let direct = eval_module("drums = within(0, 0.5, rev, bd sn cp hh)", ReplMode::Loose).unwrap();
    let piped = eval_module(
        "drums = bd sn cp hh |> within(0, 0.5, rev)",
        ReplMode::Loose,
    )
    .unwrap();

    assert_eq!(
        direct
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
        piped
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
    );
}

#[test]
fn within_full_cycle_window_matches_direct_transform() {
    let direct = eval_module("drums = rev(bd sn cp hh)", ReplMode::Loose).unwrap();
    let within = eval_module("drums = bd sn cp hh |> within(0, 1, rev)", ReplMode::Loose).unwrap();

    assert_eq!(
        direct
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
        within
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
    );
}

#[test]
fn within_accepts_parameterized_unary_transforms() {
    let module = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         drums = bd sn cp hh |> within(0, 0.5, swing(0.25))",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(
        events
            .iter()
            .map(|event| event.value.sample())
            .collect::<Vec<_>>(),
        vec!["sn", "bd", "sn", "cp", "hh"]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 8).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 8).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(3, 8).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(3, 8).unwrap());
    assert_eq!(events[2].part.end(), &Rational::new(1, 2).unwrap());
}

#[test]
fn when_accepts_parameterized_unary_transforms() {
    let module = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         drums = bd sn |> when(2, 1, swing(0.25))",
        ReplMode::Loose,
    )
    .unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 2);

    assert_eq!(
        events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["bd", "sn", "sn", "bd", "sn"]
    );
    assert_eq!(events[2]["start_num"].as_i64().unwrap(), 1);
    assert_eq!(events[2]["start_den"].as_i64().unwrap(), 1);
    assert_eq!(events[2]["end_num"].as_i64().unwrap(), 5);
    assert_eq!(events[2]["end_den"].as_i64().unwrap(), 4);
    assert_eq!(events[3]["start_num"].as_i64().unwrap(), 5);
    assert_eq!(events[3]["start_den"].as_i64().unwrap(), 4);
    assert_eq!(events[3]["end_num"].as_i64().unwrap(), 7);
    assert_eq!(events[3]["end_den"].as_i64().unwrap(), 4);
    assert_eq!(events[4]["start_num"].as_i64().unwrap(), 7);
    assert_eq!(events[4]["start_den"].as_i64().unwrap(), 4);
    assert_eq!(events[4]["end_num"].as_i64().unwrap(), 2);
    assert_eq!(events[4]["end_den"].as_i64().unwrap(), 1);
}

#[test]
fn within_rejects_invalid_windows() {
    assert_eval_error_contains(
        "drums = bd sn |> within(0.75, 0.25, rev)",
        ReplMode::Strict,
        &["`within`", "start < end"],
    );
    assert_eval_error_contains(
        "drums = bd sn |> within(-0.25, 0.5, rev)",
        ReplMode::Strict,
        &["`within`", "[0, 1]"],
    );
}

#[test]
fn when_rejects_invalid_cycle_offsets() {
    assert_eval_error_contains(
        "drums = bd sn |> when(3, 3, rev)",
        ReplMode::Strict,
        &["`when`", "offset", "less than the period"],
    );
    assert_eval_error_contains(
        "drums = bd sn |> when(3, -1, rev)",
        ReplMode::Strict,
        &["`when`", "offset", "whole number"],
    );
}

#[test]
fn mask_keeps_only_fragments_overlapping_the_gate() {
    let module = eval_module("drums = mask(bd ~ cp ~, bd sn)", ReplMode::Loose).unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].value.sample(), "bd");
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].value.sample(), "sn");
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(3, 4).unwrap());
}

#[test]
fn mask_pipe_matches_direct_call() {
    let direct = eval_module("drums = mask(bd ~ cp ~, bd sn)", ReplMode::Loose).unwrap();
    let piped = eval_module("drums = bd sn |> mask(bd ~ cp ~)", ReplMode::Loose).unwrap();

    assert_eq!(
        direct
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
        piped
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
    );
}

#[test]
fn mask_accepts_number_pattern_gates() {
    let sample_gate = eval_module("drums = mask(bd ~ cp ~, bd sn)", ReplMode::Loose).unwrap();
    let number_gate = eval_module("drums = mask(1 ~ 1 ~, bd sn)", ReplMode::Loose).unwrap();

    assert_eq!(
        sample_gate
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
        number_gate
            .get("drums")
            .unwrap()
            .as_sample_pattern()
            .unwrap()
            .query_unit()
            .unwrap(),
    );
}

#[test]
fn mask_merges_adjacent_gate_events_into_one_open_region() {
    let module = eval_module("drums = mask(bd sn, slow(2, bd))", ReplMode::Loose).unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "bd");
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::one());
}

#[test]
fn mask_drops_source_events_with_no_gate_overlap() {
    let module = eval_module("drums = mask(~ cp ~ ~, bd sn)", ReplMode::Loose).unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "bd");
    assert_eq!(events[0].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
}

#[test]
fn mask_accepts_parameterized_bindings() {
    let module = eval_module(
        "keep gate pat = pat |> mask(gate)\n\
         drums = keep(bd ~ cp ~)(bd sn)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].value.sample(), "bd");
    assert_eq!(events[1].value.sample(), "sn");
}

#[test]
fn mask_rejects_non_pattern_gates() {
    assert_eval_error_contains(
        "drums = mask(rev, bd sn)",
        ReplMode::Strict,
        &["`mask`", "pattern gate"],
    );
}

#[test]
fn euclid_generates_three_open_steps_in_eight() {
    let module = eval_module("clave = euclid(3, 8)", ReplMode::Loose).unwrap();
    let events = module
        .get("clave")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 8).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(3, 8).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[2].part.end(), &Rational::new(7, 8).unwrap());
    assert!(
        events
            .iter()
            .all(|event| (event.value - 1.0).abs() < f64::EPSILON)
    );
}

#[test]
fn euclid_zero_pulses_generates_no_events() {
    let module = eval_module("clave = euclid(0, 8)", ReplMode::Loose).unwrap();
    let events = module
        .get("clave")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert!(events.is_empty());
}

#[test]
fn euclid_full_pulses_generates_all_steps() {
    let module = eval_module("clave = euclid(8, 8)", ReplMode::Loose).unwrap();
    let events = module
        .get("clave")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 8);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 8).unwrap());
    assert_eq!(events[7].part.start(), &Rational::new(7, 8).unwrap());
    assert_eq!(events[7].part.end(), &Rational::one());
}

#[test]
fn euclid_generates_five_open_steps_in_eight() {
    let module = eval_module("clave = euclid(5, 8)", ReplMode::Loose).unwrap();
    let events = module
        .get("clave")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 5);
    assert_eq!(
        events
            .iter()
            .map(|event| (*event.part.start(), *event.part.end()))
            .collect::<Vec<_>>(),
        vec![
            (Rational::zero(), Rational::new(1, 8).unwrap()),
            (Rational::new(1, 4).unwrap(), Rational::new(3, 8).unwrap()),
            (Rational::new(3, 8).unwrap(), Rational::new(1, 2).unwrap()),
            (Rational::new(5, 8).unwrap(), Rational::new(3, 4).unwrap()),
            (Rational::new(3, 4).unwrap(), Rational::new(7, 8).unwrap()),
        ]
    );
}

#[test]
fn euclid_masks_expected_slices() {
    let module = eval_module(
        "drums = mask(euclid(3, 8), bd sn cp hh bd sn cp hh)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].value.sample(), "bd");
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 8).unwrap());
    assert_eq!(events[1].value.sample(), "hh");
    assert_eq!(events[1].part.start(), &Rational::new(3, 8).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].value.sample(), "cp");
    assert_eq!(events[2].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[2].part.end(), &Rational::new(7, 8).unwrap());
}

#[test]
fn euclid_rejects_invalid_pulses_and_steps() {
    assert_eval_error_contains(
        "clave = euclid(9, 8)",
        ReplMode::Strict,
        &["`euclid`", "pulses", "less than or equal to steps"],
    );
    assert_eval_error_contains(
        "clave = euclid(3, 0)",
        ReplMode::Strict,
        &["`euclid`", "steps", "positive whole number"],
    );
    assert_eval_error_contains(
        "clave = euclid(2.5, 8)",
        ReplMode::Strict,
        &["`euclid`", "pulses", "whole number"],
    );
}

#[test]
fn pitch_class_set_evaluates_to_a_first_class_value() {
    let module = eval_module("hirajoshi = pitch_class_set(0 2 3 7 8)", ReplMode::Loose).unwrap();

    assert_eq!(
        module.get("hirajoshi").unwrap().kind_name(),
        "pitch class set"
    );
}

#[test]
fn degrees_map_aeolian_steps_with_octave_carry() {
    let module = eval_module("line = degrees(aeolian, 0 2 4 7 8)", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 5);
    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![0.0, 3.0, 7.0, 12.0, 14.0]
    );
}

#[test]
fn degrees_map_negative_aeolian_steps_downward() {
    let module = eval_module("line = degrees(aeolian, -2 -1 0 1)", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![-4.0, -2.0, 0.0, 2.0]
    );
}

#[test]
fn degrees_map_user_defined_pitch_class_sets() {
    let module = eval_module(
        "hirajoshi = pitch_class_set(0 2 3 7 8)\n\
         line = degrees(hirajoshi, 0 1 2 4 5)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![0.0, 2.0, 3.0, 8.0, 12.0]
    );
}

#[test]
fn degrees_transpose_shifts_number_patterns_by_semitones() {
    let module = eval_module(
        "line = degrees(aeolian, 0 2 4 7 8) |> transpose(45)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![45.0, 48.0, 52.0, 57.0, 59.0]
    );
}

#[test]
fn degrees_transpose_accepts_pattern_valued_offsets() {
    let module = eval_module(
        "line = degrees(aeolian, 0 2) |> transpose(12 -12)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![12.0, -9.0]
    );
}

#[test]
fn pitch_class_set_rejects_invalid_inputs() {
    assert_eval_error_contains(
        "bad = pitch_class_set(2 3 7 8)",
        ReplMode::Strict,
        &["pitch_class_set", "0"],
    );
    assert_eval_error_contains(
        "bad = pitch_class_set(0 3 3 7)",
        ReplMode::Strict,
        &["pitch_class_set", "strictly increasing"],
    );
    assert_eval_error_contains(
        "bad = pitch_class_set(0 7 3)",
        ReplMode::Strict,
        &["pitch_class_set", "strictly increasing"],
    );
    assert_eval_error_contains(
        "bad = pitch_class_set(0 2 12)",
        ReplMode::Strict,
        &["pitch_class_set", "[0, 11]"],
    );
    assert_eval_error_contains(
        "bad = pitch_class_set(0 2.5 7)",
        ReplMode::Strict,
        &["pitch_class_set", "whole number"],
    );
}

#[test]
fn degrees_reject_string_collection_arguments_and_fractional_steps() {
    assert_eval_error_contains(
        r#"line = degrees("aeolian", 0 2 4)"#,
        ReplMode::Strict,
        &["degrees", "pitch class set", "aeolian"],
    );
    assert_eval_error_contains(
        "line = degrees(aeolian, 0 1.5 2)",
        ReplMode::Strict,
        &["degrees", "whole-number degree"],
    );
}

#[test]
fn named_pitch_literals_map_to_absolute_semitones() {
    let module = eval_module("melody = c4 a4 bf3 fs4", ReplMode::Loose).unwrap();
    let events = module
        .get("melody")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 69.0, 58.0, 66.0]
    );
}

#[test]
fn named_pitch_literals_compose_with_transpose() {
    let module = eval_module("melody = transpose(12, c4 e4 g4)", ReplMode::Loose).unwrap();
    let events = module
        .get("melody")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![72.0, 76.0, 79.0]
    );
}

#[test]
fn named_pitch_literals_report_missing_octaves() {
    assert_eval_error_contains("bad = cf", ReplMode::Strict, &["pitch", "octave"]);
}

#[test]
fn chord_stacks_interval_sets_over_a_single_root() {
    let module = eval_module("pad = chord(c4, 0 4 7)", ReplMode::Loose).unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 3);
    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0, 67.0]
    );
    assert!(events.iter().all(|event| event.part == TimeSpan::unit()));
}

#[test]
fn chord_stacks_interval_sets_over_each_root_event() {
    let module = eval_module("line = chord(c4 e4, 0 7)", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 4);
    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 67.0, 64.0, 71.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.start(), &Rational::zero());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
    assert_eq!(events[3].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
}

#[test]
fn chord_composes_with_transpose() {
    let module = eval_module(
        "pad = chord(c4, 0 3 7 10) |> transpose(12)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![72.0, 75.0, 79.0, 82.0]
    );
}

#[test]
fn chord_accepts_degree_derived_roots() {
    let module = eval_module(
        "harm = chord(degrees(aeolian, 0 2) |> transpose(60), 0 3 7)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("harm")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 63.0, 67.0, 63.0, 66.0, 70.0]
    );
}

#[test]
fn chord_rejects_non_numeric_roots() {
    assert_eval_error_contains(
        "bad = chord(bd, 0 4 7)",
        ReplMode::Loose,
        &["chord", "number pattern"],
    );
}

#[test]
fn chord_rejects_non_numeric_intervals() {
    assert_eval_error_contains(
        "bad = chord(c4, bd)",
        ReplMode::Loose,
        &["chord", "number pattern"],
    );
}

#[test]
fn invert_first_inversion_raises_the_lowest_note() {
    let module = eval_module("pad = invert(1, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![64.0, 67.0, 72.0]
    );
    assert!(events.iter().all(|event| event.part == TimeSpan::unit()));
}

#[test]
fn invert_second_inversion_repeats_the_process() {
    let module = eval_module("pad = invert(2, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![67.0, 72.0, 76.0]
    );
}

#[test]
fn invert_applies_per_exact_span_cluster() {
    let module = eval_module("line = invert(1, chord(c4 e4, 0 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![67.0, 72.0, 71.0, 76.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.start(), &Rational::zero());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
    assert_eq!(events[3].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
}

#[test]
fn invert_leaves_single_note_clusters_unchanged() {
    let module = eval_module("melody = invert(1, c4 e4)", ReplMode::Loose).unwrap();
    let events = module
        .get("melody")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0]
    );
}

#[test]
fn invert_accepts_degree_derived_harmony() {
    let module = eval_module(
        "harm = chord(degrees(aeolian, 0 2) |> transpose(60), 0 3 7) |> invert(1)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("harm")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![63.0, 67.0, 72.0, 66.0, 70.0, 75.0]
    );
}

#[test]
fn invert_rejects_negative_counts() {
    assert_eval_error_contains(
        "bad = invert(-1, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["invert", "non-negative"],
    );
}

#[test]
fn invert_rejects_fractional_counts() {
    assert_eval_error_contains(
        "bad = invert(1.5, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["invert", "whole number"],
    );
}

#[test]
fn invert_rejects_non_constant_counts() {
    assert_eval_error_contains(
        "bad = invert(1 2, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["invert", "constant"],
    );
}

#[test]
fn invert_rejects_non_numeric_patterns() {
    assert_eval_error_contains(
        "bad = invert(1, bd)",
        ReplMode::Loose,
        &["invert", "number pattern"],
    );
}

#[test]
fn drop_second_highest_note_by_one_octave() {
    let module = eval_module("pad = drop(2, chord(c4, 0 4 7 10))", ReplMode::Loose).unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![55.0, 60.0, 64.0, 70.0]
    );
}

#[test]
fn drop_third_highest_note_by_one_octave() {
    let module = eval_module("pad = drop(3, chord(c4, 0 4 7 10))", ReplMode::Loose).unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![52.0, 60.0, 67.0, 70.0]
    );
}

#[test]
fn drop_applies_per_exact_span_cluster() {
    let module = eval_module("line = drop(2, chord(c4 e4, 0 7 10))", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![55.0, 60.0, 70.0, 59.0, 64.0, 74.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::zero());
    assert_eq!(events[2].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
    assert_eq!(events[5].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[5].part.end(), &Rational::one());
}

#[test]
fn drop_leaves_small_clusters_unchanged() {
    let module = eval_module("dyad = drop(3, chord(c4, 0 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("dyad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 67.0]
    );
}

#[test]
fn drop_accepts_degree_derived_harmony() {
    let module = eval_module(
        "harm = chord(degrees(aeolian, 0 2) |> transpose(60), 0 3 7 10) |> drop(2)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("harm")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![55.0, 60.0, 63.0, 70.0, 58.0, 63.0, 66.0, 73.0]
    );
}

#[test]
fn drop_composes_with_invert() {
    let module = eval_module(
        "pad = chord(c4, 0 4 7 10) |> drop(2) |> invert(1)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0, 67.0, 70.0]
    );
}

#[test]
fn drop_rejects_zero_counts() {
    assert_eval_error_contains(
        "bad = drop(0, chord(c4, 0 4 7 10))",
        ReplMode::Loose,
        &["drop", "positive whole number"],
    );
}

#[test]
fn drop_rejects_negative_counts() {
    assert_eval_error_contains(
        "bad = drop(-1, chord(c4, 0 4 7 10))",
        ReplMode::Loose,
        &["drop", "positive whole number"],
    );
}

#[test]
fn drop_rejects_fractional_counts() {
    assert_eval_error_contains(
        "bad = drop(1.5, chord(c4, 0 4 7 10))",
        ReplMode::Loose,
        &["drop", "whole number"],
    );
}

#[test]
fn drop_rejects_non_constant_counts() {
    assert_eval_error_contains(
        "bad = drop(1 2, chord(c4, 0 4 7 10))",
        ReplMode::Loose,
        &["drop", "constant"],
    );
}

#[test]
fn drop_rejects_non_numeric_patterns() {
    assert_eval_error_contains(
        "bad = drop(2, bd)",
        ReplMode::Loose,
        &["drop", "number pattern"],
    );
}

#[test]
fn strum_partitions_triads_into_equal_thirds() {
    let module = eval_module("pad = strum(chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0, 67.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 3).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 3).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(2, 3).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(2, 3).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
}

#[test]
fn strum_applies_per_exact_span_cluster() {
    let module = eval_module("line = strum(chord(c4 e4, 0 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 67.0, 64.0, 71.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.end(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
}

#[test]
fn strum_leaves_single_note_clusters_unchanged() {
    let module = eval_module("melody = strum(c4 e4)", ReplMode::Loose).unwrap();
    let events = module
        .get("melody")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
}

#[test]
fn strum_composes_with_drop() {
    let module = eval_module(
        "pad = chord(c4, 0 4 7 10) |> drop(2) |> strum",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![55.0, 60.0, 64.0, 70.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
}

#[test]
fn strum_composes_with_invert() {
    let module = eval_module(
        "pad = chord(c4, 0 4 7) |> invert(1) |> strum",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![64.0, 67.0, 72.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 3).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(2, 3).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
}

#[test]
fn strum_pipe_matches_direct_call() {
    let direct = eval_module("pad = strum(chord(c4 e4, 0 7))", ReplMode::Loose).unwrap();
    let direct_events = direct
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    let piped = eval_module("pad = chord(c4 e4, 0 7) |> strum", ReplMode::Loose).unwrap();
    let piped_events = piped
        .get("pad")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(direct_events, piped_events);
}

#[test]
fn strum_rejects_non_numeric_patterns() {
    assert_eval_error_contains(
        "bad = strum(bd)",
        ReplMode::Loose,
        &["strum", "number pattern"],
    );
}

#[test]
fn arp_wraps_upward_across_equal_fifths() {
    let module = eval_module("lead = arp(5, up, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0, 67.0, 60.0, 64.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 5).unwrap());
    assert_eq!(events[4].part.start(), &Rational::new(4, 5).unwrap());
    assert_eq!(events[4].part.end(), &Rational::one());
}

#[test]
fn arp_wraps_downward_across_equal_fifths() {
    let module = eval_module("lead = arp(5, down, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![67.0, 64.0, 60.0, 67.0, 64.0]
    );
}

#[test]
fn arp_bounces_pingpong_across_equal_fifths() {
    let module = eval_module("lead = arp(5, pingpong, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0, 67.0, 64.0, 60.0]
    );
}

#[test]
fn arp_bounces_pingpong_across_seven_notes() {
    let module = eval_module(
        "lead = arp(7, updown, chord(c4, 0 4 7 11))",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 64.0, 67.0, 71.0, 67.0, 64.0, 60.0]
    );
}

#[test]
fn arp_applies_per_exact_span_cluster() {
    let module = eval_module("line = arp(4, up, chord(c4 e4, 0 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 67.0, 60.0, 67.0, 64.0, 71.0, 64.0, 71.0]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 8).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(3, 8).unwrap());
    assert_eq!(events[3].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[4].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[4].part.end(), &Rational::new(5, 8).unwrap());
    assert_eq!(events[7].part.start(), &Rational::new(7, 8).unwrap());
    assert_eq!(events[7].part.end(), &Rational::one());
}

#[test]
fn arp_repeats_single_note_clusters() {
    let module = eval_module("melody = arp(4, up, c4 e4)", ReplMode::Loose).unwrap();
    let events = module
        .get("melody")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![60.0, 60.0, 60.0, 60.0, 64.0, 64.0, 64.0, 64.0]
    );
}

#[test]
fn arp_pipe_matches_direct_call() {
    let direct = eval_module("lead = arp(5, up, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let direct_events = direct
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    let piped = eval_module("lead = chord(c4, 0 4 7) |> arp(5, up)", ReplMode::Loose).unwrap();
    let piped_events = piped
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(direct_events, piped_events);
}

#[test]
fn arp_composes_with_drop() {
    let module = eval_module(
        "lead = chord(c4, 0 4 7 10) |> drop(2) |> arp(6, up)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![55.0, 60.0, 64.0, 70.0, 55.0, 60.0]
    );
}

#[test]
fn arp_rejects_zero_steps() {
    assert_eval_error_contains(
        "bad = arp(0, up, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["arp", "positive"],
    );
}

#[test]
fn arp_rejects_negative_steps() {
    assert_eval_error_contains(
        "bad = arp(-1, up, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["arp", "positive"],
    );
}

#[test]
fn arp_rejects_fractional_steps() {
    assert_eval_error_contains(
        "bad = arp(1.5, up, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["arp", "whole number"],
    );
}

#[test]
fn arp_rejects_non_constant_steps() {
    assert_eval_error_contains(
        "bad = arp(1 2, up, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["arp", "constant"],
    );
}

#[test]
fn arp_rejects_non_direction_arguments() {
    assert_eval_error_contains(
        "bad = arp(5, c4, chord(c4, 0 4 7))",
        ReplMode::Loose,
        &["arp", "direction"],
    );
}

#[test]
fn arp_rejects_non_numeric_patterns() {
    assert_eval_error_contains(
        "bad = arp(5, up, bd)",
        ReplMode::Loose,
        &["arp", "number pattern"],
    );
}

#[test]
fn roll_retriggers_sample_hits_across_equal_quarters() {
    let module = eval_module("buzz = roll(4, sn)", ReplMode::Loose).unwrap();
    let events = module
        .get("buzz")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 4);
    assert!(events.iter().all(|event| event.value.sample() == "sn"));
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
}

#[test]
fn roll_retriggers_chord_clusters_across_equal_quarters() {
    let module = eval_module("stabs = roll(4, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
    let events = module
        .get("stabs")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 12);
    assert_eq!(
        events.iter().map(|event| event.value).collect::<Vec<_>>(),
        vec![
            60.0, 64.0, 67.0, 60.0, 64.0, 67.0, 60.0, 64.0, 67.0, 60.0, 64.0, 67.0
        ]
    );
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[2].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[3].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[11].part.end(), &Rational::one());
}

#[test]
fn roll_applies_per_exact_span_cluster() {
    let module = eval_module("line = roll(4, bd sn)", ReplMode::Loose).unwrap();
    let events = module
        .get("line")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 8);
    assert_eq!(
        events
            .iter()
            .map(|event| (
                event.value.sample().to_owned(),
                *event.part.start(),
                *event.part.end()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "bd".to_string(),
                Rational::zero(),
                Rational::new(1, 8).unwrap()
            ),
            (
                "bd".to_string(),
                Rational::new(1, 8).unwrap(),
                Rational::new(1, 4).unwrap()
            ),
            (
                "bd".to_string(),
                Rational::new(1, 4).unwrap(),
                Rational::new(3, 8).unwrap()
            ),
            (
                "bd".to_string(),
                Rational::new(3, 8).unwrap(),
                Rational::new(1, 2).unwrap()
            ),
            (
                "sn".to_string(),
                Rational::new(1, 2).unwrap(),
                Rational::new(5, 8).unwrap()
            ),
            (
                "sn".to_string(),
                Rational::new(5, 8).unwrap(),
                Rational::new(3, 4).unwrap()
            ),
            (
                "sn".to_string(),
                Rational::new(3, 4).unwrap(),
                Rational::new(7, 8).unwrap()
            ),
            (
                "sn".to_string(),
                Rational::new(7, 8).unwrap(),
                Rational::one()
            ),
        ]
    );
}

#[test]
fn roll_pipe_matches_direct_call() {
    let direct = eval_module("buzz = roll(4, sn)", ReplMode::Loose).unwrap();
    let direct_events = direct
        .get("buzz")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    let piped = eval_module("buzz = sn |> roll(4)", ReplMode::Loose).unwrap();
    let piped_events = piped
        .get("buzz")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(direct_events, piped_events);
}

#[test]
fn roll_with_one_step_is_identity() {
    let original = eval_module("buzz = bd sn", ReplMode::Loose).unwrap();
    let original_events = original
        .get("buzz")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    let rolled = eval_module("buzz = roll(1, bd sn)", ReplMode::Loose).unwrap();
    let rolled_events = rolled
        .get("buzz")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(rolled_events, original_events);
}

#[test]
fn roll_rejects_zero_steps() {
    assert_eval_error_contains("bad = roll(0, sn)", ReplMode::Strict, &["roll", "positive"]);
}

#[test]
fn roll_rejects_negative_steps() {
    assert_eval_error_contains(
        "bad = roll(-1, sn)",
        ReplMode::Strict,
        &["roll", "positive"],
    );
}

#[test]
fn roll_rejects_fractional_steps() {
    assert_eval_error_contains(
        "bad = roll(1.5, sn)",
        ReplMode::Strict,
        &["roll", "whole number"],
    );
}

#[test]
fn roll_rejects_non_constant_steps() {
    assert_eval_error_contains(
        "bad = roll(1 2, sn)",
        ReplMode::Strict,
        &["roll", "constant"],
    );
}

#[test]
fn roll_rejects_non_pattern_values() {
    assert_eval_error_contains(
        "bad = roll(4, aeolian)",
        ReplMode::Strict,
        &["roll", "pattern"],
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
        .query_unit()
        .unwrap();
    assert_eq!(event.len(), 1);
    assert!((event[0].value.gain() - 0.8).abs() < f64::EPSILON);
}

#[test]
fn meter_translates_beats_into_cycle_relative_time() {
    let module = eval_module(
        "bridge = meter(4, 4, stream(at(beat(2), bd)))",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("bridge")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].part.start().numerator(), 1);
    assert_eq!(events[0].part.start().denominator(), 2);
}

#[test]
fn meter_prefix_annotation_translates_beats_into_cycle_relative_time() {
    let module = eval_module(
        "bridge = meter(4, 4) stream(at(beat(2), bd))",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("bridge")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].part.start().numerator(), 1);
    assert_eq!(events[0].part.start().denominator(), 2);
}

#[test]
fn sample_builtin_lifts_string_literals_into_sample_patterns() {
    let module = eval_module(r#"lead = sample("vox_ah")"#, ReplMode::Loose).unwrap();

    assert_eq!(sample_names(module.get("lead").unwrap()), ["vox_ah"]);
}

#[test]
fn sample_calls_can_form_pattern_sequences() {
    let module = eval_module(
        r#"lead = sample("vox_ah") sample("vox_oh")"#,
        ReplMode::Loose,
    )
    .unwrap();

    assert_eq!(
        sample_names(module.get("lead").unwrap()),
        ["vox_ah", "vox_oh"]
    );
}

#[test]
fn rate_and_slice_builtins_update_sample_event_playback_params() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> slice(0.25, 1) |> rate(2) |> gain(0.5) |> pan(-1)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "vox_ah");
    assert!((events[0].value.gain() - 0.5).abs() < f64::EPSILON);
    assert!((events[0].value.rate() - 2.0).abs() < f64::EPSILON);
    assert!((events[0].value.slice_start() - 0.25).abs() < f64::EPSILON);
    assert!((events[0].value.slice_end() - 1.0).abs() < f64::EPSILON);
    assert!((events[0].value.pan() - -1.0).abs() < f64::EPSILON);
}

#[test]
fn filter_builtins_update_sample_event_filter_params() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> lpf(800) |> hpf(200)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "vox_ah");
    assert_eq!(events[0].value.lpf_cutoff_hz(), Some(800.0));
    assert_eq!(events[0].value.hpf_cutoff_hz(), Some(200.0));
}

#[test]
fn synth_atoms_and_controls_update_sample_event_params() {
    let module = eval_module(
        r"lead = pulse |> cutoff(1200) |> res(0.25) |> drive(1.2) |> pw(0.35)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "pulse");
    assert_eq!(events[0].value.lpf_cutoff_hz(), Some(1200.0));
    assert!((events[0].value.resonance() - 0.25).abs() < f64::EPSILON);
    assert!((events[0].value.drive() - 1.2).abs() < f64::EPSILON);
    assert!((events[0].value.pulse_width() - 0.35).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_synth_controls_split_sample_events() {
    let module = eval_module(
        r"lead = pulse |> res(0.2 0.6) |> drive(1.0 1.5) |> pw(0.25 0.75)",
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value.resonance() - 0.2).abs() < f64::EPSILON);
    assert!((events[0].value.drive() - 1.0).abs() < f64::EPSILON);
    assert!((events[0].value.pulse_width() - 0.25).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value.resonance() - 0.6).abs() < f64::EPSILON);
    assert!((events[1].value.drive() - 1.5).abs() < f64::EPSILON);
    assert!((events[1].value.pulse_width() - 0.75).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_filter_controls_split_sample_events() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> lpf(400 800) |> hpf(100 200)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[0].value.lpf_cutoff_hz(), Some(400.0));
    assert_eq!(events[0].value.hpf_cutoff_hz(), Some(100.0));
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert_eq!(events[1].value.lpf_cutoff_hz(), Some(800.0));
    assert_eq!(events[1].value.hpf_cutoff_hz(), Some(200.0));
}

#[test]
fn lpf_rejects_non_positive_cutoff_controls() {
    assert_eval_error_contains(
        r#"lead = sample("vox_ah") |> lpf(0)"#,
        ReplMode::Loose,
        &["`lpf` requires a positive finite numeric value"],
    );
}

#[test]
fn negative_rate_is_preserved_on_sample_events() {
    let module = eval_module(r#"lead = sample("vox_ah") |> rate(-1)"#, ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert!((events[0].value.rate() - -1.0).abs() < f64::EPSILON);
}

#[test]
fn pitch_builtin_maps_semitones_to_rate_multipliers() {
    let up = eval_module(r#"lead = sample("vox_ah") |> pitch(12)"#, ReplMode::Loose).unwrap();
    let up_events = up
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    assert_eq!(up_events.len(), 1);
    assert!((up_events[0].value.rate() - 2.0).abs() < f64::EPSILON);

    let down = eval_module(r#"lead = sample("vox_ah") |> pitch(-12)"#, ReplMode::Loose).unwrap();
    let down_events = down
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    assert_eq!(down_events.len(), 1);
    assert!((down_events[0].value.rate() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn pitch_accepts_pattern_valued_controls_and_composes_with_existing_rate() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> rate(0.5) |> pitch(12 -12)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value.rate() - 1.0).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value.rate() - 0.25).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_pitch_controls_repeat_under_fast() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> pitch(0 12) |> fast(2)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let rates = events
        .iter()
        .map(|event| event.value.rate())
        .collect::<Vec<_>>();

    assert_eq!(rates, vec![1.0, 2.0, 1.0, 2.0]);
}

#[test]
fn slice_accepts_pattern_valued_start_and_end_controls() {
    let module = eval_module(
        r#"lead = sample("amen") |> slice(0 0.25, 0.5 1)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value.slice_start() - 0.0).abs() < f64::EPSILON);
    assert!((events[0].value.slice_end() - 0.5).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value.slice_start() - 0.25).abs() < f64::EPSILON);
    assert!((events[1].value.slice_end() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_slice_controls_compose_with_pitch_and_fast() {
    let module = eval_module(
        r#"lead = sample("amen") |> slice(0 0.25, 0.5 1) |> pitch(0 12) |> fast(2)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .map(|event| event.value.slice_start())
            .collect::<Vec<_>>(),
        vec![0.0, 0.25, 0.0, 0.25]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.value.rate())
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 1.0, 2.0]
    );
}

#[test]
fn rate_accepts_pattern_valued_controls_and_splits_sample_events() {
    let module = eval_module(r#"lead = sample("vox_ah") |> rate(0.5 2)"#, ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value.rate() - 0.5).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value.rate() - 2.0).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_rate_controls_repeat_under_fast() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> rate(0.5 2) |> fast(2)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let rates = events
        .iter()
        .map(|event| event.value.rate())
        .collect::<Vec<_>>();

    assert_eq!(rates, vec![0.5, 2.0, 0.5, 2.0]);
}

#[test]
fn gain_accepts_pattern_valued_controls_and_splits_sample_events() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> gain(0.25 0.75)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value.gain() - 0.25).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value.gain() - 0.75).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_gain_controls_repeat_under_fast() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> gain(0.25 0.75) |> fast(2)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let gains = events
        .iter()
        .map(|event| event.value.gain())
        .collect::<Vec<_>>();

    assert_eq!(gains, vec![0.25, 0.75, 0.25, 0.75]);
}

#[test]
fn insert_effect_builtins_update_sample_event_params_and_export() {
    let module = eval_module(
        r#"lead = sample("vox_ah")
            |> delay(0.40)
            |> delay_time(0.125)
            |> delay_feedback(0.60)
            |> reverb(0.30)
            |> reverb_room(0.85)
            |> reverb_damp(0.25)
            |> chorus(0.20)
            |> chorus_depth(0.45)
            |> chorus_rate(0.35)
            |> compressor(0.70)
            |> compressor_threshold(0.30)
            |> compressor_ratio(4)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = exported_sample_events(module.get("lead").unwrap(), 1);

    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event["sample"].as_str().unwrap(), "vox_ah");
    assert!((event["delay_mix"].as_f64().unwrap() - 0.40).abs() < f64::EPSILON);
    assert!((event["delay_time"].as_f64().unwrap() - 0.125).abs() < f64::EPSILON);
    assert!((event["delay_feedback"].as_f64().unwrap() - 0.60).abs() < f64::EPSILON);
    assert!((event["reverb_mix"].as_f64().unwrap() - 0.30).abs() < f64::EPSILON);
    assert!((event["reverb_room"].as_f64().unwrap() - 0.85).abs() < f64::EPSILON);
    assert!((event["reverb_damp"].as_f64().unwrap() - 0.25).abs() < f64::EPSILON);
    assert!((event["chorus_mix"].as_f64().unwrap() - 0.20).abs() < f64::EPSILON);
    assert!((event["chorus_depth"].as_f64().unwrap() - 0.45).abs() < f64::EPSILON);
    assert!((event["chorus_rate"].as_f64().unwrap() - 0.35).abs() < f64::EPSILON);
    assert!((event["compressor_mix"].as_f64().unwrap() - 0.70).abs() < f64::EPSILON);
    assert!((event["compressor_threshold"].as_f64().unwrap() - 0.30).abs() < f64::EPSILON);
    assert!((event["compressor_ratio"].as_f64().unwrap() - 4.0).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_insert_effect_controls_split_sample_events() {
    let module = eval_module(
        r#"lead = sample("vox_ah")
            |> delay(0.20 0.80)
            |> reverb_room(0.30 0.90)
            |> compressor_ratio(2 6)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[0].value.delay_mix() - 0.20).abs() < f64::EPSILON);
    assert!((events[0].value.reverb_room() - 0.30).abs() < f64::EPSILON);
    assert!((events[0].value.compressor_ratio() - 2.0).abs() < f64::EPSILON);
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::one());
    assert!((events[1].value.delay_mix() - 0.80).abs() < f64::EPSILON);
    assert!((events[1].value.reverb_room() - 0.90).abs() < f64::EPSILON);
    assert!((events[1].value.compressor_ratio() - 6.0).abs() < f64::EPSILON);
}

#[test]
fn insert_effect_builtins_reject_invalid_values() {
    assert_eval_error_contains(
        r#"lead = sample("vox_ah") |> delay_feedback(1.5)"#,
        ReplMode::Loose,
        &["`delay_feedback`", "[0, 1]"],
    );
    assert_eval_error_contains(
        r#"lead = sample("vox_ah") |> delay_time(0)"#,
        ReplMode::Loose,
        &["`delay_time`", "positive finite"],
    );
    assert_eval_error_contains(
        r#"lead = sample("vox_ah") |> compressor_ratio(0.5)"#,
        ReplMode::Loose,
        &["`compressor_ratio`", ">= 1"],
    );
}

#[test]
fn pan_accepts_pattern_valued_controls_and_composes_with_existing_pan() {
    let module = eval_module(
        r#"lead = sample("vox_ah") |> pan(-0.25) |> pan(0.5 -0.5)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 2);
    assert!((events[0].value.pan() - 0.25).abs() < f64::EPSILON);
    assert!((events[1].value.pan() - -0.75).abs() < f64::EPSILON);
}

#[test]
fn slice_idx_builtin_maps_zero_based_segments_into_normalized_slice_bounds() {
    let module = eval_module(
        r#"lead = sample("amen") |> slice_idx(3, 8)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "amen");
    assert!((events[0].value.slice_start() - 0.375).abs() < f64::EPSILON);
    assert!((events[0].value.slice_end() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn slice_idx_composes_with_existing_slice_bounds() {
    let module = eval_module(
        r#"lead = sample("amen") |> slice(0.25, 0.75) |> slice_idx(1, 2)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert!((events[0].value.slice_start() - 0.5).abs() < f64::EPSILON);
    assert!((events[0].value.slice_end() - 0.75).abs() < f64::EPSILON);
}

#[test]
fn slice_idx_accepts_pattern_valued_indices_and_splits_sample_events() {
    let module = eval_module(
        r#"lead = sample("amen") |> slice_idx(0 3 1 7, 8)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 4);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert!((events[0].value.slice_start() - 0.0).abs() < f64::EPSILON);
    assert!((events[0].value.slice_end() - 0.125).abs() < f64::EPSILON);

    assert_eq!(events[1].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert!((events[1].value.slice_start() - 0.375).abs() < f64::EPSILON);
    assert!((events[1].value.slice_end() - 0.5).abs() < f64::EPSILON);

    assert_eq!(events[2].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.end(), &Rational::new(3, 4).unwrap());
    assert!((events[2].value.slice_start() - 0.125).abs() < f64::EPSILON);
    assert!((events[2].value.slice_end() - 0.25).abs() < f64::EPSILON);

    assert_eq!(events[3].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[3].part.end(), &Rational::one());
    assert!((events[3].value.slice_start() - 0.875).abs() < f64::EPSILON);
    assert!((events[3].value.slice_end() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn pattern_valued_slice_idx_repeats_under_fast() {
    let module = eval_module(
        r#"lead = sample("amen") |> slice_idx(0 3, 8) |> fast(2)"#,
        ReplMode::Loose,
    )
    .unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let slice_starts = events
        .iter()
        .map(|event| event.value.slice_start())
        .collect::<Vec<_>>();

    assert_eq!(slice_starts, vec![0.0, 0.375, 0.0, 0.375]);
}

#[test]
fn onset_builtin_marks_sample_events_for_transient_lookup() {
    let module = eval_module(r#"lead = sample("amen") |> onset(2)"#, ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].value.sample(), "amen");
    assert_eq!(events[0].value.onset_index(), Some(2));
    assert!((events[0].value.slice_start() - 0.0).abs() < f64::EPSILON);
    assert!((events[0].value.slice_end() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn onset_accepts_pattern_valued_indices_and_splits_sample_events() {
    let module = eval_module(r#"lead = sample("amen") |> onset(0 2 1)"#, ReplMode::Loose).unwrap();
    let events = module
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 3).unwrap());
    assert_eq!(events[0].value.onset_index(), Some(0));
    assert_eq!(events[1].part.start(), &Rational::new(1, 3).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(2, 3).unwrap());
    assert_eq!(events[1].value.onset_index(), Some(2));
    assert_eq!(events[2].part.start(), &Rational::new(2, 3).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
    assert_eq!(events[2].value.onset_index(), Some(1));
}

#[test]
fn evaluating_multiple_top_level_bindings_reuses_prior_definitions() {
    let module = eval_module("verse = bd sn\nsong = fast(2, verse)", ReplMode::Loose).unwrap();

    assert_eq!(sample_names(module.get("verse").unwrap()), ["bd", "sn"]);
    assert_eq!(
        sample_names(module.get("song").unwrap()),
        ["bd", "sn", "bd", "sn"]
    );
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
    assert_eval_error_contains(
        "drums = every(2, fast, bd sn)",
        ReplMode::Loose,
        &["`every` requires a unary pattern transform as its second argument"],
    );
    assert_eval_error_contains(
        "drums = sometimes(fast, bd sn)",
        ReplMode::Loose,
        &["`sometimes` requires a unary pattern transform as its first argument"],
    );
    assert_eval_error_contains(
        "drums = shift(0 0.25, bd sn)",
        ReplMode::Loose,
        &["`shift` requires a constant number argument"],
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

#[test]
fn slice_idx_rejects_non_integer_arguments() {
    assert_eval_error_contains(
        r#"lead = sample("amen") |> slice_idx(1.5, 8)"#,
        ReplMode::Loose,
        &["`slice_idx index` requires a whole number"],
    );
    assert_eval_error_contains(
        r#"lead = sample("amen") |> slice_idx(1, 8.5)"#,
        ReplMode::Loose,
        &["`slice_idx segments` requires a positive whole number"],
    );
    assert_eval_error_contains(
        r#"lead = sample("amen") |> slice_idx(0 1.5, 8)"#,
        ReplMode::Loose,
        &["`slice_idx` requires whole-number control values"],
    );
}

#[test]
fn onset_rejects_non_integer_arguments() {
    assert_eval_error_contains(
        r#"lead = sample("amen") |> onset(1.5)"#,
        ReplMode::Loose,
        &["`onset index` requires a whole number"],
    );
    assert_eval_error_contains(
        r#"lead = sample("amen") |> onset(0 1.5)"#,
        ReplMode::Loose,
        &["`onset` requires whole-number control values"],
    );
}

#[test]
fn slice_rejects_pattern_controls_with_start_not_before_end() {
    assert_eval_error_contains(
        r#"lead = sample("amen") |> slice(0.5 0.75, 0.5 1)"#,
        ReplMode::Loose,
        &["`slice` requires control values with start < end"],
    );
}

#[test]
fn slice_idx_rejects_out_of_range_indices() {
    assert_eval_error_contains(
        r#"lead = sample("amen") |> slice_idx(8, 8)"#,
        ReplMode::Loose,
        &["`slice_idx` requires index < segments"],
    );
    assert_eval_error_contains(
        r#"lead = sample("amen") |> slice_idx(0, 0)"#,
        ReplMode::Loose,
        &["`slice_idx segments` requires a positive whole number"],
    );
}

#[test]
fn pan_rejects_out_of_range_values() {
    assert_eval_error_contains(
        r#"lead = sample("amen") |> pan(1.5)"#,
        ReplMode::Loose,
        &["`pan` requires a finite number within [-1, 1]"],
    );
}

#[test]
fn rand_builtin_generates_deterministic_random_numbers() {
    let module = eval_module("r = rand()", ReplMode::Loose).unwrap();
    let pattern = module.get("r").unwrap().as_number_pattern().unwrap();

    let span1 = TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap();
    let span2 = TimeSpan::new(Rational::new(1, 2).unwrap(), Rational::one()).unwrap();

    // Use try_query directly to request these explicit spans instead of querying the unit and filtering
    let events1 = pattern.try_query(&span1).unwrap();
    let events2 = pattern.try_query(&span2).unwrap();

    assert_eq!(events1.len(), 1);
    assert_eq!(events2.len(), 1);

    let v1 = events1[0].value;
    let v2 = events2[0].value;

    assert!((0.0..=1.0).contains(&v1));
    assert!((0.0..=1.0).contains(&v2));
    assert!((v1 - v2).abs() > f64::EPSILON);
}

#[test]
fn apply_user_function_curries_arguments_when_partially_applied() {
    let module = eval_module("f x y = x y\npartial = f(1)", ReplMode::Loose).unwrap();
    let partial = module.get("partial").unwrap();
    assert!(matches!(partial, Value::Function(FunctionValue::User(_))));
}

#[test]
fn apply_user_function_returns_error_when_overapplied() {
    assert_eval_error_contains(
        "f x = x\nerr = f(1, 2)",
        ReplMode::Loose,
        &["function expected 1 argument(s), got 2"],
    );
}

#[test]
fn apply_builtin_function_returns_curried_function_when_underapplied() {
    let module = eval_module("partial = fast(2)", ReplMode::Loose).unwrap();
    let partial = module.get("partial").unwrap();
    assert!(matches!(
        partial,
        Value::Function(FunctionValue::Builtin(_))
    ));
}

#[test]
fn apply_builtin_function_returns_error_when_overapplied() {
    assert_eval_error_contains(
        "err = fast(2, bd, 3)",
        ReplMode::Loose,
        &["`fast` expected 2 argument(s), got 3"],
    );
}

fn first_sample_rate(module: &std::collections::BTreeMap<String, Value>, binding: &str) -> f64 {
    let events = module
        .get(binding)
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    assert_eq!(events.len(), 1, "expected a single event");
    events[0].value.rate()
}

fn sample_rates(module: &std::collections::BTreeMap<String, Value>, binding: &str) -> Vec<f64> {
    module
        .get(binding)
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap()
        .into_iter()
        .map(|e| e.value.rate())
        .collect()
}

#[test]
fn tuning_builtin_binds_ratio_list() {
    let env = eval_module("t = tuning(1.0 1.125 1.25 1.5 2.0)", ReplMode::Loose).unwrap();
    let value = env.get("t").unwrap();
    let tuning = value.as_tuning().expect("binding should be a tuning value");
    let ratios: Vec<f64> = tuning.ratios().to_vec();
    assert_eq!(ratios.len(), 4);
    assert!((ratios[0] - 1.0).abs() < f64::EPSILON);
    assert!((ratios[1] - 1.125).abs() < f64::EPSILON);
    assert!((ratios[2] - 1.25).abs() < f64::EPSILON);
    assert!((ratios[3] - 1.5).abs() < f64::EPSILON);
    assert!((tuning.period() - 2.0).abs() < f64::EPSILON);
}

#[test]
fn tuning_builtin_rejects_non_octave_period() {
    assert_eval_error_contains("t = tuning(1.0 1.5 3.0)", ReplMode::Loose, &["period"]);
}

#[test]
fn tuning_builtin_rejects_non_monotone_ratios() {
    assert_eval_error_contains(
        "t = tuning(1.0 1.5 1.25 2.0)",
        ReplMode::Loose,
        &["monotone", "increasing", "strictly"],
    );
}

#[test]
fn tune_overrides_twelve_tet_for_pitch() {
    let env = eval_module(
        r#"
t = tuning(1.0 1.125 1.25 1.5 2.0)
lead = sample("vox_ah") |> pitch(2) |> tune(t)
"#,
        ReplMode::Loose,
    )
    .unwrap();
    let rate = first_sample_rate(&env, "lead");
    assert!(
        (rate - 1.25).abs() < f64::EPSILON,
        "expected 1.25 got {rate}"
    );
}

#[test]
fn tune_wraps_out_of_octave_semitones() {
    let env = eval_module(
        r#"
t = tuning(1.0 1.125 1.25 1.5 2.0)
up = sample("vox_ah") |> pitch(5) |> tune(t)
"#,
        ReplMode::Loose,
    )
    .unwrap();
    let rate = first_sample_rate(&env, "up");
    // scale length 4 (ratios are [1.0, 1.125, 1.25, 1.5]); step 5 wraps to idx 1, octave 1.
    // rate = 1.125 * 2.0^1 = 2.25
    assert!(
        (rate - 2.25).abs() < f64::EPSILON,
        "expected 2.25 got {rate}"
    );
}

#[test]
fn tune_handles_negative_semitones() {
    let env = eval_module(
        r#"
t = tuning(1.0 1.125 1.25 1.5 2.0)
down = sample("vox_ah") |> pitch(-1) |> tune(t)
"#,
        ReplMode::Loose,
    )
    .unwrap();
    let rate = first_sample_rate(&env, "down");
    // step = -1; ratios len N=4 -> idx = (-1).rem_euclid(4) = 3; octave = (-1).div_euclid(4) = -1
    // rate = ratios[3] * 2.0^-1 = 1.5 * 0.5 = 0.75
    assert!(
        (rate - 0.75).abs() < f64::EPSILON,
        "expected 0.75 got {rate}"
    );
}

#[test]
fn tune_supports_pattern_valued_pitch_control() {
    let env = eval_module(
        r#"
t = tuning(1.0 1.125 1.25 1.5 2.0)
line = sample("vox_ah") |> pitch(0 2) |> tune(t)
"#,
        ReplMode::Loose,
    )
    .unwrap();
    let rates = sample_rates(&env, "line");
    assert_eq!(rates.len(), 2);
    assert!((rates[0] - 1.0).abs() < f64::EPSILON);
    assert!((rates[1] - 1.25).abs() < f64::EPSILON);
}

#[test]
fn untuned_pitch_remains_twelve_tet() {
    // Regression: patterns without `tune` must keep exact 12-TET behavior.
    let env = eval_module(r#"lead = sample("vox_ah") |> pitch(12)"#, ReplMode::Loose).unwrap();
    let rate = first_sample_rate(&env, "lead");
    assert!((rate - 2.0).abs() < f64::EPSILON);
}

#[test]
fn load_scl_binds_tuning_from_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tuning/just_intonation.scl");
    let source = format!(
        "t = load_scl({:?})\nlead = sample(\"vox_ah\") |> pitch(7) |> tune(t)\n",
        path.to_str().unwrap()
    );
    let env = eval_module(&source, ReplMode::Loose).unwrap();
    let rate = first_sample_rate(&env, "lead");
    // 5-limit JI step 7 == 3/2 perfect fifth
    assert!((rate - 1.5).abs() < f64::EPSILON, "expected 1.5 got {rate}");
}

// --- cycle alternation: cat/slowcat/append, iter/iter_back, <a b c> grammar ---

fn exported_sample_names(value: &Value, cycle_count: u64) -> Vec<String> {
    exported_sample_events(value, cycle_count)
        .iter()
        .map(|event| event["sample"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn cat_plays_one_pattern_per_cycle_in_rotation() {
    let module = eval_module("drums = cat(bd, sn cp)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 3),
        ["bd", "sn", "cp", "bd"]
    );
}

#[test]
fn slowcat_is_an_alias_for_cat() {
    let module = eval_module("drums = slowcat(bd, sn cp)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 3),
        ["bd", "sn", "cp", "bd"]
    );
}

#[test]
fn cat_accepts_more_than_two_patterns() {
    let module = eval_module("drums = cat(bd, sn, cp)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 4),
        ["bd", "sn", "cp", "bd"]
    );
}

#[test]
fn append_concatenates_two_patterns_cyclewise() {
    let module = eval_module("drums = append(bd, sn)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 2),
        ["bd", "sn"]
    );
}

#[test]
fn cat_localizes_child_cycles_like_every() {
    // The child pattern only advances its own cycle counter on the cycles it
    // actually plays, so `every(2, rev, ...)` fires on child cycles 0 and 2,
    // which are global cycles 0 and 4.
    let module = eval_module("drums = cat(every(2, rev, bd sn), cp)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 6),
        ["sn", "bd", "cp", "bd", "sn", "cp", "sn", "bd", "cp"]
    );
}

#[test]
fn cat_requires_matching_pattern_kinds() {
    assert_eval_error_contains(
        "drums = cat(bd, 1 2)",
        ReplMode::Loose,
        &["`cat`", "same pattern kind"],
    );
}

#[test]
fn iter_rotates_the_pattern_by_one_step_each_cycle() {
    let module = eval_module("drums = iter(4, bd sn cp hh)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 5),
        [
            "bd", "sn", "cp", "hh", // cycle 0
            "sn", "cp", "hh", "bd", // cycle 1
            "cp", "hh", "bd", "sn", // cycle 2
            "hh", "bd", "sn", "cp", // cycle 3
            "bd", "sn", "cp", "hh", // cycle 4 wraps around
        ]
    );
}

#[test]
fn iter_back_rotates_in_the_opposite_direction() {
    let module = eval_module("drums = iter_back(4, bd sn cp hh)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 2),
        ["bd", "sn", "cp", "hh", "hh", "bd", "sn", "cp"]
    );
}

#[test]
fn iter_rejects_non_positive_step_counts() {
    assert_eval_error_contains(
        "drums = iter(0, bd sn)",
        ReplMode::Loose,
        &["`iter`", "positive integer"],
    );
}

#[test]
fn alternation_in_a_sequence_alternates_across_cycles() {
    let module = eval_module("drums = bd <sn cp>", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 3);
    assert_eq!(
        events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["bd", "sn", "bd", "cp", "bd", "sn"]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| {
                (
                    event["start_num"].as_i64().unwrap(),
                    event["start_den"].as_i64().unwrap(),
                )
            })
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 2), (1, 1), (3, 2), (2, 1), (5, 2)]
    );
}

#[test]
fn standalone_alternation_behaves_like_cat() {
    let module = eval_module("drums = <bd sn>", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 2),
        ["bd", "sn"]
    );
}

#[test]
fn alternation_inside_a_group_alternates_across_cycles() {
    let module = eval_module("drums = bd (sn <cp hh>)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 2),
        ["bd", "sn", "cp", "bd", "sn", "hh"]
    );
}

#[test]
fn alternation_supports_rest_elements() {
    let module = eval_module("drums = bd <sn ~>", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 2),
        ["bd", "sn", "bd"]
    );
}

#[test]
fn nested_alternations_advance_only_when_selected() {
    let module = eval_module("drums = <bd <sn cp>>", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 4),
        ["bd", "sn", "bd", "cp"]
    );
}

#[test]
fn alternation_supports_number_patterns() {
    let module = eval_module("melody = <1 2> 3", ReplMode::Loose).unwrap();
    let pattern = module.get("melody").unwrap().as_number_pattern().unwrap();
    let span = orpheus_lang::render_span(2).unwrap();
    let values = pattern
        .try_query(&span)
        .unwrap()
        .into_iter()
        .map(|event| event.value)
        .collect::<Vec<_>>();
    assert_eq!(values, [1.0, 3.0, 2.0, 3.0]);
}

// --- degrade family: deterministic per-event randomness ---

/// Materializes a number pattern's events as comparable tuples of
/// `(part start numerator, part start denominator, value bits)`.
fn number_event_keys(source: &str, name: &str, cycles: u64) -> Vec<(i128, i128, u64)> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get(name).unwrap().as_number_pattern().unwrap();
    let span = orpheus_lang::render_span(cycles).unwrap();
    pattern
        .try_query(&span)
        .unwrap()
        .into_iter()
        .map(|event| {
            (
                event.part.start().numerator(),
                event.part.start().denominator(),
                event.value.to_bits(),
            )
        })
        .collect()
}

fn cycle_time_span(cycle: i64) -> TimeSpan {
    TimeSpan::new(
        Rational::new(cycle, 1).unwrap(),
        Rational::new(cycle + 1, 1).unwrap(),
    )
    .unwrap()
}

#[test]
fn degrade_queries_are_deterministic_across_repeats() {
    let first = number_event_keys("m = 0 1 2 3 4 5 6 7 |> degrade", "m", 8);
    let second = number_event_keys("m = 0 1 2 3 4 5 6 7 |> degrade", "m", 8);
    assert_eq!(first, second);
}

#[test]
fn degrade_is_stable_regardless_of_query_window_chunking() {
    let module = eval_module("m = 0 1 2 3 4 5 6 7 |> degrade", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();

    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    let key = |events: &[orpheus_pattern::Event<f64>]| {
        events
            .iter()
            .map(|event| {
                (
                    event.part.start().numerator(),
                    event.part.start().denominator(),
                    event.value.to_bits(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(key(&whole), key(&chunked));
}

#[test]
fn degrade_drops_a_strict_subset_of_events() {
    let base = number_event_keys("m = 0 1 2 3 4 5 6 7", "m", 16);
    let degraded = number_event_keys("m = 0 1 2 3 4 5 6 7 |> degrade", "m", 16);

    assert!(!degraded.is_empty(), "degrade should keep some events");
    assert!(
        degraded.len() < base.len(),
        "degrade should drop some events"
    );
    for event in &degraded {
        assert!(
            base.contains(event),
            "degrade must not invent events: {event:?}"
        );
    }
}

#[test]
fn degrade_by_zero_keeps_every_event() {
    let base = number_event_keys("m = 0 1 2 3", "m", 4);
    let kept = number_event_keys("m = 0 1 2 3 |> degrade_by(0.0)", "m", 4);
    assert_eq!(base, kept);
}

#[test]
fn degrade_by_one_drops_every_event() {
    let kept = number_event_keys("m = 0 1 2 3 |> degrade_by(1.0)", "m", 4);
    assert!(kept.is_empty());
}

#[test]
fn degrade_by_keeps_roughly_the_complementary_fraction() {
    let base = number_event_keys("m = 0 1 2 3 4 5 6 7 |> fast(2)", "m", 64);
    let kept = number_event_keys(
        "m = 0 1 2 3 4 5 6 7 |> fast(2) |> degrade_by(0.25)",
        "m",
        64,
    );

    #[allow(clippy::cast_precision_loss)]
    let ratio = kept.len() as f64 / base.len() as f64;
    assert!(
        (0.65..=0.85).contains(&ratio),
        "expected ~75% of events kept, got {ratio}"
    );
}

#[test]
fn degrade_by_rejects_probabilities_outside_the_unit_interval() {
    assert_eval_error_contains(
        "m = 0 1 |> degrade_by(1.5)",
        ReplMode::Loose,
        &["`degrade_by` requires a probability within [0.0, 1.0]"],
    );
    assert_eval_error_contains(
        "m = 0 1 |> degrade_by(-0.25)",
        ReplMode::Loose,
        &["`degrade_by` requires a probability within [0.0, 1.0]"],
    );
}

#[test]
fn degrade_hashes_by_event_time_so_identical_layers_flip_together() {
    // Two identical stacked layers produce events at identical times, so the
    // per-event coin flip must keep or drop both copies together.
    let degraded = number_event_keys("m = stack(0 1 2 3, 0 1 2 3) |> degrade", "m", 16);
    let mut counts = std::collections::BTreeMap::new();
    for event in degraded {
        *counts.entry(event).or_insert(0_u32) += 1;
    }
    assert!(!counts.is_empty());
    for (event, count) in counts {
        assert_eq!(count, 2, "event {event:?} kept {count} times, expected 2");
    }
}

#[test]
fn degrade_composes_with_cat_and_fast_deterministically() {
    let module = eval_module(
        "m = cat(0 1 2 3, 4 5 6 7) |> fast(2) |> degrade",
        ReplMode::Loose,
    )
    .unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    assert!(!whole.is_empty());
    assert_eq!(whole.len(), chunked.len());
    for (a, b) in whole.iter().zip(chunked.iter()) {
        assert_eq!(a.part.start(), b.part.start());
        assert!((a.value - b.value).abs() < f64::EPSILON);
    }
}

#[test]
fn degrade_thins_sample_patterns() {
    let module = eval_module("drums = hh |> fast(8) |> degrade", ReplMode::Loose).unwrap();
    let names = exported_sample_names(module.get("drums").unwrap(), 8);
    assert!(!names.is_empty());
    assert!(names.len() < 64, "expected fewer than the 64 base events");
    assert!(names.iter().all(|name| name == "hh"));
}

#[test]
fn sometimes_by_partitions_events_into_exact_complements() {
    // `transpose(100)` marks transformed events while preserving timing, so
    // every base event must appear exactly once: either untouched or +100.
    let base = number_event_keys("m = 0 1 2 3", "m", 32);
    let mixed = number_event_keys("m = 0 1 2 3 |> sometimes_by(0.5, transpose(100))", "m", 32);

    assert_eq!(mixed.len(), base.len());
    let transformed_count = mixed
        .iter()
        .filter(|(_, _, bits)| f64::from_bits(*bits) >= 100.0)
        .count();
    assert!(transformed_count > 0, "some events should be transformed");
    assert!(
        transformed_count < mixed.len(),
        "some events should stay untouched"
    );

    let mixed_set: std::collections::BTreeSet<_> = mixed.iter().copied().collect();
    assert_eq!(mixed_set.len(), mixed.len(), "no duplicated events");
    for (num, den, bits) in base {
        let value = f64::from_bits(bits);
        let untouched = mixed_set.contains(&(num, den, bits));
        let transformed = mixed_set.contains(&(num, den, (value + 100.0).to_bits()));
        assert!(
            untouched ^ transformed,
            "event at {num}/{den} must appear exactly once, transformed or not"
        );
    }
}

#[test]
fn sometimes_by_transforms_roughly_the_requested_fraction() {
    let mixed = number_event_keys(
        "m = 0 1 2 3 |> fast(4) |> sometimes_by(0.25, transpose(100))",
        "m",
        32,
    );
    let transformed = mixed
        .iter()
        .filter(|(_, _, bits)| f64::from_bits(*bits) >= 100.0)
        .count();
    #[allow(clippy::cast_precision_loss)]
    let ratio = transformed as f64 / mixed.len() as f64;
    assert!(
        (0.15..=0.35).contains(&ratio),
        "expected ~25% transformed, got {ratio}"
    );
}

#[test]
fn sometimes_by_rejects_probabilities_outside_the_unit_interval() {
    assert_eval_error_contains(
        "m = 0 1 |> sometimes_by(2.0, transpose(100))",
        ReplMode::Loose,
        &["`sometimes_by` requires a probability within [0.0, 1.0]"],
    );
}

#[test]
fn sometimes_by_applies_transforms_to_sample_patterns() {
    let module = eval_module(
        "drums = hh hh hh hh |> sometimes_by(0.5, gain(2))",
        ReplMode::Loose,
    )
    .unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 8);
    assert_eq!(events.len(), 32, "every event appears exactly once");
    for event in &events {
        let gain = event["gain"].as_f64().unwrap();
        assert!(
            (gain - 1.0).abs() < f64::EPSILON || (gain - 2.0).abs() < f64::EPSILON,
            "gain should be 1 (untouched) or 2 (transformed), got {gain}"
        );
    }
}

#[test]
fn often_transforms_more_events_than_rarely() {
    let transformed_count = |source: &str| {
        number_event_keys(source, "m", 64)
            .iter()
            .filter(|(_, _, bits)| f64::from_bits(*bits) >= 100.0)
            .count()
    };
    let often = transformed_count("m = 0 1 2 3 |> often(transpose(100))");
    let rarely = transformed_count("m = 0 1 2 3 |> rarely(transpose(100))");
    let base_len = number_event_keys("m = 0 1 2 3", "m", 64).len();

    assert!(
        often > rarely,
        "often={often} should exceed rarely={rarely}"
    );
    assert_eq!(
        number_event_keys("m = 0 1 2 3 |> often(transpose(100))", "m", 64).len(),
        base_len
    );
    assert_eq!(
        number_event_keys("m = 0 1 2 3 |> rarely(transpose(100))", "m", 64).len(),
        base_len
    );
}

// --- mini-notation step operators: `*`, `/`, `!`, `?`, `{...}` polymeter ---

/// Materializes only the onset-bearing events of a number pattern as
/// comparable `(part start numerator, part start denominator, value bits)`
/// tuples. Slowed steps (`a/n`) emit onset-less tail fragments on the cycles
/// where the stretched event continues; those fragments never retrigger, so
/// onset comparison is the Tidal-accurate equivalence.
fn number_onset_keys(source: &str, name: &str, cycles: u64) -> Vec<(i128, i128, u64)> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get(name).unwrap().as_number_pattern().unwrap();
    let span = orpheus_lang::render_span(cycles).unwrap();
    pattern
        .try_query(&span)
        .unwrap()
        .into_iter()
        .filter(|event| {
            event
                .whole
                .as_ref()
                .is_none_or(|whole| whole.start() == event.part.start())
        })
        .map(|event| {
            (
                event.part.start().numerator(),
                event.part.start().denominator(),
                event.value.to_bits(),
            )
        })
        .collect()
}

/// Exports a sample pattern as `(start numerator, start denominator, sample)`
/// tuples sorted so that simultaneous events compare deterministically.
fn sample_event_keys(source: &str, name: &str, cycles: u64) -> Vec<(i64, i64, String)> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let mut keys = exported_sample_events(module.get(name).unwrap(), cycles)
        .iter()
        .map(|event| {
            (
                event["start_num"].as_i64().unwrap(),
                event["start_den"].as_i64().unwrap(),
                event["sample"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        (left.0 * right.1)
            .cmp(&(right.0 * left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    keys
}

#[test]
fn repetition_squeezes_the_element_into_its_slot() {
    assert_eq!(
        sample_event_keys("drums = bd*2 sn", "drums", 2),
        sample_event_keys("drums = (bd bd) sn", "drums", 2),
    );
}

#[test]
fn repetition_applies_to_groups() {
    assert_eq!(
        sample_event_keys("drums = (bd sn)*2 cp", "drums", 2),
        sample_event_keys("drums = ((bd sn) (bd sn)) cp", "drums", 2),
    );
}

#[test]
fn repetition_applies_to_alternations() {
    // `<bd sn cp>*2` steps through the alternation twice per cycle, matching
    // Tidal's `"<bd sn cp>*2"`.
    let module = eval_module("drums = <bd sn cp>*2", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_names(module.get("drums").unwrap(), 3),
        ["bd", "sn", "cp", "bd", "sn", "cp"]
    );
}

#[test]
fn repetition_by_one_is_identity() {
    assert_eq!(
        sample_event_keys("drums = bd*1 sn", "drums", 2),
        sample_event_keys("drums = bd sn", "drums", 2),
    );
}

#[test]
fn repetition_factor_must_stay_within_bounds() {
    assert_eval_error_contains("drums = bd*0 sn", ReplMode::Loose, &["`*`", "1", "1024"]);
    assert_eval_error_contains("drums = bd*1025 sn", ReplMode::Loose, &["`*`", "1", "1024"]);
    assert_eval_error_contains("drums = bd*1.5 sn", ReplMode::Loose, &["`*`", "integer"]);
}

#[test]
fn replication_expands_into_separate_steps() {
    assert_eq!(
        sample_event_keys("drums = bd!3 sn", "drums", 2),
        sample_event_keys("drums = bd bd bd sn", "drums", 2),
    );
}

#[test]
fn replication_count_must_be_positive() {
    assert_eval_error_contains("drums = bd!0 sn", ReplMode::Loose, &["`!`"]);
}

#[test]
fn slow_by_one_is_identity() {
    assert_eq!(
        number_onset_keys("m = 0/1 1", "m", 2),
        number_onset_keys("m = 0 1", "m", 2),
    );
}

#[test]
fn slowed_atom_plays_every_other_cycle_like_an_alternation_with_a_rest() {
    assert_eq!(
        number_onset_keys("m = 0/2", "m", 4),
        number_onset_keys("m = <0 ~>", "m", 4),
    );
    assert_eq!(
        number_onset_keys("m = 0/2 1", "m", 4),
        number_onset_keys("m = <0 ~> 1", "m", 4),
    );
}

#[test]
fn slowed_group_windows_one_half_per_cycle() {
    // `(0 1)/2` genuinely slows the inner pair: cycle 0 shows the first half
    // (the `0`), cycle 1 the second half (the `1`).
    assert_eq!(
        number_onset_keys("m = (0 1)/2", "m", 2),
        vec![(0, 1, 0.0_f64.to_bits()), (1, 1, 1.0_f64.to_bits())],
    );
}

#[test]
fn slow_factor_must_stay_within_bounds() {
    assert_eval_error_contains("m = 0/0 1", ReplMode::Loose, &["`/`", "1", "1024"]);
    assert_eval_error_contains("m = 0/1025 1", ReplMode::Loose, &["`/`", "1", "1024"]);
}

#[test]
fn degrade_operator_is_deterministic_across_repeats() {
    let first = number_event_keys("m = (0 1 2 3 4 5 6 7)?", "m", 8);
    let second = number_event_keys("m = (0 1 2 3 4 5 6 7)?", "m", 8);
    assert_eq!(first, second);
    assert!(!first.is_empty(), "`?` should keep some events");
    assert!(
        first.len() < 64,
        "`?` should drop some of the 64 base events"
    );
}

#[test]
fn degrade_operator_is_stable_regardless_of_query_window_chunking() {
    let module = eval_module("m = (0 1 2 3 4 5 6 7)?", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();

    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    assert_eq!(whole, chunked);
}

#[test]
fn degrade_operator_thins_only_its_own_step() {
    let events = sample_event_keys("drums = bd? sn", "drums", 32);
    let sn_count = events.iter().filter(|(_, _, name)| name == "sn").count();
    let bd_count = events.iter().filter(|(_, _, name)| name == "bd").count();
    assert_eq!(sn_count, 32, "`sn` has no `?` and must always play");
    assert!(bd_count > 0, "`bd?` should keep some events");
    assert!(bd_count < 32, "`bd?` should drop some events");
}

#[test]
fn degrade_operator_sites_flip_independently() {
    // Two `?` sites over identical inner patterns receive distinct site
    // salts, so stacked layers must not always flip together.
    let keys = number_event_keys("m = stack((0 1 2 3 4 5 6 7)?, (0 1 2 3 4 5 6 7)?)", "m", 8);
    let mut counts = std::collections::BTreeMap::new();
    for key in keys {
        *counts.entry(key).or_insert(0_u32) += 1;
    }
    assert!(
        counts.values().any(|count| *count == 1),
        "independent `?` sites should disagree on at least one onset"
    );
}

#[test]
fn degrade_operator_accepts_a_probability_suffix() {
    assert_eq!(
        number_event_keys("m = (0 1 2 3)?0.0", "m", 4).len(),
        16,
        "`?0.0` keeps every event"
    );
    assert!(
        number_event_keys("m = (0 1 2 3)?1.0", "m", 4).is_empty(),
        "`?1.0` drops every event"
    );

    let sparse = number_event_keys("m = (0 1 2 3 4 5 6 7)?0.9", "m", 16).len();
    let dense = number_event_keys("m = (0 1 2 3 4 5 6 7)?0.1", "m", 16).len();
    assert!(
        sparse < dense,
        "`?0.9` ({sparse}) should keep fewer events than `?0.1` ({dense})"
    );
}

#[test]
fn degrade_operator_rejects_probabilities_outside_the_unit_interval() {
    assert_eval_error_contains("m = (0 1)?1.5", ReplMode::Loose, &["`?`", "probability"]);
}

#[test]
fn polymeter_steps_each_subsequence_against_the_base_step_count() {
    // `{bd sn, hh cp sn}`: two steps per cycle from each subsequence; the
    // three-step subsequence wraps across cycles.
    assert_eq!(
        sample_event_keys("drums = {bd sn, hh cp sn}", "drums", 4),
        vec![
            (0, 1, "bd".to_owned()),
            (0, 1, "hh".to_owned()),
            (1, 2, "cp".to_owned()),
            (1, 2, "sn".to_owned()),
            (1, 1, "bd".to_owned()),
            (1, 1, "sn".to_owned()),
            (3, 2, "hh".to_owned()),
            (3, 2, "sn".to_owned()),
            (2, 1, "bd".to_owned()),
            (2, 1, "cp".to_owned()),
            (5, 2, "sn".to_owned()),
            (5, 2, "sn".to_owned()),
            (3, 1, "bd".to_owned()),
            (3, 1, "hh".to_owned()),
            (7, 2, "cp".to_owned()),
            (7, 2, "sn".to_owned()),
        ],
    );
}

#[test]
fn polymeter_supports_an_explicit_step_count() {
    // `{0 1 2}%4` plays four steps per cycle from an infinitely repeating
    // `0 1 2`, continuing where the previous cycle left off.
    assert_eq!(
        number_onset_keys("m = {0 1 2}%4", "m", 2),
        vec![
            (0, 1, 0.0_f64.to_bits()),
            (1, 4, 1.0_f64.to_bits()),
            (1, 2, 2.0_f64.to_bits()),
            (3, 4, 0.0_f64.to_bits()),
            (1, 1, 1.0_f64.to_bits()),
            (5, 4, 2.0_f64.to_bits()),
            (3, 2, 0.0_f64.to_bits()),
            (7, 4, 1.0_f64.to_bits()),
        ],
    );
}

#[test]
fn polymeter_with_a_single_subsequence_and_matching_steps_is_a_plain_sequence() {
    assert_eq!(
        sample_event_keys("drums = {bd sn}", "drums", 2),
        sample_event_keys("drums = bd sn", "drums", 2),
    );
}

#[test]
fn polymeter_is_stable_regardless_of_query_window_chunking() {
    let module = eval_module("m = {0 1, 2 3 4}", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(6).unwrap();
    let mut whole = pattern.try_query(&span).unwrap();

    let mut chunked = Vec::new();
    for cycle in 0..6 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    // Simultaneous events from the stacked subsequences carry no defined
    // relative order, so canonicalize ties by value before comparing.
    let canonicalize = |events: &mut Vec<orpheus_pattern::Event<f64>>| {
        events.sort_by(|left, right| {
            left.part
                .start()
                .cmp(right.part.start())
                .then_with(|| left.value.total_cmp(&right.value))
        });
    };
    canonicalize(&mut whole);
    canonicalize(&mut chunked);

    assert_eq!(whole, chunked);
}

#[test]
fn polymeter_step_count_must_stay_within_bounds() {
    assert_eval_error_contains("m = {0 1}%0", ReplMode::Loose, &["polymeter", "1", "1024"]);
}

#[test]
fn step_operators_compose_on_a_single_step() {
    // `bd!2?` replicates first, then degrades each copy independently.
    let events = sample_event_keys("drums = bd!2? sn", "drums", 32);
    let sn_count = events.iter().filter(|(_, _, name)| name == "sn").count();
    let bd_count = events.iter().filter(|(_, _, name)| name == "bd").count();
    assert_eq!(sn_count, 32);
    assert!(bd_count > 0, "`bd!2?` should keep some events");
    assert!(bd_count < 64, "`bd!2?` should drop some events");
}

#[test]
fn pedal_graph_arithmetic_still_evaluates_with_tight_multiplication() {
    // `dry*0.2` inside `graph { ... }` must stay pedal-DSL multiplication
    // even without spaces around `*`.
    let module = eval_module(
        "drivebox = graph { dry = input ; wet = input |> clip(model=silicon_hard) ; mix(dry*0.2 + wet*0.8, dry) |> output }",
        ReplMode::Loose,
    )
    .unwrap();
    assert!(matches!(module.get("drivebox").unwrap(), Value::Pedal(_)));
}

#[test]
fn almost_always_and_almost_never_bracket_the_probability_range() {
    let transformed_count = |source: &str| {
        number_event_keys(source, "m", 64)
            .iter()
            .filter(|(_, _, bits)| f64::from_bits(*bits) >= 100.0)
            .count()
    };
    let almost_always = transformed_count("m = 0 1 2 3 |> almost_always(transpose(100))");
    let almost_never = transformed_count("m = 0 1 2 3 |> almost_never(transpose(100))");
    let total = number_event_keys("m = 0 1 2 3", "m", 64).len();

    #[allow(clippy::cast_precision_loss)]
    let always_ratio = almost_always as f64 / total as f64;
    #[allow(clippy::cast_precision_loss)]
    let never_ratio = almost_never as f64 / total as f64;
    assert!(always_ratio > 0.8, "almost_always ratio {always_ratio}");
    assert!(never_ratio < 0.2, "almost_never ratio {never_ratio}");
}

// --- segment / range / choose / wchoose / irand: sampling continuous patterns ---

/// Queries a number pattern binding over the unit cycle.
fn unit_number_events(source: &str, name: &str) -> Vec<orpheus_pattern::Event<f64>> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get(name).unwrap().as_number_pattern().unwrap();
    pattern.try_query(&TimeSpan::unit()).unwrap()
}

#[test]
fn segment_samples_rand_into_discrete_slots() {
    let events = unit_number_events("m = rand() |> segment(4)", "m");

    assert_eq!(events.len(), 4);
    for (index, event) in events.iter().enumerate() {
        let index = i64::try_from(index).unwrap();
        let expected_start = Rational::new(index, 4).unwrap();
        let expected_end = Rational::new(index + 1, 4).unwrap();
        let whole = event.whole.expect("segment slots have whole spans");
        assert_eq!(*whole.start(), expected_start);
        assert_eq!(*whole.end(), expected_end);
        assert_eq!(event.part, whole, "unit-cycle query keeps whole == part");
        assert!((0.0..=1.0).contains(&event.value));
    }

    let distinct = events
        .iter()
        .map(|event| event.value.to_bits())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(distinct.len() > 1, "rand slots should not all be equal");
}

#[test]
fn segment_accepts_bare_rand_in_value_position() {
    let events = unit_number_events("m = segment(4, rand)", "m");
    assert_eq!(events.len(), 4);
}

#[test]
fn segment_queries_are_deterministic_across_repeats() {
    let first = number_event_keys("m = rand() |> segment(8)", "m", 8);
    let second = number_event_keys("m = rand() |> segment(8)", "m", 8);
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
}

#[test]
fn segment_is_stable_under_query_chunking() {
    let module = eval_module("m = rand() |> segment(4)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(2).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    assert_eq!(whole.len(), 8);

    // Chunk boundaries intentionally split slots (1/8 is inside slot [0, 1/4)).
    let boundaries = [
        Rational::new(0, 1).unwrap(),
        Rational::new(1, 8).unwrap(),
        Rational::new(5, 8).unwrap(),
        Rational::new(2, 1).unwrap(),
    ];
    let mut chunked = Vec::new();
    for window in boundaries.windows(2) {
        let chunk = TimeSpan::new(window[0], window[1]).unwrap();
        chunked.extend(pattern.try_query(&chunk).unwrap());
    }

    let whole_keys: std::collections::BTreeSet<(i128, i128, u64)> = whole
        .iter()
        .map(|event| {
            let slot = event.whole.expect("segment emits whole slots");
            (
                slot.start().numerator(),
                slot.start().denominator(),
                event.value.to_bits(),
            )
        })
        .collect();

    for event in &chunked {
        let slot = event.whole.expect("segment emits whole slots");
        let key = (
            slot.start().numerator(),
            slot.start().denominator(),
            event.value.to_bits(),
        );
        assert!(
            whole_keys.contains(&key),
            "chunked slot {key:?} must match the whole-span query"
        );
    }
    let chunked_slots: std::collections::BTreeSet<(i128, i128)> = chunked
        .iter()
        .map(|event| {
            let slot = event.whole.unwrap();
            (slot.start().numerator(), slot.start().denominator())
        })
        .collect();
    assert_eq!(chunked_slots.len(), 8, "chunking must cover every slot");
}

#[test]
fn segment_resamples_discrete_patterns() {
    let events = unit_number_events("m = 10 20 |> segment(4)", "m");
    let values = events.iter().map(|event| event.value).collect::<Vec<_>>();
    assert_eq!(values, [10.0, 10.0, 20.0, 20.0]);
}

#[test]
fn segment_applies_to_sample_patterns() {
    let module = eval_module("drums = bd |> segment(4)", ReplMode::Loose).unwrap();
    let names = sample_names(module.get("drums").unwrap());
    assert_eq!(names, ["bd", "bd", "bd", "bd"]);
}

#[test]
fn segment_rejects_invalid_slot_counts() {
    assert_eval_error_contains(
        "m = rand() |> segment(0)",
        ReplMode::Loose,
        &["`segment` requires a positive integer factor"],
    );
    assert_eval_error_contains(
        "m = rand() |> segment(2000)",
        ReplMode::Loose,
        &["`segment` factor exceeded the maximum allowed bound of 1024"],
    );
}

#[test]
fn range_rescales_unit_interval_values_linearly() {
    let events = unit_number_events("m = 0 0.5 1 |> range(200, 2000)", "m");
    let values = events.iter().map(|event| event.value).collect::<Vec<_>>();
    assert_eq!(values, [200.0, 1100.0, 2000.0]);
}

#[test]
fn range_supports_inverted_bounds() {
    let events = unit_number_events("m = 0 1 |> range(10, -10)", "m");
    let values = events.iter().map(|event| event.value).collect::<Vec<_>>();
    assert_eq!(values, [10.0, -10.0]);
}

#[test]
fn range_composes_after_segment() {
    let first = number_event_keys("m = rand() |> segment(8) |> range(200, 2000)", "m", 4);
    let second = number_event_keys("m = rand() |> segment(8) |> range(200, 2000)", "m", 4);
    assert_eq!(first, second);
    assert_eq!(first.len(), 32);
    for (_, _, bits) in first {
        let value = f64::from_bits(bits);
        assert!(
            (200.0..=2000.0).contains(&value),
            "expected value within [200, 2000], got {value}"
        );
    }
}

#[test]
fn range_rejects_sample_patterns() {
    assert_eval_error_contains(
        "m = bd |> range(0, 1)",
        ReplMode::Loose,
        &["`range` requires a number pattern argument"],
    );
}

#[test]
fn choose_draws_only_from_the_given_values() {
    let draws = number_event_keys("m = choose(1, 2, 3) |> segment(8)", "m", 8);
    assert_eq!(draws.len(), 64);

    let allowed: std::collections::BTreeSet<u64> = [1.0_f64, 2.0, 3.0]
        .iter()
        .map(|value| value.to_bits())
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for (_, _, bits) in draws {
        assert!(
            allowed.contains(&bits),
            "choose drew a value outside the given set: {}",
            f64::from_bits(bits)
        );
        seen.insert(bits);
    }
    assert_eq!(seen.len(), 3, "64 draws should hit every option");
}

#[test]
fn choose_queries_are_deterministic_across_repeats() {
    let first = number_event_keys("m = choose(1, 2, 3) |> segment(8)", "m", 8);
    let second = number_event_keys("m = choose(1, 2, 3) |> segment(8)", "m", 8);
    assert_eq!(first, second);
}

#[test]
fn choose_is_roughly_uniform_over_many_draws() {
    let draws = number_event_keys("m = choose(0, 1) |> segment(16)", "m", 32);
    assert_eq!(draws.len(), 512);
    let ones = draws
        .iter()
        .filter(|(_, _, bits)| *bits == 1.0_f64.to_bits())
        .count();
    #[allow(clippy::cast_precision_loss)]
    let ratio = ones as f64 / draws.len() as f64;
    assert!(
        (0.35..=0.65).contains(&ratio),
        "expected roughly uniform draws, got ratio {ratio}"
    );
}

#[test]
fn choose_call_sites_have_distinct_streams() {
    let module = eval_module(
        "a = choose(0, 1) |> segment(16)\nb = choose(0, 1) |> segment(16)",
        ReplMode::Loose,
    )
    .unwrap();
    let span = orpheus_lang::render_span(8).unwrap();
    let a = module
        .get("a")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .try_query(&span)
        .unwrap();
    let b = module
        .get("b")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .try_query(&span)
        .unwrap();

    assert_eq!(a.len(), b.len());
    let differs = a
        .iter()
        .zip(b.iter())
        .any(|(left, right)| (left.value - right.value).abs() > f64::EPSILON);
    assert!(differs, "distinct call sites must produce distinct streams");
}

#[test]
fn choose_rejects_non_constant_arguments() {
    assert_eval_error_contains(
        "m = choose(1 2, 3)",
        ReplMode::Loose,
        &["`choose` requires a constant number argument"],
    );
}

#[test]
fn wchoose_respects_relative_weights() {
    let draws = number_event_keys("m = wchoose(0, 1, 1, 3) |> segment(16)", "m", 32);
    assert_eq!(draws.len(), 512);
    let ones = draws
        .iter()
        .filter(|(_, _, bits)| *bits == 1.0_f64.to_bits())
        .count();
    #[allow(clippy::cast_precision_loss)]
    let ratio = ones as f64 / draws.len() as f64;
    assert!(
        (0.6..=0.9).contains(&ratio),
        "expected roughly 75% weighted draws, got ratio {ratio}"
    );
}

#[test]
fn wchoose_never_picks_zero_weight_values() {
    let draws = number_event_keys("m = wchoose(5, 0, 7, 1) |> segment(16)", "m", 16);
    assert_eq!(draws.len(), 256);
    for (_, _, bits) in draws {
        assert_eq!(
            bits,
            7.0_f64.to_bits(),
            "zero-weight values must never be chosen"
        );
    }
}

#[test]
fn wchoose_rejects_invalid_weights() {
    assert_eval_error_contains(
        "m = wchoose(1, -0.5, 2, 1)",
        ReplMode::Loose,
        &["`wchoose` requires non-negative finite weights"],
    );
    assert_eval_error_contains(
        "m = wchoose(1, 0, 2, 0)",
        ReplMode::Loose,
        &["`wchoose` requires at least one positive weight"],
    );
    assert_eval_error_contains(
        "m = wchoose(1, 1, 2, 1, 3)",
        ReplMode::Loose,
        &["`wchoose` requires value/weight pairs"],
    );
}

#[test]
fn irand_draws_whole_numbers_below_the_bound() {
    let draws = number_event_keys("m = irand(4) |> segment(16)", "m", 16);
    assert_eq!(draws.len(), 256);
    let mut seen = std::collections::BTreeSet::new();
    for (_, _, bits) in &draws {
        let value = f64::from_bits(*bits);
        assert!(
            value.fract() == 0.0 && (0.0..4.0).contains(&value),
            "irand(4) must draw whole numbers in [0, 4), got {value}"
        );
        seen.insert(*bits);
    }
    assert!(seen.len() > 1, "irand should draw more than one value");

    let repeat = number_event_keys("m = irand(4) |> segment(16)", "m", 16);
    assert_eq!(draws, repeat);
}

#[test]
fn rand_segment_range_cutoff_idiom_works_end_to_end() {
    let source = "\
ctrl = rand |> segment(8) |> range(200, 2000) |> cutoff
filtered = bd |> ctrl
";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("filtered").unwrap().as_sample_pattern().unwrap();

    let events = pattern.query_unit().unwrap();
    assert_eq!(events.len(), 8, "8 control slots split the cycle into 8");

    let mut cutoffs = Vec::new();
    for event in &events {
        assert_eq!(event.value.sample(), "bd");
        let cutoff = event
            .value
            .lpf_cutoff_hz()
            .expect("cutoff control must set the filter cutoff");
        assert!(
            (200.0..=2000.0).contains(&cutoff),
            "cutoff {cutoff} outside [200, 2000]"
        );
        cutoffs.push(cutoff);
    }
    let distinct = cutoffs
        .iter()
        .map(|value| value.to_bits())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(distinct.len() > 1, "slots should carry different cutoffs");

    // Deterministic: re-evaluating the module yields the same cutoffs.
    let module_again = eval_module(source, ReplMode::Loose).unwrap();
    let events_again = module_again
        .get("filtered")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    let cutoffs_again = events_again
        .iter()
        .map(|event| event.value.lpf_cutoff_hz().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(cutoffs, cutoffs_again);
}

// --- randcat/wrandcat + the off/rot/chunk/shuffle/scramble transform family ---

/// Materializes a number pattern one cycle at a time so tests can assert
/// per-cycle content.
fn number_values_per_cycle(source: &str, name: &str, cycles: i64) -> Vec<Vec<f64>> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get(name).unwrap().as_number_pattern().unwrap();
    (0..cycles)
        .map(|cycle| {
            pattern
                .try_query(&cycle_time_span(cycle))
                .unwrap()
                .into_iter()
                .map(|event| event.value)
                .collect()
        })
        .collect()
}

#[test]
fn randcat_plays_exactly_one_argument_pattern_per_cycle() {
    let per_cycle = number_values_per_cycle("m = randcat(0 1, 2 3 4)", "m", 32);
    let mut saw_first = false;
    let mut saw_second = false;
    for values in &per_cycle {
        match values.as_slice() {
            [a, b] if (*a - 0.0).abs() < f64::EPSILON && (*b - 1.0).abs() < f64::EPSILON => {
                saw_first = true;
            }
            [a, b, c]
                if (*a - 2.0).abs() < f64::EPSILON
                    && (*b - 3.0).abs() < f64::EPSILON
                    && (*c - 4.0).abs() < f64::EPSILON =>
            {
                saw_second = true;
            }
            other => panic!("cycle played something that is not an argument pattern: {other:?}"),
        }
    }
    assert!(saw_first, "randcat never played its first argument");
    assert!(saw_second, "randcat never played its second argument");
}

#[test]
fn randcat_queries_are_deterministic_across_repeats() {
    let first = number_event_keys("m = randcat(0 1, 2 3)", "m", 16);
    let second = number_event_keys("m = randcat(0 1, 2 3)", "m", 16);
    assert_eq!(first, second);
}

#[test]
fn randcat_choice_is_roughly_uniform_over_many_cycles() {
    let per_cycle = number_values_per_cycle("m = randcat(0, 1)", "m", 128);
    let zeros = per_cycle
        .iter()
        .filter(|values| values.as_slice() == [0.0])
        .count();
    assert!(
        (32..=96).contains(&zeros),
        "expected a roughly uniform split, got {zeros} zeros out of 128"
    );
}

#[test]
fn randcat_is_chunking_stable() {
    let module = eval_module("m = randcat(0 1, 2 3)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    assert!(!whole.is_empty());
    assert_eq!(whole.len(), chunked.len());
    for (a, b) in whole.iter().zip(chunked.iter()) {
        assert_eq!(a.part.start(), b.part.start());
        assert!((a.value - b.value).abs() < f64::EPSILON);
    }
}

#[test]
fn randcat_localizes_child_cycles_like_slowcat() {
    // The `every(2, ...)` child advances its own localized cycle counter
    // (`cycle div 2` with two children), so whenever it plays on global cycle
    // `k` it is transformed exactly when `(k div 2) mod 2 == 0`, i.e.
    // `k mod 4 < 2` -- independent of which cycles the PRNG picks it on.
    let per_cycle =
        number_values_per_cycle("m = randcat(every(2, transpose(100), 0 1), 7)", "m", 64);
    let mut saw_transformed = false;
    let mut saw_untransformed = false;
    for (cycle, values) in per_cycle.iter().enumerate() {
        match values.as_slice() {
            [v] => assert!((*v - 7.0).abs() < f64::EPSILON),
            [a, b] => {
                let expect_transformed = cycle % 4 < 2;
                let expected = if expect_transformed {
                    saw_transformed = true;
                    [100.0, 101.0]
                } else {
                    saw_untransformed = true;
                    [0.0, 1.0]
                };
                assert!(
                    (*a - expected[0]).abs() < f64::EPSILON
                        && (*b - expected[1]).abs() < f64::EPSILON,
                    "cycle {cycle}: expected {expected:?}, got [{a}, {b}]"
                );
            }
            other => panic!("cycle {cycle} played unexpected content: {other:?}"),
        }
    }
    assert!(saw_transformed, "the every-child never played transformed");
    assert!(
        saw_untransformed,
        "the every-child never played untransformed"
    );
}

#[test]
fn randcat_supports_sample_patterns() {
    let module = eval_module("drums = randcat(bd, sn)", ReplMode::Loose).unwrap();
    let names = exported_sample_names(module.get("drums").unwrap(), 32);
    assert_eq!(names.len(), 32, "expected exactly one event per cycle");
    assert!(names.iter().all(|name| name == "bd" || name == "sn"));
    assert!(names.iter().any(|name| name == "bd"));
    assert!(names.iter().any(|name| name == "sn"));
}

#[test]
fn randcat_requires_matching_pattern_kinds() {
    assert_eval_error_contains(
        "drums = randcat(bd, 1 2)",
        ReplMode::Loose,
        &["`randcat`", "same pattern kind"],
    );
}

#[test]
fn wrandcat_zero_weight_children_are_never_played() {
    let per_cycle = number_values_per_cycle("m = wrandcat(0, 1, 1, 0)", "m", 32);
    for values in &per_cycle {
        assert_eq!(values.as_slice(), [0.0]);
    }
}

#[test]
fn wrandcat_weights_bias_the_choice() {
    let per_cycle = number_values_per_cycle("m = wrandcat(0, 1, 1, 3)", "m", 128);
    let ones = per_cycle
        .iter()
        .filter(|values| values.as_slice() == [1.0])
        .count();
    // Expected 96 of 128 (weight 3 of 4); allow a wide deterministic margin.
    assert!(
        (70..=122).contains(&ones),
        "expected the weight-3 child on roughly three quarters of cycles, got {ones}/128"
    );
}

#[test]
fn wrandcat_requires_pattern_weight_pairs() {
    assert_eval_error_contains(
        "m = wrandcat(0, 1, 2, 3, 4)",
        ReplMode::Loose,
        &["`wrandcat`", "pattern/weight pairs"],
    );
}

#[test]
fn wrandcat_rejects_negative_weights_and_all_zero_weights() {
    assert_eval_error_contains(
        "m = wrandcat(0, -1.0, 1, 1)",
        ReplMode::Loose,
        &["`wrandcat`", "non-negative"],
    );
    assert_eval_error_contains(
        "m = wrandcat(0, 0, 1, 0)",
        ReplMode::Loose,
        &["`wrandcat`", "positive weight"],
    );
}

// --- pchoose/wpchoose: per-slot random choice among pattern values ---
//
// `pchoose(p1, p2, ...)` splits each cycle into as many equal slots as the
// busiest argument has events that cycle, and each slot independently plays
// the slot-slice of one argument chosen at deterministic, site-salted
// random. `wpchoose(p1, w1, p2, w2, ...)` biases the per-slot draw by the
// interleaved weights (the `wchoose`/`wrandcat` convention).

/// Float equality within `f64::EPSILON`, for asserting on drawn values.
fn is_value(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < f64::EPSILON
}

#[test]
fn pchoose_plays_only_argument_pattern_events_per_slot() {
    let per_cycle = number_values_per_cycle("m = pchoose(0 1, 2 3)", "m", 32);
    let mut saw_first = false;
    let mut saw_second = false;
    let mut saw_mixed_cycle = false;
    for (cycle, values) in per_cycle.iter().enumerate() {
        let [a, b] = values.as_slice() else {
            panic!("cycle {cycle}: expected exactly two slot events, got {values:?}");
        };
        assert!(
            is_value(*a, 0.0) || is_value(*a, 2.0),
            "cycle {cycle}: slot 0 played a non-argument value {a}"
        );
        assert!(
            is_value(*b, 1.0) || is_value(*b, 3.0),
            "cycle {cycle}: slot 1 played a non-argument value {b}"
        );
        if is_value(*a, 0.0) && is_value(*b, 1.0) {
            saw_first = true;
        }
        if is_value(*a, 2.0) && is_value(*b, 3.0) {
            saw_second = true;
        }
        if is_value(*a, 0.0) != is_value(*b, 1.0) {
            saw_mixed_cycle = true;
        }
    }
    assert!(saw_first, "pchoose never played its first argument");
    assert!(saw_second, "pchoose never played its second argument");
    assert!(
        saw_mixed_cycle,
        "pchoose never mixed arguments within one cycle, so it is not per-slot"
    );
}

#[test]
fn pchoose_queries_are_deterministic_across_repeats() {
    let first = number_event_keys("m = pchoose(0 1, 2 3)", "m", 16);
    let second = number_event_keys("m = pchoose(0 1, 2 3)", "m", 16);
    assert_eq!(first, second);
}

#[test]
fn pchoose_is_chunking_stable_at_cycle_and_sub_cycle_granularity() {
    let module = eval_module("m = pchoose(0 1, 2 3)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();

    let mut per_cycle = Vec::new();
    for cycle in 0..8 {
        per_cycle.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    let mut half_cycle = Vec::new();
    for half in 0..16 {
        let chunk = TimeSpan::new(
            Rational::new(half, 2).unwrap(),
            Rational::new(half + 1, 2).unwrap(),
        )
        .unwrap();
        half_cycle.extend(pattern.try_query(&chunk).unwrap());
    }

    assert!(!whole.is_empty());
    for chunked in [per_cycle, half_cycle] {
        assert_eq!(whole.len(), chunked.len());
        for (a, b) in whole.iter().zip(chunked.iter()) {
            assert_eq!(a.part.start(), b.part.start());
            assert!((a.value - b.value).abs() < f64::EPSILON);
        }
    }
}

#[test]
fn pchoose_call_sites_have_distinct_streams() {
    let module = eval_module("a = pchoose(0, 1)\nb = pchoose(0, 1)", ReplMode::Loose).unwrap();
    let span = orpheus_lang::render_span(32).unwrap();
    let a = module
        .get("a")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .try_query(&span)
        .unwrap();
    let b = module
        .get("b")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .try_query(&span)
        .unwrap();

    assert_eq!(a.len(), b.len());
    let differs = a
        .iter()
        .zip(b.iter())
        .any(|(left, right)| (left.value - right.value).abs() > f64::EPSILON);
    assert!(differs, "distinct call sites must produce distinct streams");
}

#[test]
fn pchoose_choice_is_roughly_uniform_over_many_slots() {
    let per_cycle = number_values_per_cycle("m = pchoose(0 0 0 0, 1 1 1 1)", "m", 128);
    let draws: Vec<f64> = per_cycle.into_iter().flatten().collect();
    assert_eq!(draws.len(), 512);
    let ones = draws.iter().filter(|value| is_value(**value, 1.0)).count();
    #[allow(clippy::cast_precision_loss)]
    let ratio = ones as f64 / draws.len() as f64;
    assert!(
        (0.35..=0.65).contains(&ratio),
        "expected roughly uniform per-slot draws, got ratio {ratio}"
    );
}

#[test]
fn pchoose_slot_grid_follows_the_busiest_argument() {
    let per_cycle = number_values_per_cycle("m = pchoose(0 1 2 3, 9)", "m", 32);
    let mut saw_mixed_cycle = false;
    for (cycle, values) in per_cycle.iter().enumerate() {
        assert_eq!(
            values.len(),
            4,
            "cycle {cycle}: expected the four-slot grid of the busiest argument"
        );
        for (slot, value) in values.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let own = slot as f64;
            assert!(
                is_value(*value, own) || is_value(*value, 9.0),
                "cycle {cycle} slot {slot}: expected {own} or 9, got {value}"
            );
        }
        if values.iter().any(|v| is_value(*v, 9.0)) && values.iter().any(|v| !is_value(*v, 9.0)) {
            saw_mixed_cycle = true;
        }
    }
    assert!(
        saw_mixed_cycle,
        "pchoose never mixed the sparse and dense arguments within one cycle"
    );
}

#[test]
fn pchoose_composes_under_slowcat() {
    // Even cycles play the pchoose slots, odd cycles play the constant.
    let per_cycle = number_values_per_cycle("m = cat(pchoose(0 1, 2 3), 7)", "m", 16);
    for (cycle, values) in per_cycle.iter().enumerate() {
        if cycle % 2 == 0 {
            let [a, b] = values.as_slice() else {
                panic!("cycle {cycle}: expected two slot events, got {values:?}");
            };
            assert!(
                is_value(*a, 0.0) || is_value(*a, 2.0),
                "cycle {cycle}: bad slot 0 value {a}"
            );
            assert!(
                is_value(*b, 1.0) || is_value(*b, 3.0),
                "cycle {cycle}: bad slot 1 value {b}"
            );
        } else {
            assert_eq!(values.as_slice(), [7.0], "cycle {cycle}");
        }
    }
}

#[test]
fn pchoose_composes_under_fast() {
    let per_cycle = number_values_per_cycle("m = pchoose(0, 1) |> fast(2)", "m", 32);
    let mut draws = Vec::new();
    for (cycle, values) in per_cycle.iter().enumerate() {
        assert_eq!(
            values.len(),
            2,
            "cycle {cycle}: fast(2) should squeeze two one-slot cycles into one"
        );
        draws.extend(values.iter().copied());
    }
    assert!(draws.iter().all(|v| is_value(*v, 0.0) || is_value(*v, 1.0)));
    assert!(draws.iter().any(|v| is_value(*v, 0.0)));
    assert!(draws.iter().any(|v| is_value(*v, 1.0)));
}

#[test]
fn pchoose_supports_sample_patterns_from_mini_notation() {
    let module = eval_module("drums = pchoose(bd*2, sn cp)", ReplMode::Loose).unwrap();
    let names = exported_sample_names(module.get("drums").unwrap(), 32);
    assert_eq!(
        names.len(),
        64,
        "expected exactly two slot events per cycle"
    );
    for pair in names.chunks(2) {
        assert!(
            pair[0] == "bd" || pair[0] == "sn",
            "slot 0 played a non-argument sample {}",
            pair[0]
        );
        assert!(
            pair[1] == "bd" || pair[1] == "cp",
            "slot 1 played a non-argument sample {}",
            pair[1]
        );
    }
    for expected in ["bd", "sn", "cp"] {
        assert!(
            names.iter().any(|name| name == expected),
            "pchoose never played `{expected}`"
        );
    }
}

#[test]
fn pchoose_requires_matching_pattern_kinds() {
    assert_eval_error_contains(
        "drums = pchoose(bd, 1 2)",
        ReplMode::Loose,
        &["`pchoose`", "same pattern kind"],
    );
}

#[test]
fn wpchoose_zero_weight_patterns_are_never_played() {
    let per_cycle = number_values_per_cycle("m = wpchoose(0, 1, 1, 0)", "m", 32);
    for values in &per_cycle {
        assert_eq!(values.as_slice(), [0.0]);
    }
}

#[test]
fn wpchoose_weights_bias_the_choice() {
    let per_cycle = number_values_per_cycle("m = wpchoose(0 0 0 0, 1, 1 1 1 1, 3)", "m", 128);
    let draws: Vec<f64> = per_cycle.into_iter().flatten().collect();
    assert_eq!(draws.len(), 512);
    let ones = draws.iter().filter(|value| is_value(**value, 1.0)).count();
    // Expected 384 of 512 (weight 3 of 4); allow a wide deterministic margin.
    assert!(
        (300..=460).contains(&ones),
        "expected the weight-3 pattern on roughly three quarters of slots, got {ones}/512"
    );
}

#[test]
fn wpchoose_requires_pattern_weight_pairs() {
    assert_eval_error_contains(
        "m = wpchoose(0, 1, 2, 3, 4)",
        ReplMode::Loose,
        &["`wpchoose`", "pattern/weight pairs"],
    );
}

#[test]
fn wpchoose_rejects_negative_weights_and_all_zero_weights() {
    assert_eval_error_contains(
        "m = wpchoose(0, -1.0, 1, 1)",
        ReplMode::Loose,
        &["`wpchoose`", "non-negative"],
    );
    assert_eval_error_contains(
        "m = wpchoose(0, 0, 1, 0)",
        ReplMode::Loose,
        &["`wpchoose`", "positive weight"],
    );
}

#[test]
fn wpchoose_queries_are_deterministic_across_repeats() {
    let first = number_event_keys("m = wpchoose(0 1, 1, 2 3, 2)", "m", 16);
    let second = number_event_keys("m = wpchoose(0 1, 1, 2 3, 2)", "m", 16);
    assert_eq!(first, second);
}

// --- markov: state-transition pattern sequencing ---
//
// `markov(s0, w0_0, ..., w0_{k-1}, s1, w1_0, ..., w1_{k-1}, ...)` takes `k`
// state patterns, each followed by its `k` outgoing transition weights (in
// state order). Cycle 0 plays the first state; each later cycle draws the
// next state from the current state's weight row, deterministically from the
// call-site salt and the cycle number. Cycles before 0 clamp to the initial
// state.

#[test]
fn markov_queries_are_deterministic_across_repeats() {
    let source = "m = markov(0 1, 1, 2, 2 3 4, 3, 1)";
    let first = number_event_keys(source, "m", 16);
    let second = number_event_keys(source, "m", 16);
    assert!(!first.is_empty());
    assert_eq!(first, second);
}

#[test]
fn markov_is_chunking_stable() {
    let module = eval_module("m = markov(0 1, 1, 2, 2 3, 3, 1)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    assert!(!whole.is_empty());
    assert_eq!(whole.len(), chunked.len());
    for (a, b) in whole.iter().zip(chunked.iter()) {
        assert_eq!(a.part.start(), b.part.start());
        assert!((a.value - b.value).abs() < f64::EPSILON);
    }
}

#[test]
fn markov_state_sequence_is_stable_across_queries() {
    let source = "m = markov(0, 1, 2, 1, 3, 1)";
    let first = number_values_per_cycle(source, "m", 20);
    let second = number_values_per_cycle(source, "m", 20);
    assert_eq!(first, second);
    // Querying a late cycle alone must agree with the full walk.
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();
    for cycle in [5_i64, 13, 19] {
        let alone: Vec<f64> = pattern
            .try_query(&cycle_time_span(cycle))
            .unwrap()
            .into_iter()
            .map(|event| event.value)
            .collect();
        let index = usize::try_from(cycle).unwrap();
        assert_eq!(alone, first[index], "cycle {cycle} diverged");
    }
}

#[test]
fn markov_deterministic_matrix_walks_the_exact_state_sequence() {
    // From state 0 all mass goes to state 1 and vice versa: the chain must
    // alternate 0, 1, 0, 1, ... starting from the initial state on cycle 0.
    let per_cycle = number_values_per_cycle("m = markov(0, 0, 1, 1, 1, 0)", "m", 8);
    for (cycle, values) in per_cycle.iter().enumerate() {
        let expected = if cycle % 2 == 0 { 0.0 } else { 1.0 };
        assert_eq!(values.as_slice(), [expected], "cycle {cycle}");
    }
}

#[test]
fn markov_three_state_rotation_cycles_through_all_states() {
    // 0 -> 1 -> 2 -> 0 -> ... with a deterministic three-state matrix.
    let per_cycle =
        number_values_per_cycle("m = markov(0, 0, 1, 0, 1, 0, 0, 1, 2, 1, 0, 0)", "m", 9);
    for (cycle, values) in per_cycle.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let expected = (cycle % 3) as f64;
        assert_eq!(values.as_slice(), [expected], "cycle {cycle}");
    }
}

#[test]
fn markov_weights_bias_state_occupancy() {
    // Both rows put weight 3 on state 1 and weight 1 on state 0, so the
    // stationary occupancy of state 1 is 3/4; allow a wide margin.
    let per_cycle = number_values_per_cycle("m = markov(0, 1, 3, 1, 1, 3)", "m", 128);
    let ones = per_cycle
        .iter()
        .filter(|values| values.as_slice() == [1.0])
        .count();
    assert!(
        (70..=122).contains(&ones),
        "expected state 1 on roughly three quarters of cycles, got {ones}/128"
    );
}

#[test]
fn markov_zero_weight_transitions_never_occur() {
    // State 0 only loops onto itself, so the chain can never leave it.
    let per_cycle = number_values_per_cycle("m = markov(0, 1, 0, 1, 1, 0)", "m", 32);
    for values in &per_cycle {
        assert_eq!(values.as_slice(), [0.0]);
    }

    // No row ever puts mass on state 2, so its pattern is never played.
    let per_cycle =
        number_values_per_cycle("m = markov(0, 1, 1, 0, 1, 1, 1, 0, 2, 1, 0, 0)", "m", 64);
    for values in &per_cycle {
        assert_ne!(values.as_slice(), [2.0], "unreachable state 2 was played");
    }
}

#[test]
fn markov_negative_cycles_clamp_to_the_initial_state() {
    let module = eval_module("m = markov(0, 0, 1, 1, 1, 0)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();
    for cycle in [-3_i64, -2, -1] {
        let values: Vec<f64> = pattern
            .try_query(&cycle_time_span(cycle))
            .unwrap()
            .into_iter()
            .map(|event| event.value)
            .collect();
        assert_eq!(
            values,
            [0.0],
            "cycle {cycle} did not play the initial state"
        );
    }
}

#[test]
fn markov_localizes_child_cycles_like_slowcat() {
    // The deterministic alternator plays its `every(2, ...)` first state on
    // even cycles; that child advances a localized (`cycle div 2`) counter,
    // so it is transformed exactly when `(k div 2) mod 2 == 0`.
    let per_cycle = number_values_per_cycle(
        "m = markov(every(2, transpose(100), 0 1), 0, 1, 7, 1, 0)",
        "m",
        16,
    );
    for (cycle, values) in per_cycle.iter().enumerate() {
        if cycle % 2 == 0 {
            let expected = if (cycle / 2) % 2 == 0 {
                [100.0, 101.0]
            } else {
                [0.0, 1.0]
            };
            assert_eq!(values.as_slice(), expected, "cycle {cycle}");
        } else {
            assert_eq!(values.as_slice(), [7.0], "cycle {cycle}");
        }
    }
}

#[test]
fn markov_composes_under_fast_and_slowcat() {
    // fast(2, ...) squeezes chain cycles 2k and 2k+1 into cycle k: the
    // alternator always yields [0, 1] within each cycle.
    let per_cycle = number_values_per_cycle("m = fast(2, markov(0, 0, 1, 1, 1, 0))", "m", 8);
    for (cycle, values) in per_cycle.iter().enumerate() {
        assert_eq!(values.as_slice(), [0.0, 1.0], "cycle {cycle}");
    }

    // Under slowcat the markov child only advances on the cycles it plays
    // (localized `cycle div 2` counter), so it still alternates 0, 1, 0, ...
    let per_cycle = number_values_per_cycle("m = slowcat(markov(0, 0, 1, 1, 1, 0), 7)", "m", 12);
    for (cycle, values) in per_cycle.iter().enumerate() {
        if cycle % 2 == 0 {
            let expected = if (cycle / 2) % 2 == 0 { 0.0 } else { 1.0 };
            assert_eq!(values.as_slice(), [expected], "cycle {cycle}");
        } else {
            assert_eq!(values.as_slice(), [7.0], "cycle {cycle}");
        }
    }
}

#[test]
fn markov_supports_sample_patterns() {
    let module = eval_module("drums = markov(bd, 0, 1, sn, 1, 0)", ReplMode::Loose).unwrap();
    let names = exported_sample_names(module.get("drums").unwrap(), 8);
    assert_eq!(
        names,
        vec!["bd", "sn", "bd", "sn", "bd", "sn", "bd", "sn"],
        "deterministic alternating chain over samples"
    );
}

#[test]
fn markov_requires_matching_pattern_kinds() {
    assert_eval_error_contains(
        "m = markov(bd, 0, 1, 1 2, 1, 0)",
        ReplMode::Loose,
        &["`markov`", "same pattern kind"],
    );
}

#[test]
fn markov_rejects_malformed_argument_shapes() {
    // 7 arguments is not k * (k + 1) for any k >= 2.
    assert_eval_error_contains(
        "m = markov(0, 1, 1, 1, 1, 1, 1)",
        ReplMode::Loose,
        &["`markov`", "transition weights"],
    );
}

#[test]
fn markov_rejects_negative_weights_and_all_zero_rows() {
    assert_eval_error_contains(
        "m = markov(0, -1.0, 1, 1, 1, 1)",
        ReplMode::Loose,
        &["`markov`", "non-negative"],
    );
    assert_eval_error_contains(
        "m = markov(0, 0, 0, 1, 1, 1)",
        ReplMode::Loose,
        &["`markov`", "positive"],
    );
}

#[test]
fn off_overlays_a_shifted_transformed_copy() {
    let events = number_event_keys("m = 0 3 |> off(0.25, transpose(12))", "m", 1);
    // Cycle 0 plays the base events (0 at 0, 3 at 1/2) plus the transformed
    // copy shifted later by 1/4 of a cycle (12 at 1/4, 15 at 3/4). The copy
    // of the previous cycle's final event also bleeds in as an onset-less
    // fragment covering [0, 1/4), matching `shift` semantics.
    assert_eq!(
        events,
        vec![
            (0, 1, 15.0_f64.to_bits()),
            (0, 1, 0.0_f64.to_bits()),
            (1, 4, 12.0_f64.to_bits()),
            (1, 2, 3.0_f64.to_bits()),
            (3, 4, 15.0_f64.to_bits()),
        ]
    );
}

#[test]
fn off_keeps_all_original_events() {
    let base = number_event_keys("m = 0 3", "m", 4);
    let layered = number_event_keys("m = 0 3 |> off(0.25, transpose(12))", "m", 4);
    for event in &base {
        assert!(
            layered.contains(event),
            "off dropped an original event: {event:?}"
        );
    }
}

#[test]
fn off_composes_with_mini_notation_sample_patterns() {
    let keys = sample_event_keys("drums = bd*2 |> off(0.25, gain(0.5))", "drums", 1);
    // Base `bd*2` hits at 0 and 1/2, the shifted copy at 1/4 and 3/4, and the
    // previous cycle's copy bleeds a fragment into the cycle start.
    assert_eq!(
        keys,
        vec![
            (0, 1, "bd".to_owned()),
            (0, 1, "bd".to_owned()),
            (1, 4, "bd".to_owned()),
            (1, 2, "bd".to_owned()),
            (3, 4, "bd".to_owned()),
        ]
    );
}

#[test]
fn rot_rotates_values_while_keeping_onsets() {
    let base = number_event_keys("m = 10 20 30 40", "m", 1);
    let rotated = number_event_keys("m = 10 20 30 40 |> rot(1)", "m", 1);

    let onsets =
        |events: &[(i128, i128, u64)]| events.iter().map(|(n, d, _)| (*n, *d)).collect::<Vec<_>>();
    assert_eq!(onsets(&base), onsets(&rotated), "rot must not move onsets");

    let values = rotated
        .iter()
        .map(|(_, _, bits)| f64::from_bits(*bits))
        .collect::<Vec<_>>();
    assert_eq!(values, [20.0, 30.0, 40.0, 10.0]);
}

#[test]
fn rot_zero_is_identity() {
    let base = number_event_keys("m = 10 20 30 40", "m", 2);
    let rotated = number_event_keys("m = 10 20 30 40 |> rot(0)", "m", 2);
    assert_eq!(base, rotated);
}

#[test]
fn rot_wraps_and_supports_negative_steps() {
    let by_five = number_event_keys("m = 10 20 30 40 |> rot(5)", "m", 1);
    let by_one = number_event_keys("m = 10 20 30 40 |> rot(1)", "m", 1);
    assert_eq!(by_five, by_one, "rot must wrap modulo the onset count");

    let backwards = number_event_keys("m = 10 20 30 40 |> rot(-1)", "m", 1);
    let values = backwards
        .iter()
        .map(|(_, _, bits)| f64::from_bits(*bits))
        .collect::<Vec<_>>();
    assert_eq!(values, [40.0, 10.0, 20.0, 30.0]);
}

#[test]
fn chunk_transforms_one_part_per_cycle_in_rotation() {
    let per_cycle = number_values_per_cycle("m = 0 1 2 3 |> chunk(4, transpose(100))", "m", 5);
    assert_eq!(per_cycle[0], [100.0, 1.0, 2.0, 3.0]);
    assert_eq!(per_cycle[1], [0.0, 101.0, 2.0, 3.0]);
    assert_eq!(per_cycle[2], [0.0, 1.0, 102.0, 3.0]);
    assert_eq!(per_cycle[3], [0.0, 1.0, 2.0, 103.0]);
    assert_eq!(
        per_cycle[4],
        [100.0, 1.0, 2.0, 3.0],
        "cycle 4 wraps to part 0"
    );
}

#[test]
fn chunk_transforms_every_part_exactly_once_over_n_cycles() {
    let per_cycle = number_values_per_cycle("m = 0 1 2 3 |> chunk(4, transpose(100))", "m", 4);
    let mut transformed_parts = Vec::new();
    for values in &per_cycle {
        let parts = values
            .iter()
            .enumerate()
            .filter(|(_, value)| **value >= 100.0)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(parts.len(), 1, "exactly one part per cycle is transformed");
        transformed_parts.push(parts[0]);
    }
    transformed_parts.sort_unstable();
    assert_eq!(transformed_parts, [0, 1, 2, 3]);
}

#[test]
fn chunk_back_sweeps_in_the_reverse_direction() {
    let per_cycle = number_values_per_cycle("m = 0 1 2 3 |> chunk_back(4, transpose(100))", "m", 2);
    assert_eq!(per_cycle[0], [0.0, 1.0, 2.0, 103.0]);
    assert_eq!(per_cycle[1], [0.0, 1.0, 102.0, 3.0]);
}

#[test]
fn shuffle_plays_a_permutation_of_the_slots_each_cycle() {
    let per_cycle = number_values_per_cycle("m = 10 20 30 40 |> shuffle(4)", "m", 16);
    let mut distinct_orders = std::collections::BTreeSet::new();
    for (cycle, values) in per_cycle.iter().enumerate() {
        assert_eq!(values.len(), 4, "cycle {cycle} must keep all four slots");
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        assert_eq!(
            sorted,
            [10.0, 20.0, 30.0, 40.0],
            "cycle {cycle} must be a permutation of the slot values"
        );
        distinct_orders.insert(
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
        );
    }
    assert!(
        distinct_orders.len() >= 2,
        "shuffle should produce different orders on different cycles"
    );
}

#[test]
fn shuffle_keeps_the_slot_onsets() {
    let base = number_event_keys("m = 10 20 30 40", "m", 4);
    let shuffled = number_event_keys("m = 10 20 30 40 |> shuffle(4)", "m", 4);
    let onsets =
        |events: &[(i128, i128, u64)]| events.iter().map(|(n, d, _)| (*n, *d)).collect::<Vec<_>>();
    assert_eq!(onsets(&base), onsets(&shuffled));
}

#[test]
fn shuffle_queries_are_deterministic_and_chunking_stable() {
    let first = number_event_keys("m = 10 20 30 40 |> shuffle(4)", "m", 8);
    let second = number_event_keys("m = 10 20 30 40 |> shuffle(4)", "m", 8);
    assert_eq!(first, second);

    let module = eval_module("m = 10 20 30 40 |> shuffle(4)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();
    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }
    assert_eq!(whole.len(), chunked.len());
    for (a, b) in whole.iter().zip(chunked.iter()) {
        assert_eq!(a.part.start(), b.part.start());
        assert!((a.value - b.value).abs() < f64::EPSILON);
    }
}

#[test]
fn scramble_draws_slots_independently_with_repeats() {
    let per_cycle = number_values_per_cycle("m = 10 20 30 40 |> scramble(4)", "m", 32);
    let mut saw_repeat = false;
    for (cycle, values) in per_cycle.iter().enumerate() {
        assert_eq!(values.len(), 4, "cycle {cycle} must fill all four slots");
        for value in values {
            assert!(
                [10.0, 20.0, 30.0, 40.0].contains(value),
                "cycle {cycle} drew a value that is not a slot value: {value}"
            );
        }
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        sorted.dedup();
        if sorted.len() < 4 {
            saw_repeat = true;
        }
    }
    assert!(
        saw_repeat,
        "scramble draws with replacement, so 32 cycles should contain a repeat"
    );
}

#[test]
fn scramble_queries_are_deterministic_and_chunking_stable() {
    let first = number_event_keys("m = 10 20 30 40 |> scramble(4)", "m", 8);
    let second = number_event_keys("m = 10 20 30 40 |> scramble(4)", "m", 8);
    assert_eq!(first, second);

    let module = eval_module("m = 10 20 30 40 |> scramble(4)", ReplMode::Loose).unwrap();
    let pattern = module.get("m").unwrap().as_number_pattern().unwrap();
    let span = orpheus_lang::render_span(8).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    let mut chunked = Vec::new();
    for cycle in 0..8 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }
    assert_eq!(whole.len(), chunked.len());
    for (a, b) in whole.iter().zip(chunked.iter()) {
        assert_eq!(a.part.start(), b.part.start());
        assert!((a.value - b.value).abs() < f64::EPSILON);
    }
}

#[test]
fn shuffle_and_scramble_reject_non_positive_slot_counts() {
    assert_eval_error_contains(
        "m = shuffle(0, 0 1)",
        ReplMode::Loose,
        &["`shuffle`", "positive integer"],
    );
    assert_eval_error_contains(
        "m = scramble(0, 0 1)",
        ReplMode::Loose,
        &["`scramble`", "positive integer"],
    );
}

// --- euclid rotation / euclid_inv / euclid_full / run / scan / whenmod ---

/// Materializes a number pattern's unit-cycle events as `(start, end)` part
/// boundary pairs.
fn number_part_spans(source: &str, name: &str) -> Vec<(Rational, Rational)> {
    let module = eval_module(source, ReplMode::Loose).unwrap();
    module
        .get(name)
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit()
        .into_iter()
        .map(|event| (*event.part.start(), *event.part.end()))
        .collect()
}

fn eighth(numerator: i64) -> (Rational, Rational) {
    (
        Rational::new(numerator, 8).unwrap(),
        Rational::new(numerator + 1, 8).unwrap(),
    )
}

#[test]
fn euclid_rotation_shifts_gates_left() {
    // Base (3, 8) is x..x..x. (gates at steps 0, 3, 6); rotating by 1 plays
    // original step `i + 1` at step `i`, giving gates at steps 2, 5, 7.
    assert_eq!(
        number_part_spans("clave = euclid(3, 8, 1)", "clave"),
        vec![eighth(2), eighth(5), eighth(7)]
    );
    // Rotation 2 shifts gates to steps 1, 4, 6.
    assert_eq!(
        number_part_spans("clave = euclid(3, 8, 2)", "clave"),
        vec![eighth(1), eighth(4), eighth(6)]
    );
}

#[test]
fn euclid_rotation_wraps_and_accepts_negative_offsets() {
    let base = number_part_spans("clave = euclid(3, 8)", "clave");
    assert_eq!(number_part_spans("clave = euclid(3, 8, 8)", "clave"), base);
    assert_eq!(number_part_spans("clave = euclid(3, 8, 0)", "clave"), base);
    // Rotating by -1 is the same as rotating by steps - 1.
    assert_eq!(
        number_part_spans("clave = euclid(3, 8, -1)", "clave"),
        number_part_spans("clave = euclid(3, 8, 7)", "clave")
    );
}

#[test]
fn euclid_two_arg_form_is_unchanged_by_rotation_support() {
    assert_eq!(
        number_part_spans("clave = euclid(3, 8)", "clave"),
        vec![eighth(0), eighth(3), eighth(6)]
    );
}

#[test]
fn euclid_rejects_fractional_or_excess_rotation_arguments() {
    assert_eval_error_contains(
        "clave = euclid(3, 8, 0.5)",
        ReplMode::Loose,
        &["`euclid`", "rotation", "whole number"],
    );
    assert_eval_error_contains(
        "clave = euclid(3, 8, 1, 2)",
        ReplMode::Loose,
        &["`euclid`", "at most"],
    );
}

#[test]
fn euclid_inv_plays_exactly_where_euclid_rests() {
    let hits = number_part_spans("clave = euclid(3, 8)", "clave");
    let rests = number_part_spans("clave = euclid_inv(3, 8)", "clave");

    assert_eq!(hits.len(), 3);
    assert_eq!(rests.len(), 5);
    for span in &rests {
        assert!(!hits.contains(span), "euclid_inv overlaps euclid: {span:?}");
    }
    let mut union = hits;
    union.extend(rests);
    union.sort();
    assert_eq!(union, (0..8).map(eighth).collect::<Vec<_>>());
}

#[test]
fn euclid_inv_honors_rotation() {
    // euclid(3, 8, 1) gates land on steps 2, 5, 7; the inversion covers the rest.
    assert_eq!(
        number_part_spans("clave = euclid_inv(3, 8, 1)", "clave"),
        vec![eighth(0), eighth(1), eighth(3), eighth(4), eighth(6)]
    );
}

#[test]
fn euclid_full_plays_hits_and_rests_from_two_patterns() {
    let module = eval_module("drums = euclid_full(3, 8, bd*8, sn*8)", ReplMode::Loose).unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    assert_eq!(events.len(), 8);
    assert_eq!(
        events
            .iter()
            .map(|event| event.value.sample().to_owned())
            .collect::<Vec<_>>(),
        vec!["bd", "sn", "sn", "bd", "sn", "sn", "bd", "sn"]
    );
}

#[test]
fn euclid_full_accepts_a_rotation_argument() {
    let module = eval_module("drums = euclid_full(3, 8, 1, bd*8, sn*8)", ReplMode::Loose).unwrap();
    let events = module
        .get("drums")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();

    // Gates rotate to steps 2, 5, 7.
    assert_eq!(
        events
            .iter()
            .map(|event| event.value.sample().to_owned())
            .collect::<Vec<_>>(),
        vec!["sn", "sn", "bd", "sn", "sn", "bd", "sn", "bd"]
    );
}

#[test]
fn euclid_full_rejects_mismatched_pattern_kinds() {
    assert_eval_error_contains(
        "drums = euclid_full(3, 8, bd*8, 1 2)",
        ReplMode::Loose,
        &["`euclid_full`", "same kind"],
    );
}

#[test]
fn run_counts_upward_once_per_cycle() {
    let module = eval_module("ramp = run(4)", ReplMode::Loose).unwrap();
    let events = module
        .get("ramp")
        .unwrap()
        .as_number_pattern()
        .unwrap()
        .query_unit();

    assert_eq!(events.len(), 4);
    for (index, event) in events.iter().enumerate() {
        let index_i64 = i64::try_from(index).unwrap();
        assert_eq!(event.part.start(), &Rational::new(index_i64, 4).unwrap());
        assert_eq!(event.part.end(), &Rational::new(index_i64 + 1, 4).unwrap());
        #[allow(clippy::cast_precision_loss)]
        let expected = index as f64;
        assert!((event.value - expected).abs() < f64::EPSILON);
    }
}

#[test]
fn run_repeats_the_same_ramp_every_cycle() {
    let module = eval_module("ramp = run(3)", ReplMode::Loose).unwrap();
    let pattern = module.get("ramp").unwrap().as_number_pattern().unwrap();
    let span = orpheus_lang::render_span(2).unwrap();
    let values = pattern
        .try_query(&span)
        .unwrap()
        .into_iter()
        .map(|event| event.value)
        .collect::<Vec<_>>();

    assert_eq!(values, [0.0, 1.0, 2.0, 0.0, 1.0, 2.0]);
}

#[test]
fn run_rejects_non_positive_step_counts() {
    assert_eval_error_contains(
        "ramp = run(0)",
        ReplMode::Loose,
        &["`run`", "positive whole number"],
    );
}

#[test]
fn scan_grows_the_prefix_each_cycle_and_wraps_like_slowcat() {
    let module = eval_module("ramp = scan(3)", ReplMode::Loose).unwrap();
    let pattern = module.get("ramp").unwrap().as_number_pattern().unwrap();

    let cycle_values = |cycle: i64| -> Vec<f64> {
        pattern
            .try_query(&cycle_time_span(cycle))
            .unwrap()
            .into_iter()
            .map(|event| event.value)
            .collect()
    };

    // Tidal `scan n = slowcat $ map run [1 .. n]`: cycle k plays
    // run((k mod n) + 1), restarting the growth loop after the full run.
    assert_eq!(cycle_values(0), [0.0]);
    assert_eq!(cycle_values(1), [0.0, 1.0]);
    assert_eq!(cycle_values(2), [0.0, 1.0, 2.0]);
    assert_eq!(cycle_values(3), [0.0]);
    assert_eq!(cycle_values(4), [0.0, 1.0]);
    assert_eq!(cycle_values(5), [0.0, 1.0, 2.0]);
    assert_eq!(cycle_values(6), [0.0]);
}

#[test]
fn scan_handles_negative_cycles_with_slowcat_modulo() {
    let module = eval_module("ramp = scan(3)", ReplMode::Loose).unwrap();
    let pattern = module.get("ramp").unwrap().as_number_pattern().unwrap();

    let cycle_values = |cycle: i64| -> Vec<f64> {
        pattern
            .try_query(&cycle_time_span(cycle))
            .unwrap()
            .into_iter()
            .map(|event| event.value)
            .collect()
    };

    // Like `slowcat`, negative cycles use a euclidean modulo: cycle -1 plays
    // run(3), cycle -2 plays run(2), cycle -3 wraps back to run(1).
    assert_eq!(cycle_values(-1), [0.0, 1.0, 2.0]);
    assert_eq!(cycle_values(-2), [0.0, 1.0]);
    assert_eq!(cycle_values(-3), [0.0]);
}

#[test]
fn scan_subdivides_each_cycle_into_its_prefix_length() {
    let module = eval_module("ramp = scan(3)", ReplMode::Loose).unwrap();
    let pattern = module.get("ramp").unwrap().as_number_pattern().unwrap();
    let events = pattern.try_query(&cycle_time_span(1)).unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::new(1, 1).unwrap());
    assert_eq!(events[0].part.end(), &Rational::new(3, 2).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(3, 2).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(2, 1).unwrap());
}

#[test]
fn scan_is_stable_regardless_of_query_window_chunking() {
    let module = eval_module("ramp = scan(4)", ReplMode::Loose).unwrap();
    let pattern = module.get("ramp").unwrap().as_number_pattern().unwrap();

    let span = orpheus_lang::render_span(6).unwrap();
    let whole = pattern.try_query(&span).unwrap();
    let mut chunked = Vec::new();
    for cycle in 0..6 {
        chunked.extend(pattern.try_query(&cycle_time_span(cycle)).unwrap());
    }

    assert_eq!(whole.len(), chunked.len());
    for (a, b) in whole.iter().zip(chunked.iter()) {
        assert_eq!(a.part.start(), b.part.start());
        assert!((a.value - b.value).abs() < f64::EPSILON);
    }
}

#[test]
fn scan_rejects_non_positive_step_counts() {
    assert_eval_error_contains(
        "ramp = scan(0)",
        ReplMode::Loose,
        &["`scan`", "positive whole number"],
    );
}

#[test]
fn whenmod_applies_transform_when_cycle_mod_reaches_threshold() {
    let module = eval_module("drums = bd sn |> whenmod(4, 2, rev)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 8);

    // rev applies on cycles where cycle mod 4 >= 2: cycles 2, 3, 6, 7.
    assert_eq!(
        events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "bd", "sn", "bd", "sn", "sn", "bd", "sn", "bd", "bd", "sn", "bd", "sn", "sn", "bd",
            "sn", "bd",
        ]
    );
}

#[test]
fn whenmod_with_zero_threshold_transforms_every_cycle() {
    let module = eval_module("drums = bd sn |> whenmod(3, 0, rev)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 3);

    assert_eq!(
        events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["sn", "bd", "sn", "bd", "sn", "bd"]
    );
}

#[test]
fn whenmod_rejects_thresholds_at_or_above_the_period() {
    assert_eval_error_contains(
        "drums = bd sn |> whenmod(3, 3, rev)",
        ReplMode::Loose,
        &["`whenmod`", "threshold less than the period"],
    );
}

// --- rational (non-integer) factors for fast/slow ---

/// Extracts each event's `(start_num, start_den, end_num, end_den, sample)`
/// from exported JSON so spans can be asserted as exact rationals.
fn exact_sample_spans(events: &[JsonValue]) -> Vec<(i64, i64, i64, i64, String)> {
    events
        .iter()
        .map(|event| {
            (
                event["start_num"].as_i64().unwrap(),
                event["start_den"].as_i64().unwrap(),
                event["end_num"].as_i64().unwrap(),
                event["end_den"].as_i64().unwrap(),
                event["sample"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn fast_accepts_fractional_factors_with_exact_rational_spans() {
    let module = eval_module("drums = fast(1.5, bd sn)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 2);

    assert_eq!(
        exact_sample_spans(&events),
        vec![
            (0, 1, 1, 3, "bd".to_owned()),
            (1, 3, 2, 3, "sn".to_owned()),
            (2, 3, 1, 1, "bd".to_owned()),
            (1, 1, 4, 3, "sn".to_owned()),
            (4, 3, 5, 3, "bd".to_owned()),
            (5, 3, 2, 1, "sn".to_owned()),
        ]
    );
}

#[test]
fn fast_three_halves_over_two_cycles_matches_fast_three_over_one_cycle_stretched() {
    // fast(3/2, p) over [0, 2) plays exactly what fast(3, p) plays over
    // [0, 1), stretched by 2: event k's exact rational start doubles.
    let fractional = eval_module("drums = fast(1.5, bd sn)", ReplMode::Loose).unwrap();
    let integral = eval_module("drums = fast(3, bd sn)", ReplMode::Loose).unwrap();
    let fractional_events = exported_sample_events(fractional.get("drums").unwrap(), 2);
    let integral_events = exported_sample_events(integral.get("drums").unwrap(), 1);

    assert_eq!(fractional_events.len(), integral_events.len());
    for (frac, int) in fractional_events.iter().zip(&integral_events) {
        assert_eq!(frac["sample"], int["sample"]);
        let frac_start = Rational::checked_from_parts(
            i128::from(frac["start_num"].as_i64().unwrap()),
            i128::from(frac["start_den"].as_i64().unwrap()),
        )
        .unwrap();
        let int_start = Rational::checked_from_parts(
            i128::from(int["start_num"].as_i64().unwrap()) * 2,
            i128::from(int["start_den"].as_i64().unwrap()),
        )
        .unwrap();
        assert_eq!(frac_start, int_start);
    }
}

#[test]
fn slow_fractional_factor_is_inverted_by_fast_with_the_same_factor() {
    let round_trip =
        eval_module("drums = fast(1.5, slow(1.5, bd sn cp))", ReplMode::Loose).unwrap();
    let plain = eval_module("drums = bd sn cp", ReplMode::Loose).unwrap();

    let round_trip_events = exported_sample_events(round_trip.get("drums").unwrap(), 6);
    let plain_events = exported_sample_events(plain.get("drums").unwrap(), 6);

    assert_eq!(round_trip_events, plain_events);
}

#[test]
fn slow_one_half_is_equivalent_to_fast_two() {
    let slowed = eval_module("drums = slow(0.5, bd sn)", ReplMode::Loose).unwrap();
    let fasted = eval_module("drums = fast(2, bd sn)", ReplMode::Loose).unwrap();

    assert_eq!(
        exported_sample_events(slowed.get("drums").unwrap(), 3),
        exported_sample_events(fasted.get("drums").unwrap(), 3),
    );
}

#[test]
fn integer_fast_factors_keep_their_existing_exact_behavior() {
    let module = eval_module("drums = fast(2, bd sn)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 1);

    assert_eq!(
        exact_sample_spans(&events),
        vec![
            (0, 1, 1, 4, "bd".to_owned()),
            (1, 4, 1, 2, "sn".to_owned()),
            (1, 2, 3, 4, "bd".to_owned()),
            (3, 4, 1, 1, "sn".to_owned()),
        ]
    );
}

#[test]
fn fast_boundary_integer_factors_still_work() {
    let identity = eval_module("drums = fast(1, bd sn)", ReplMode::Loose).unwrap();
    let plain = eval_module("drums = bd sn", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_events(identity.get("drums").unwrap(), 2),
        exported_sample_events(plain.get("drums").unwrap(), 2),
    );

    let max = eval_module("drums = fast(1024, bd)", ReplMode::Loose).unwrap();
    assert_eq!(
        exported_sample_events(max.get("drums").unwrap(), 1).len(),
        1024
    );
}

#[test]
fn fast_fractional_wholes_straddle_cycle_boundaries_with_exact_parts() {
    // fast(0.75, bd sn) = slow(4/3): sn's whole is [2/3, 4/3), which
    // straddles the cycle-0/cycle-1 boundary. Its clipped part inside a
    // 1-cycle export window ends exactly at 1.
    let module = eval_module("drums = fast(0.75, bd sn)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 1);

    assert_eq!(
        exact_sample_spans(&events),
        vec![(0, 1, 2, 3, "bd".to_owned()), (2, 3, 1, 1, "sn".to_owned()),]
    );
}

#[test]
fn fast_decimal_point_one_becomes_exactly_one_tenth() {
    // 0.1 converts to the exact rational 1/10 (decimal-literal rendering,
    // not the f64 bit pattern), so fast(0.1, bd) == slow(10, bd): one bd
    // spanning exactly [0, 10).
    let module = eval_module("drums = fast(0.1, bd)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 10);

    assert_eq!(
        exact_sample_spans(&events),
        vec![(0, 1, 10, 1, "bd".to_owned())]
    );
}

#[test]
fn fast_rejects_non_positive_and_out_of_range_fractional_factors() {
    assert_eval_error_contains(
        "drums = fast(0, bd sn)",
        ReplMode::Loose,
        &["`fast` requires a positive factor"],
    );
    assert_eval_error_contains(
        "drums = fast(-1.5, bd sn)",
        ReplMode::Loose,
        &["`fast` requires a positive factor"],
    );
    // 0.0001 = 1/10000: the reduced denominator exceeds the 1024 bound.
    assert_eval_error_contains(
        "drums = fast(0.0001, bd sn)",
        ReplMode::Loose,
        &["`fast` factor denominator exceeded the maximum allowed bound of 1024"],
    );
    // 0.123456789 = 123456789/1000000000: reduced denominator is still huge.
    assert_eval_error_contains(
        "drums = fast(0.123456789, bd sn)",
        ReplMode::Loose,
        &["`fast` factor denominator exceeded the maximum allowed bound of 1024"],
    );
    assert_eval_error_contains(
        "drums = slow(1024.5, bd sn)",
        ReplMode::Loose,
        &["`slow` factor exceeded the maximum allowed bound of 1024"],
    );
}

#[test]
fn fractional_fast_composes_with_slowcat_and_every() {
    let module = eval_module("drums = cat(fast(1.5, bd sn), cp)", ReplMode::Loose).unwrap();
    let events = exported_sample_events(module.get("drums").unwrap(), 2);
    // Cycle 0 plays fast(3/2, bd sn) => bd sn bd at thirds; cycle 1 plays cp.
    assert_eq!(
        exact_sample_spans(&events),
        vec![
            (0, 1, 1, 3, "bd".to_owned()),
            (1, 3, 2, 3, "sn".to_owned()),
            (2, 3, 1, 1, "bd".to_owned()),
            (1, 1, 2, 1, "cp".to_owned()),
        ]
    );

    let every = eval_module("drums = bd sn |> every(2, fast(1.5))", ReplMode::Loose).unwrap();
    let every_events = exported_sample_events(every.get("drums").unwrap(), 2);
    // Cycle 0 is transformed: fast(3/2, bd sn) plays bd sn bd at thirds
    // within cycle 0. Cycle 1 is the plain bd sn.
    assert_eq!(
        every_events
            .iter()
            .map(|event| event["sample"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["bd", "sn", "bd", "bd", "sn"]
    );
}
