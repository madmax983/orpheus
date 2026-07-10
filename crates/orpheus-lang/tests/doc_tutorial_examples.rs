//! Verification harness for the runnable snippets in `docs/tutorial/`.
//!
//! Every code block presented in the tutorial as something the reader can type
//! is exercised here through the same `eval_module` entry point the language
//! uses, so the documentation cannot drift away from the real surface. If a
//! snippet stops evaluating, fix the tutorial to use a real form — do not
//! weaken these assertions to accept a fake example.

use orpheus_lang::{ReplMode, Value, eval_module};

/// Collect the ordered one-cycle sample names for a binding, mirroring the
/// `sample_names` helper in `tests/eval.rs`.
fn sample_names(source: &str, binding: &str) -> Vec<String> {
    let module = eval_module(source, ReplMode::Loose)
        .unwrap_or_else(|error| panic!("`{source}` should evaluate, but failed: {error}"));
    module
        .get(binding)
        .unwrap_or_else(|| panic!("`{source}` should bind `{binding}`"))
        .as_sample_pattern()
        .unwrap_or_else(|| panic!("`{binding}` should be a sample pattern"))
        .query_unit()
        .unwrap()
        .into_iter()
        .map(|event| event.value.sample().to_owned())
        .collect()
}

/// Assert a snippet evaluates cleanly and binds `binding` to a sample pattern,
/// returning its one-cycle event count.
fn sample_event_count(source: &str, binding: &str) -> usize {
    sample_names(source, binding).len()
}

/// Assert a snippet evaluates cleanly in the given mode (used for forms whose
/// audio output cannot be inspected as discrete sample events, e.g. voices).
fn assert_evaluates(source: &str, mode: ReplMode) -> Value {
    let module = eval_module(source, mode)
        .unwrap_or_else(|error| panic!("`{source}` should evaluate, but failed: {error}"));
    module.into_values().next().expect("a bound value")
}

// ---------------------------------------------------------------------------
// Chapter 1 / 2: sequences and notation.
// ---------------------------------------------------------------------------

#[test]
fn chapter1_first_sound_is_a_four_event_sequence() {
    assert_eq!(
        sample_names("d1 = bd sn cp sn", "d1"),
        ["bd", "sn", "cp", "sn"]
    );
}

#[test]
fn chapter2_rests_hold_their_slot() {
    // `bd ~ sn ~` keeps two audible events out of four steps.
    assert_eq!(sample_names("d = bd ~ sn ~", "d"), ["bd", "sn"]);
}

#[test]
fn chapter2_alternation_advances_one_entry_per_cycle() {
    // `bd <sn cp>` plays `bd sn` on cycle 0 (query_unit reads cycle 0).
    assert_eq!(sample_names("d = bd <sn cp>", "d"), ["bd", "sn"]);
}

#[test]
fn chapter2_replicate_expands_to_separate_steps() {
    assert_eq!(sample_names("d = bd!3 sn", "d"), ["bd", "bd", "bd", "sn"]);
}

#[test]
fn chapter2_star_repeats_within_a_step() {
    assert_eq!(sample_names("d = bd*2 sn", "d"), ["bd", "bd", "sn"]);
}

#[test]
fn chapter2_inline_euclid_spreads_pulses_over_steps() {
    // The tresillo: three kicks distributed across eight slots.
    let names = sample_names("d = bd(3,8)", "d");
    assert_eq!(names.len(), 3);
    assert!(names.iter().all(|name| name == "bd"));
}

#[test]
fn chapter2_top_level_comma_stacks_layers() {
    // Kick pattern and hat pattern layered into one binding.
    let names = sample_names("d = bd ~ bd ~, hh hh hh hh", "d");
    assert_eq!(names.len(), 6);
    assert_eq!(names.iter().filter(|name| *name == "bd").count(), 2);
    assert_eq!(names.iter().filter(|name| *name == "hh").count(), 4);
}

#[test]
fn chapter2_polymeter_shares_a_step_count() {
    // `{bd sn cp, hh hh}%4` steps both subsequences four times per cycle.
    assert_eq!(sample_event_count("d = {bd sn cp, hh hh}%4", "d"), 8);
}

#[test]
fn chapter2_degrade_modifier_thins_a_dense_run() {
    // The `?` modifier drops some events; the result is deterministic and
    // strictly between empty and the full eight hats.
    let count = sample_event_count("d = hh*8?", "d");
    assert!(
        (1..8).contains(&count),
        "degrade should thin the run: {count}"
    );
}

// ---------------------------------------------------------------------------
// Chapter 3: transforms.
// ---------------------------------------------------------------------------

#[test]
fn chapter3_fast_pipe_repeats_within_the_cycle() {
    assert_eq!(
        sample_names("d = bd sn |> fast(2)", "d"),
        ["bd", "sn", "bd", "sn"]
    );
}

#[test]
fn chapter3_pipe_chain_of_fast_and_gain() {
    // A multi-stage pipe: notation, then a time transform, then a control.
    assert_eq!(
        sample_names("d = bd sn |> fast(2) |> gain(0.8)", "d"),
        ["bd", "sn", "bd", "sn"]
    );
}

#[test]
fn chapter3_rev_reverses_the_cycle() {
    assert_eq!(
        sample_names("d = bd sn cp hh |> rev", "d"),
        ["hh", "cp", "sn", "bd"]
    );
}

#[test]
fn chapter3_every_applies_its_transform_on_cycle_zero() {
    // On cycle 0, `every(2, fast(2))` fires, doubling `bd sn`.
    assert_eq!(
        sample_names("d = bd sn |> every(2, fast(2))", "d"),
        ["bd", "sn", "bd", "sn"]
    );
}

