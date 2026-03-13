use orpheus_lang::{ReplMode, infer_module};

#[test]
fn loose_mode_infers_number_sequences_as_number_patterns() {
    let typed = infer_module("cutoff = 400 800 1200", ReplMode::Loose).unwrap();

    assert_eq!(typed.type_of("cutoff").to_string(), "Pattern<Number>");
}

#[test]
fn strict_mode_rejects_mixed_stack_types() {
    let error = infer_module("layer = stack(bd sn, 1 2)", ReplMode::Strict).unwrap_err();

    assert!(error.to_string().contains("Pattern<Sample>"));
}

#[test]
fn rev_preserves_sample_pattern_types() {
    let typed = infer_module("drums = rev(bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn shift_preserves_pattern_types() {
    let sample_typed = infer_module("drums = shift(0.25, bd sn)", ReplMode::Strict).unwrap();
    let number_typed = infer_module("swing = shift(0.5, 1 2)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(number_typed.type_of("swing").to_string(), "Pattern<Number>");
}

#[test]
fn meter_stream_and_sections_infer_sample_patterns() {
    let typed = infer_module(
        "song = seq_sections(section(meter(4, 4, stream(at(beat(0), bd), at(beat(2), sn))), 2))",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("song").to_string(), "Pattern<Sample>");
}

#[test]
fn meter_prefix_annotation_infers_sample_patterns() {
    let typed = infer_module(
        "song = seq_sections(section(meter(4, 4) stream(at(beat(0), bd), at(beat(2), sn)), 2))",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("song").to_string(), "Pattern<Sample>");
}

#[test]
fn sample_builtin_infers_sample_pattern_from_string_literal() {
    let typed = infer_module(r#"lead = sample("vox_ah")"#, ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn sample_calls_in_sequences_still_infer_sample_patterns() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") sample("vox_oh")"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn rate_and_slice_builtins_preserve_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") |> slice(0.25, 1) |> rate(2) |> pan(0.5)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn pattern_valued_slice_preserves_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("amen") |> slice(0 0.25, 0.5 1)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn dynamic_and_negative_rate_preserve_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") |> rate(0.5 -1)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn pitch_preserves_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") |> pitch(0 12 -12)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn pattern_valued_gain_and_pan_preserve_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") |> gain(0.25 0.75) |> pan(-0.5 0.5)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn filter_builtins_preserve_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("vox_ah") |> lpf(400 800) |> hpf(100 200)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn slice_idx_preserves_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("amen") |> slice_idx(3, 8)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn pattern_valued_slice_idx_preserves_sample_pattern_types() {
    let typed = infer_module(
        r#"lead = sample("amen") |> slice_idx(0 3 1 7, 8)"#,
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn multiple_top_level_bindings_infer_in_order() {
    let typed = infer_module("verse = bd sn\nsong = fast(2, verse)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("verse").to_string(), "Pattern<Sample>");
    assert_eq!(typed.type_of("song").to_string(), "Pattern<Sample>");
}
