use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_lang::{FunctionValue, ReplMode, Value, eval_module, export_sample_pattern_to_json};
use orpheus_pattern::{Rational, TimeSpan};
use serde_json::Value as JsonValue;

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
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
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

    assert!(matches!(module.get("fx"), Some(Value::Pedal(_))));
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
            .map(|event| (event.part.start().clone(), event.part.end().clone()))
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
                event.part.start().clone(),
                event.part.end().clone()
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