#[test]
fn chapter3_within_transforms_only_part_of_the_cycle() {
    // Reversing only the first half of `bd sn cp hh` swaps the first two.
    assert_eq!(
        sample_names("d = bd sn cp hh |> within(0, 0.5, rev)", "d"),
        ["sn", "bd", "cp", "hh"]
    );
}

#[test]
fn chapter3_off_layers_a_shifted_copy() {
    // `off` adds a delayed, gained copy, so the cycle carries more events than
    // the two-step original.
    assert!(sample_event_count("d = bd sn |> off(0.25, gain(0.5))", "d") > 2);
}

#[test]
fn chapter3_jux_widens_into_two_channels() {
    // `jux(rev)` stacks the original with a reversed copy: eight events from
    // four.
    assert_eq!(sample_event_count("d = bd sn cp hh |> jux(rev)", "d"), 8);
}

#[test]
fn chapter3_mask_with_euclid_keeps_aligned_events() {
    // `mask(euclid(5, 8))` keeps five of eight hats.
    assert_eq!(sample_event_count("d = bd*8 |> mask(euclid(5, 8))", "d"), 5);
}

#[test]
fn chapter3_segment_and_range_sample_a_continuous_signal() {
    // `rand |> segment(8) |> range(...)` yields a number pattern of eight
    // per-cycle values inside the requested range.
    let module = eval_module(
        "n = rand |> segment(8) |> range(200, 2000)",
        ReplMode::Loose,
    )
    .expect("the segment/range chain should evaluate");
    let events = module
        .get("n")
        .unwrap()
        .as_number_pattern()
        .expect("segment(rand) is a number pattern")
        .query_unit();
    assert_eq!(events.len(), 8);
    for event in &events {
        assert!(
            (200.0..=2000.0).contains(&event.value),
            "range must bound the value: {}",
            event.value
        );
    }
}

#[test]
fn chapter3_stack_function_combines_named_patterns() {
    let source = "kick = bd ~ bd ~\nsnare = ~ sn ~ sn\ndrums = stack(kick, snare)";
    let names = sample_names(source, "drums");
    assert_eq!(names.len(), 4);
}

#[test]
fn chapter3_effect_controls_chain_on_a_pattern() {
    // gain / lpf / reverb attach without changing the four-event rhythm.
    assert_eq!(
        sample_names(
            "d = bd sn cp sn |> gain(0.8) |> lpf(1200) |> reverb(0.2)",
            "d"
        ),
        ["bd", "sn", "cp", "sn"]
    );
}

// ---------------------------------------------------------------------------
// Chapter 4: instruments (`voice { }`) and the pitch-driven token path.
// ---------------------------------------------------------------------------

#[test]
fn chapter4_pluck_voice_evaluates_to_a_voice_value() {
    let value = assert_evaluates(
        "pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }",
        ReplMode::Strict,
    );
    assert!(
        matches!(value, Value::Voice(_)),
        "a voice block should evaluate to a voice value, got {}",
        value.kind_name()
    );
}

#[test]
fn chapter4_acid_voice_with_filter_and_drive_evaluates() {
    let value = assert_evaluates(
        "acid = voice { body = saw(freq) + tri(freq) * 0.5 ; shaped = body |> lowpass(1200, 0.3) |> drive(1.5) ; shaped * ar(gate, 0.001, 0.08) }",
        ReplMode::Strict,
    );
    assert!(matches!(value, Value::Voice(_)));
}

#[test]
fn chapter4_voice_with_per_note_param_evaluates() {
    let value = assert_evaluates(
        "acid = voice { f = saw(freq) |> svf_lp(p1, 0.7) ; f * ar(gate, 0.001, 0.05) }",
        ReplMode::Strict,
    );
    assert!(matches!(value, Value::Voice(_)));
}

#[test]
#[allow(clippy::float_cmp)] // exact control constants pass through unchanged
fn chapter4_pattern_stamps_per_note_params() {
    // `p1(...)` stamps a per-note parameter onto each event (chapter 4).
    let module = eval_module("line = bd sn |> p1(300 6000)", ReplMode::Strict)
        .expect("the p1 control should evaluate");
    let events = module
        .get("line")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .query_unit()
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].value.voice_params()[0], 300.0);
    assert_eq!(events[1].value.voice_params()[0], 6000.0);
}

#[test]
fn chapter4_voice_token_is_driven_by_a_pitch_pattern() {
    // A bound voice becomes a pattern token; `pitch` sets each note and the
    // result stays a sample pattern with one event per step (chapter 4).
    let source = "beep = voice { sine(freq) * ar(gate, 0.001, 0.08) }\n\
                  melody = beep beep beep beep |> pitch(c4 e4 g4 c5)";
    assert_eq!(sample_event_count(source, "melody"), 4);
}

// ---------------------------------------------------------------------------
// Chapter 7: the capstone `.ode` file evaluates end to end in strict mode.
// ---------------------------------------------------------------------------

#[test]
fn chapter7_capstone_ode_file_evaluates_in_strict_mode() {
    let source = include_str!("../../../docs/examples/tutorial_capstone.ode");
    let module =
        eval_module(source, ReplMode::Strict).expect("the capstone .ode should evaluate cleanly");
    assert!(
        module.contains_key("song"),
        "the capstone should bind `song`"
    );
    // The final piece is a playable sample pattern with events in cycle 0.
    let events = module
        .get("song")
        .unwrap()
        .as_sample_pattern()
        .expect("`song` should be a sample pattern")
        .query_unit()
        .unwrap();
    assert!(
        !events.is_empty(),
        "the capstone should produce audio events"
    );
}
