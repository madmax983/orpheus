use orpheus_lang::{ReplMode, eval_module};

#[test]
fn explicit_time_mixed_types_returns_error() {
    let result = eval_module(
        "test = stream(at(0, sample(\"bd\")), at(0, 1))",
        ReplMode::Strict,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "explicit-time items must all resolve to the same pattern kind"
    );
}

#[test]
fn explicit_time_seq_sections_mixed_types_returns_error() {
    let result = eval_module(
        "test = seq_sections(section(at(0, sample(\"bd\")), 1), section(at(0, 1), 1))",
        ReplMode::Strict,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "explicit-time items must all resolve to the same pattern kind"
    );
}

#[test]
fn section_cycle_count_exceeds_max() {
    let result = eval_module(
        "test = seq_sections(section(at(0, 1), 1025))",
        ReplMode::Strict,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "section cycle count exceeded the maximum allowed bound of 1024"
    );
}

#[test]
fn section_cycle_count_exceeds_max_in_events() {
    let result = eval_module(
        "test = seq_sections(section(at(0, 1), 1), section(at(0, 1), 1025))",
        ReplMode::Strict,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "section cycle count exceeded the maximum allowed bound of 1024"
    );
}

#[test]
fn stream_empty_items_returns_error() {
    let result = eval_module("test = stream()", ReplMode::Strict);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`stream` requires at least one item"
    );
}

#[test]
fn seq_sections_empty_sections_returns_error() {
    let result = eval_module("test = seq_sections()", ReplMode::Strict);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`seq_sections` requires at least one section"
    );
}

#[test]
fn section_outside_seq_sections_returns_error() {
    let result = eval_module("test = section(at(0, 1), 1)", ReplMode::Strict);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`section(...)` can only appear inside `seq_sections(...)`"
    );
}

#[test]
fn seq_sections_invalid_item_returns_error() {
    let result = eval_module("test = seq_sections(at(0, 1))", ReplMode::Strict);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`seq_sections` only accepts `section(pattern, cycles)` items"
    );
}

#[test]
fn seq_sections_invalid_item_in_length_returns_error() {
    let result = eval_module(
        "test = seq_sections(section(at(0, 1), 1), at(0, 1))",
        ReplMode::Strict,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`seq_sections` only accepts `section(pattern, cycles)` items"
    );
}

#[test]
fn beat_outside_meter_returns_error() {
    let result = eval_module("test = at(beat(1), 1)", ReplMode::Strict);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`beat(...)` requires an enclosing `meter(...)`"
    );
}

#[test]
fn section_cycle_count_must_be_positive_returns_error() {
    let result = eval_module(
        "test = seq_sections(section(at(0, 1), 0))",
        ReplMode::Strict,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "section cycle count must be a positive integer"
    );
}
