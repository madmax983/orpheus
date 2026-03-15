use orpheus_lang::{ReplMode, Value, eval_module};
use orpheus_pattern::{Rational, TimeSpan};

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
