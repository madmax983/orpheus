//! Integration tests for the type inference engine to guarantee strict checking for `.ode` files and loose, forgiving checking for REPL sessions.
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
fn synth_atoms_and_controls_preserve_sample_pattern_types() {
    let typed = infer_module(
        r"lead = pulse |> cutoff(1200) |> res(0.25 0.5) |> drive(1.2) |> pw(0.35)",
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
fn pedal_graph_binding_infers_pedal_type() {
    let typed = infer_module("fx = graph { level = 0.5 ; level }", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("fx"), &Type::Pedal);
}

#[test]
fn through_applies_pedal_to_sample_pattern() {
    let typed = infer_module(
        "fx = graph { level = 0.5 ; level }\nlead = through(fx, bd sn)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
}

#[test]
fn pedal_value_is_not_a_number_pattern() {
    let error = infer_module(
        "fx = graph { level = 0.5 ; level }\nclock = rate(fx, bd sn)",
        ReplMode::Strict,
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("type mismatch: expected Pattern<Number>, found Pedal")
    );
}

#[test]
fn pedal_value_is_not_a_sample_pattern() {
    let error = infer_module(
        "fx = graph { level = 0.5 ; level }\nlead = rate(1, fx)",
        ReplMode::Strict,
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("type mismatch: expected Pattern<Sample>, found Pedal")
    );
}

#[test]
fn binary_control_expressions_infer_number_patterns() {
    let typed = infer_module("mix = 1 + 2 * 3", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("mix").to_string(), "Pattern<Number>");
}

#[test]
fn named_call_arguments_infer_from_their_rhs_value() {
    let typed = infer_module(r#"lead = sample(name="vox_ah")"#, ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Sample>");
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

#[test]
fn invert_infers_number_patterns() {
    let typed = infer_module("pad = invert(1, chord(c4, 0 4 7))", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn invert_pipe_form_infers_number_patterns() {
    let typed = infer_module("pad = chord(c4, 0 4 7) |> invert(1)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn invert_rejects_sample_patterns_at_typecheck() {
    let error = infer_module("bad = invert(1, bd)", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("expected Number"));
    assert!(message.contains("Sample"));
}

#[test]
fn drop_infers_number_patterns() {
    let typed = infer_module("pad = drop(2, chord(c4, 0 4 7 10))", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn drop_pipe_form_infers_number_patterns() {
    let typed = infer_module("pad = chord(c4, 0 4 7 10) |> drop(2)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn drop_rejects_sample_patterns_at_typecheck() {
    let error = infer_module("bad = drop(2, bd)", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("expected Number"));
    assert!(message.contains("Sample"));
}

#[test]
fn strum_infers_number_patterns() {
    let typed = infer_module("pad = strum(chord(c4, 0 4 7))", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn strum_pipe_form_infers_number_patterns() {
    let typed = infer_module("pad = chord(c4, 0 4 7) |> strum", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("pad").to_string(), "Pattern<Number>");
}

#[test]
fn strum_rejects_sample_patterns_at_typecheck() {
    let error = infer_module("bad = strum(bd)", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("expected Number"));
    assert!(message.contains("Sample"));
}

#[test]
fn arp_infers_number_patterns() {
    let typed = infer_module("lead = arp(5, up, chord(c4, 0 4 7))", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Number>");
}

#[test]
fn arp_pipe_form_infers_number_patterns() {
    let typed = infer_module("lead = chord(c4, 0 4 7) |> arp(5, up)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("lead").to_string(), "Pattern<Number>");
}

#[test]
fn arp_rejects_sample_patterns_at_typecheck() {
    let error = infer_module("bad = arp(5, up, bd)", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("expected Number"));
    assert!(message.contains("Sample"));
}

#[test]
fn arp_rejects_non_direction_arguments_at_typecheck() {
    let error = infer_module("bad = arp(5, c4, chord(c4, 0 4 7))", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("ArpDirection"));
    assert!(message.contains("Pattern<Number>") || message.contains("Number"));
}

#[test]
fn roll_infers_sample_patterns() {
    let typed = infer_module("buzz = roll(4, sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("buzz").to_string(), "Pattern<Sample>");
}

#[test]
fn roll_infers_number_patterns() {
    let typed = infer_module("stabs = roll(4, chord(c4, 0 4 7))", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("stabs").to_string(), "Pattern<Number>");
}

#[test]
fn roll_pipe_form_preserves_sample_patterns() {
    let typed = infer_module("buzz = sn |> roll(4)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("buzz").to_string(), "Pattern<Sample>");
}

#[test]
fn roll_rejects_non_pattern_values_at_typecheck() {
    let error = infer_module("bad = roll(4, aeolian)", ReplMode::Strict).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("Pattern"));
    assert!(message.contains("PitchClassSet"));
}

#[test]
fn cat_preserves_sample_pattern_types() {
    let typed = infer_module("drums = cat(bd, sn cp)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn variadic_cat_accepts_more_than_two_patterns() {
    let typed = infer_module("drums = cat(bd, sn, cp)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn slowcat_and_append_preserve_pattern_types() {
    let slowcat_typed = infer_module("drums = slowcat(bd, sn)", ReplMode::Strict).unwrap();
    let append_typed = infer_module("swing = append(1 2, 3)", ReplMode::Strict).unwrap();

    assert_eq!(
        slowcat_typed.type_of("drums").to_string(),
        "Pattern<Sample>"
    );
    assert_eq!(append_typed.type_of("swing").to_string(), "Pattern<Number>");
}

#[test]
fn iter_preserves_pattern_types() {
    let sample_typed = infer_module("drums = iter(4, bd sn cp hh)", ReplMode::Strict).unwrap();
    let number_typed = infer_module("swing = iter_back(2, 1 2)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(number_typed.type_of("swing").to_string(), "Pattern<Number>");
}

#[test]
fn alternations_infer_the_pattern_type_of_their_elements() {
    let typed = infer_module("drums = bd <sn cp>", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn strict_mode_rejects_mixed_alternation_types() {
    let error = infer_module("drums = <bd 1>", ReplMode::Strict).unwrap_err();

    assert!(error.to_string().contains("Pattern<Sample>"));
}

#[test]
fn degrade_preserves_pattern_types() {
    let sample_typed = infer_module("drums = degrade(bd sn)", ReplMode::Strict).unwrap();
    let number_typed = infer_module("melody = degrade(1 2)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(
        number_typed.type_of("melody").to_string(),
        "Pattern<Number>"
    );
}

#[test]
fn degrade_by_preserves_sample_pattern_types() {
    let typed = infer_module("drums = degrade_by(0.25, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn sometimes_by_infers_a_polymorphic_pattern_transform_function() {
    let typed = infer_module("warp = sometimes_by(0.5, fast(2))", ReplMode::Strict).unwrap();

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
fn sometimes_by_preserves_sample_pattern_types() {
    let typed = infer_module("drums = sometimes_by(0.5, rev, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn often_and_rarely_preserve_sample_pattern_types() {
    let often_typed = infer_module("drums = often(rev, bd sn)", ReplMode::Strict).unwrap();
    let rarely_typed = infer_module("drums = rarely(rev, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(often_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(rarely_typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn almost_always_and_almost_never_preserve_sample_pattern_types() {
    let always_typed = infer_module("drums = almost_always(rev, bd sn)", ReplMode::Strict).unwrap();
    let never_typed = infer_module("drums = almost_never(rev, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(always_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(never_typed.type_of("drums").to_string(), "Pattern<Sample>");
}

// --- segment / range / choose / wchoose / irand typing ---

#[test]
fn segment_preserves_pattern_types() {
    let number_typed = infer_module("m = 1 2 |> segment(4)", ReplMode::Strict).unwrap();
    assert_eq!(number_typed.type_of("m").to_string(), "Pattern<Number>");

    let sample_typed = infer_module("drums = bd sn |> segment(4)", ReplMode::Strict).unwrap();
    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn range_maps_number_patterns_to_number_patterns() {
    let typed = infer_module("m = range(200, 2000, 0 0.5 1)", ReplMode::Strict).unwrap();
    assert_eq!(typed.type_of("m").to_string(), "Pattern<Number>");
}

#[test]
fn range_rejects_sample_patterns() {
    let error = infer_module("m = bd |> range(0, 1)", ReplMode::Strict).unwrap_err();
    assert!(error.to_string().contains("Sample"));
}

#[test]
fn choose_infers_a_number_pattern() {
    let two = infer_module("m = choose(1, 2)", ReplMode::Strict).unwrap();
    assert_eq!(two.type_of("m").to_string(), "Pattern<Number>");

    let variadic = infer_module("m = choose(1, 2, 3, 4)", ReplMode::Strict).unwrap();
    assert_eq!(variadic.type_of("m").to_string(), "Pattern<Number>");
}

#[test]
fn variadic_choose_rejects_sample_arguments() {
    let error = infer_module("m = choose(1, bd, 3)", ReplMode::Strict).unwrap_err();
    assert!(error.to_string().contains("Sample"));
}

#[test]
fn wchoose_infers_a_number_pattern() {
    let base = infer_module("m = wchoose(1, 1, 2, 3)", ReplMode::Strict).unwrap();
    assert_eq!(base.type_of("m").to_string(), "Pattern<Number>");

    let variadic = infer_module("m = wchoose(1, 1, 2, 3, 4, 1)", ReplMode::Strict).unwrap();
    assert_eq!(variadic.type_of("m").to_string(), "Pattern<Number>");
}

#[test]
fn irand_infers_a_number_pattern() {
    let typed = infer_module("m = irand(8)", ReplMode::Strict).unwrap();
    assert_eq!(typed.type_of("m").to_string(), "Pattern<Number>");
}

#[test]
fn bare_rand_pipes_into_segment_range_and_cutoff() {
    let typed = infer_module(
        "ctrl = rand |> segment(8) |> range(200, 2000) |> cutoff",
        ReplMode::Strict,
    )
    .unwrap();

    match typed.type_of("ctrl") {
        Type::Function(args, ret) => {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].to_string(), "Pattern<Sample>");
            assert_eq!(ret.to_string(), "Pattern<Sample>");
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn bare_rand_and_rand_call_coerce_in_argument_position() {
    let bare = infer_module("m = segment(4, rand)", ReplMode::Strict).unwrap();
    assert_eq!(bare.type_of("m").to_string(), "Pattern<Number>");

    let called = infer_module("m = segment(4, rand())", ReplMode::Strict).unwrap();
    assert_eq!(called.type_of("m").to_string(), "Pattern<Number>");
}

#[test]
fn randcat_preserves_pattern_types() {
    let sample_typed = infer_module("drums = randcat(bd, sn cp)", ReplMode::Strict).unwrap();
    let number_typed = infer_module("melody = randcat(0 1, 2 3)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(
        number_typed.type_of("melody").to_string(),
        "Pattern<Number>"
    );
}

#[test]
fn variadic_randcat_accepts_more_than_two_patterns() {
    let typed = infer_module("drums = randcat(bd, sn, cp)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn strict_mode_rejects_mixed_randcat_patterns() {
    let error = infer_module("drums = randcat(bd, 1 2, cp)", ReplMode::Strict).unwrap_err();

    assert!(error.to_string().contains("Pattern"));
}

#[test]
fn wrandcat_preserves_pattern_types() {
    let typed = infer_module("drums = wrandcat(bd, 1, sn, 3)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn variadic_wrandcat_accepts_more_than_two_pairs() {
    let typed = infer_module("drums = wrandcat(bd, 1, sn, 2, cp, 3)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn markov_preserves_pattern_types() {
    let sample_typed =
        infer_module("drums = markov(bd, 0, 1, sn, 1, 0)", ReplMode::Strict).unwrap();
    let number_typed =
        infer_module("melody = markov(0 1, 1, 2, 2 3, 3, 1)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(
        number_typed.type_of("melody").to_string(),
        "Pattern<Number>"
    );
}

#[test]
fn variadic_markov_accepts_three_states() {
    let typed = infer_module(
        "drums = markov(bd, 0, 1, 0, sn, 0, 0, 1, cp, 1, 0, 0)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn strict_mode_rejects_mixed_markov_state_patterns() {
    // The two-state base form is checked by the environment scheme.
    let error = infer_module("drums = markov(bd, 0, 1, 1 2, 1, 0)", ReplMode::Strict).unwrap_err();
    assert!(error.to_string().contains("Sample"));

    // The variadic form is checked by the block-structured special case.
    let error = infer_module(
        "drums = markov(bd, 0, 1, 0, sn, 0, 0, 1, 1 2, 1, 0, 0)",
        ReplMode::Strict,
    )
    .unwrap_err();
    assert!(error.to_string().contains("markov"));
    assert!(error.to_string().contains("same type"));
}

#[test]
fn strict_mode_rejects_non_number_markov_weights() {
    let error = infer_module(
        "drums = markov(bd, 0, 1, 0, sn, 0, 0, 1, cp, bd, 0, 0)",
        ReplMode::Strict,
    )
    .unwrap_err();

    assert!(error.to_string().contains("markov"));
}

#[test]
fn strict_mode_rejects_malformed_markov_argument_counts() {
    let error = infer_module(
        "drums = markov(bd, 0, 1, sn, 1, 0, cp, 1)",
        ReplMode::Strict,
    )
    .unwrap_err();

    assert!(error.to_string().contains("markov"));
}

#[test]
fn off_preserves_pattern_types() {
    let sample_typed = infer_module("drums = off(0.25, rev, bd sn)", ReplMode::Strict).unwrap();
    let number_typed =
        infer_module("melody = 0 3 |> off(0.5, transpose(12))", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(
        number_typed.type_of("melody").to_string(),
        "Pattern<Number>"
    );
}

#[test]
fn rot_preserves_pattern_types() {
    let sample_typed = infer_module("drums = rot(1, bd sn cp)", ReplMode::Strict).unwrap();
    let number_typed = infer_module("melody = 10 20 30 |> rot(2)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(
        number_typed.type_of("melody").to_string(),
        "Pattern<Number>"
    );
}

#[test]
fn chunk_and_chunk_back_preserve_pattern_types() {
    let chunk_typed = infer_module("drums = chunk(4, rev, bd sn cp hh)", ReplMode::Strict).unwrap();
    let back_typed = infer_module(
        "melody = 0 1 2 3 |> chunk_back(4, transpose(12))",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(chunk_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(back_typed.type_of("melody").to_string(), "Pattern<Number>");
}

#[test]
fn shuffle_and_scramble_preserve_pattern_types() {
    let shuffle_typed = infer_module("drums = shuffle(4, bd sn cp hh)", ReplMode::Strict).unwrap();
    let scramble_typed = infer_module("melody = 0 1 2 3 |> scramble(4)", ReplMode::Strict).unwrap();

    assert_eq!(
        shuffle_typed.type_of("drums").to_string(),
        "Pattern<Sample>"
    );
    assert_eq!(
        scramble_typed.type_of("melody").to_string(),
        "Pattern<Number>"
    );
}

#[test]
fn euclid_rotation_infers_number_patterns() {
    let typed = infer_module("clave = euclid(3, 8, 2)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("clave").to_string(), "Pattern<Number>");
}

#[test]
fn euclid_inv_infers_number_patterns() {
    let two_arg = infer_module("clave = euclid_inv(3, 8)", ReplMode::Strict).unwrap();
    let three_arg = infer_module("clave = euclid_inv(3, 8, 1)", ReplMode::Strict).unwrap();

    assert_eq!(two_arg.type_of("clave").to_string(), "Pattern<Number>");
    assert_eq!(three_arg.type_of("clave").to_string(), "Pattern<Number>");
}

#[test]
fn euclid_full_preserves_pattern_types() {
    let samples = infer_module("drums = euclid_full(3, 8, bd*8, sn*8)", ReplMode::Strict).unwrap();
    let rotated =
        infer_module("drums = euclid_full(3, 8, 1, bd*8, sn*8)", ReplMode::Strict).unwrap();
    let numbers = infer_module("line = euclid_full(3, 8, 1 2, 3 4)", ReplMode::Strict).unwrap();

    assert_eq!(samples.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(rotated.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(numbers.type_of("line").to_string(), "Pattern<Number>");
}

#[test]
fn euclid_full_rejects_mismatched_pattern_kinds() {
    let error = infer_module("drums = euclid_full(3, 8, bd*8, 1 2)", ReplMode::Strict).unwrap_err();

    assert!(
        error.to_string().contains("type mismatch"),
        "unexpected error: {error}"
    );
}

#[test]
fn run_and_scan_infer_number_patterns() {
    let run_typed = infer_module("ramp = run(4)", ReplMode::Strict).unwrap();
    let scan_typed = infer_module("ramp = scan(4)", ReplMode::Strict).unwrap();

    assert_eq!(run_typed.type_of("ramp").to_string(), "Pattern<Number>");
    assert_eq!(scan_typed.type_of("ramp").to_string(), "Pattern<Number>");
}

#[test]
fn whenmod_preserves_sample_pattern_types() {
    let typed = infer_module("drums = whenmod(4, 2, rev, bd sn)", ReplMode::Strict).unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}

#[test]
fn whenmod_infers_a_polymorphic_pattern_transform_function() {
    let typed = infer_module("warp = whenmod(4, 2, fast(2))", ReplMode::Strict).unwrap();

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
fn fast_and_slow_accept_fractional_factors_and_preserve_pattern_types() {
    let sample_typed = infer_module("drums = fast(1.5, bd sn)", ReplMode::Strict).unwrap();
    let number_typed = infer_module("swing = slow(0.5, 1 2)", ReplMode::Strict).unwrap();

    assert_eq!(sample_typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(number_typed.type_of("swing").to_string(), "Pattern<Number>");
}
