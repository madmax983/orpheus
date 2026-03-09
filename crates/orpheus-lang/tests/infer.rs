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
