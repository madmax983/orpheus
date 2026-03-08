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
