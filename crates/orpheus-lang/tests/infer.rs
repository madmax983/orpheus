use orpheus_lang::{ReplMode, Type, infer_module};

#[test]
fn loose_mode_infers_number_sequences_as_number_patterns() {
    let typed = infer_module("cutoff = 400 800 1200", ReplMode::Loose).unwrap();

    assert_eq!(typed.type_of("cutoff").to_string(), "Pattern<Number>");
}

#[test]
fn sequences_with_rests_infer_from_non_rest_items() {
    let typed = infer_module("drums = bd ~ cp ~", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
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
fn every_infers_a_polymorphic_pattern_transform_function() {
    let typed = infer_module("warp = every(2, fast(2))", ReplMode::Strict).unwrap();

    match typed.type_of("warp") {
        Type::Function(args, ret) => {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], *ret.clone());
            assert!(matches!(args[0], Type::Pattern(_)));
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn sometimes_infers_a_polymorphic_pattern_transform_function() {
    let typed = infer_module("warp = sometimes(fast(2))", ReplMode::Strict).unwrap();

    match typed.type_of("warp") {
        Type::Function(args, ret) => {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], *ret.clone());
            assert!(matches!(args[0], Type::Pattern(_)));
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn when_infers_a_polymorphic_pattern_transform_function() {
    let typed = infer_module("warp = when(3, 1, fast(2))", ReplMode::Strict).unwrap();

    match typed.type_of("warp") {
        Type::Function(args, ret) => {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], *ret.clone());
            assert!(matches!(args[0], Type::Pattern(_)));
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn every_preserves_sample_pattern_types() {
    let typed = infer_module("drums = every(2, fast(2), bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn sometimes_preserves_sample_pattern_types() {
    let typed = infer_module("drums = sometimes(rev, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn when_preserves_sample_pattern_types() {
    let typed = infer_module("drums = when(3, 1, rev, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
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

#[test]
fn parameterized_binding_infers_a_curried_function_type() {
    let typed = infer_module("swing amt pat = pat |> shift(amt)", ReplMode::Strict).unwrap();

    match typed.type_of("swing") {
        Type::Function(first_args, first_ret) => {
            assert_eq!(first_args.as_slice(), &[Type::pattern(Type::Number)]);
            match first_ret.as_ref() {
                Type::Function(second_args, second_ret) => {
                    assert_eq!(second_args.len(), 1);
                    assert_eq!(second_args[0], *second_ret.clone());
                    assert!(matches!(second_args[0], Type::Pattern(_)));
                }
                other => panic!("expected curried return function, got {other:?}"),
            }
        }
        other => panic!("expected curried function type, got {other:?}"),
    }
}

#[test]
fn parameterized_bindings_are_generalized_at_each_use_site() {
    let typed = infer_module(
        "id pat = pat\n\
         drums = id(bd sn)\n\
         cutoff = id(400 800)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(typed.type_of("cutoff").to_string(), "Pattern<Number>");
}

#[test]
fn sequencing_syntax_still_parses_as_sequence() {
    let typed = infer_module("drums = bd sn cp", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn within_preserves_sample_pattern_types() {
    let typed = infer_module("drums = within(0, 0.5, rev, bd sn cp hh)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn within_accepts_parameterized_unary_transforms() {
    let typed = infer_module(
        "swing amt pat = pat |> shift(amt)\n\
         drums = within(0, 0.5, swing(0.25), bd sn cp hh)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn when_accepts_parameterized_unary_transforms() {
    let typed = infer_module(
        "swing amt pat = pat |> shift(amt)\n\
         drums = when(2, 1, swing(0.25), bd sn)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn mask_infers_a_polymorphic_gate_function() {
    let typed = infer_module("mute = mask(bd ~ cp ~)", ReplMode::Strict).unwrap();

    match typed.type_of("mute") {
        Type::Function(args, ret) => {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], *ret.clone());
            assert!(matches!(args[0], Type::Pattern(_)));
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn mask_preserves_sample_pattern_types() {
    let typed = infer_module("drums = mask(bd ~ cp ~, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn mask_accepts_number_pattern_gates() {
    let typed = infer_module("drums = mask(1 ~ 1 ~, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn mask_accepts_parameterized_bindings() {
    let typed = infer_module(
        "keep gate pat = pat |> mask(gate)\n\
         drums = keep(bd ~ cp ~)(bd sn)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn euclid_infers_number_patterns() {
    let typed = infer_module("clave = euclid(3, 8)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("clave").to_string(), "Pattern<Number>");
}

#[test]
fn euclid_masks_preserve_source_pattern_types() {
    let typed = infer_module(
        "drums = mask(euclid(3, 8), bd sn cp hh bd sn cp hh)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn pitch_class_set_infers_first_class_values() {
    let typed = infer_module("hirajoshi = pitch_class_set(0 2 3 7 8)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("hirajoshi").to_string(), "PitchClassSet");
}

#[test]
fn degrees_accept_canonical_pitch_class_set_bindings() {
    let typed = infer_module("line = degrees(aeolian, 0 2 4)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("line").to_string(), "Pattern<Number>");
}

#[test]
fn degrees_accept_user_defined_pitch_class_sets() {
    let typed = infer_module(
        "hirajoshi = pitch_class_set(0 2 3 7 8)\n\
         line = degrees(hirajoshi, 0 1 2 4)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("line").to_string(), "Pattern<Number>");
}

#[test]
fn degrees_transpose_preserves_number_pattern_types() {
    let typed = infer_module(
        "line = degrees(aeolian, 0 2 4) |> transpose(45)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("line").to_string(), "Pattern<Number>");
}

#[test]
fn degrees_reject_string_collection_arguments() {
    let error = infer_module(r#"line = degrees("aeolian", 0 2 4)"#, ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("PitchClassSet") || message.contains("degrees"));
}

#[test]
fn named_pitch_literals_infer_number_patterns() {
    let typed = infer_module("melody = c4 ef4 g4 bf4", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("melody").to_string(), "Pattern<Number>");
}

#[test]
fn named_pitch_literals_compose_with_transforms() {
    let typed = infer_module("riff = fs4 a4 cs5 |> fast(2)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("riff").to_string(), "Pattern<Number>");
}

#[test]
fn named_pitch_literals_report_pitch_specific_diagnostics() {
    let error = infer_module("bad = cf", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("pitch"));
    assert!(message.contains("octave"));
}

#[test]
fn chord_infers_number_patterns() {
    let typed = infer_module("pad = chord(c4, 0 4 7)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn chord_infers_over_root_sequences() {
    let typed = infer_module("line = chord(c4 e4, 0 7)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("line").to_string(), "Pattern<Number>");
}

#[test]
fn chord_accepts_degree_derived_roots() {
    let typed = infer_module(
        "harm = chord(degrees(aeolian, 0 2) |> transpose(60), 0 3 7)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("harm").to_string(), "Pattern<Number>");
}
