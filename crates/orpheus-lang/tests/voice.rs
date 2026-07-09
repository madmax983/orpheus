//! Integration tests for the `voice { ... }` definition surface (ADR 0010).
//!
//! A named voice binding compiles to a `GraphVoiceSpec`, registers with the
//! engine's graph voice bank at the next cycle boundary, and its binding name
//! resolves as a pattern token exactly like the built-in `gsine`.

use orpheus_dsp::EngineHandle;
use orpheus_lang::{ReplMode, ReplSession, Value, eval_module};

const PLUCK: &str =
    "pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }";

fn eval_voice(source: &str) -> Value {
    let bindings = eval_module(source, ReplMode::Strict).unwrap();
    bindings.into_values().next().unwrap()
}

fn eval_error(source: &str) -> String {
    eval_module(source, ReplMode::Strict)
        .unwrap_err()
        .to_string()
}

#[test]
fn voice_definition_evaluates_to_voice_value() {
    let value = eval_voice(PLUCK);
    let Value::Voice(voice) = value else {
        panic!("expected a voice value, got {}", value.kind_name());
    };

    // The release tail follows the envelope's release segment.
    assert!((voice.release_seconds() - 0.05).abs() < 1e-6);

    // The compiled spec drives a real graph voice with the fixed interface:
    // 4 control inputs in, one stereo frame out.
    let spec = voice.to_spec("pluck").unwrap();
    assert_eq!(spec.token(), "pluck");
    let mut compiled = spec.build_voice(48_000.0);
    compiled.prepare();
    let mut energy = 0.0_f32;
    for _ in 0..2_400 {
        let (left, right) = compiled.process_frame(1.0, 220.0, 0.5, 0.0);
        energy += left.abs() + right.abs();
    }
    assert!(energy > 1.0, "compiled voice should be audible while gated");
}

#[test]
fn voice_vocabulary_covers_oscillators_filters_and_both_envelopes() {
    let value = eval_voice(
        "acid = voice { body = saw(freq) + tri(freq) * 0.5 ; shaped = body |> lowpass(1200, 0.3) |> drive(1.5) ; shaped * ar(gate, 0.001, 0.08) }",
    );
    let Value::Voice(voice) = value else {
        panic!("expected a voice value, got {}", value.kind_name());
    };
    assert!((voice.release_seconds() - 0.08).abs() < 1e-6);
    assert!(voice.to_spec("acid").is_ok());
}

#[test]
fn voice_without_envelope_still_gets_a_release_tail() {
    let value = eval_voice("beep = voice { sine(freq) * 0.5 }");
    let Value::Voice(voice) = value else {
        panic!("expected a voice value, got {}", value.kind_name());
    };
    assert!(voice.release_seconds() > 0.0);
}

#[test]
fn voice_body_rejects_unknown_stages() {
    let message = eval_error("bad = voice { warble(freq) }");
    assert!(message.contains("warble"), "unexpected error: {message}");
}

#[test]
fn voice_body_rejects_unbound_names() {
    let message = eval_error("bad = voice { osc * 2 }");
    assert!(message.contains("osc"), "unexpected error: {message}");
}

#[test]
fn voice_body_rejects_non_literal_envelope_times() {
    let message = eval_error("bad = voice { sine(freq) * adsr(gate, freq, 0.1, 0.5, 0.1) }");
    assert!(
        message.contains("adsr") && message.contains("literal"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_body_rejects_wrong_arity() {
    let message = eval_error("bad = voice { sine(freq, 2) }");
    assert!(message.contains("sine"), "unexpected error: {message}");
}

#[test]
fn voice_body_rejects_pattern_forms() {
    let message = eval_error("bad = voice { stack(sine(freq), saw(freq)) }");
    assert!(!message.is_empty());
}

fn stereo_frames(interleaved: &[f32]) -> Vec<(f32, f32)> {
    interleaved
        .chunks_exact(2)
        .map(|frame| (frame[0], frame[1]))
        .collect()
}

#[test]
fn session_plays_user_defined_voice_from_pattern_token() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    let banner = session.eval_line(PLUCK).unwrap();
    assert!(banner.contains("Voice"), "unexpected banner: {banner}");
    let _ = session.render_test_block_for_tui(1);

    // The binding name is now a pattern token, like `gsine` (event = 1/4
    // cycle -> 2400 gate frames at 1200 BPM / 48 kHz).
    session.eval_line("melody = pluck ~ ~ ~").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    let frames = stereo_frames(&rendered);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(
        frames[200..2_400]
            .iter()
            .any(|&(left, right)| left.abs() > 0.01 && right.abs() > 0.01),
        "user-defined voice should be audible on both channels during the gate"
    );
    assert!(
        frames[7_000..9_500]
            .iter()
            .all(|&(left, right)| left.abs() < 1e-3 && right.abs() < 1e-3),
        "user-defined voice should decay to near-silence after release"
    );
}

#[test]
fn session_redefining_a_voice_swaps_the_program() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session.eval_line(PLUCK).unwrap();
    session.eval_line("melody = pluck ~ ~ ~").unwrap();
    let _ = session.render_test_block_for_tui(1);
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
    let first = session.render_test_block_for_tui(9_600);

    // Redefine the voice mid-session; the new program applies from the next
    // cycle boundary (built off the audio thread, swapped like a sample bank).
    session
        .eval_line("pluck = voice { sine(freq) * ar(gate, 0.001, 0.03) }")
        .unwrap();
    // One transition cycle: the swap lands at its closing boundary.
    let _ = session.render_test_block_for_tui(9_600);
    let swapped = session.render_test_block_for_tui(9_600);

    assert!(first.iter().any(|sample| sample.abs() > 0.01));
    assert!(swapped.iter().any(|sample| sample.abs() > 0.01));
    assert_ne!(first, swapped, "redefined voice should change the audio");
}

#[test]
fn session_keeps_builtin_tokens_and_unknown_tokens_working() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session.eval_line(PLUCK).unwrap();
    session.eval_line("drums = bd sn").unwrap();
    let _ = session.render_test_block_for_tui(1);
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
    let rendered = session.render_test_block_for_tui(9_600);
    assert!(
        rendered.iter().any(|sample| sample.abs() > 0.01),
        "sample-bank tokens must keep rendering after a voice definition"
    );
}

#[test]
fn voice_binding_summary_reports_voice_type() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(PLUCK).unwrap();
    assert!(
        session
            .binding_summaries()
            .iter()
            .any(|summary| summary == "pluck: Voice"),
        "summaries: {:?}",
        session.binding_summaries()
    );
}

// ---------------------------------------------------------------------------
// Richer voice-body vocabulary (ADR 0010 addendum): feedback loops, fixed
// delays, parallel fan/merge, and the `poly`/`release` pragma bindings.
// ---------------------------------------------------------------------------

const ECHO: &str = "echo = voice { release = 0.5 ; dry = sine(freq) * ar(gate, 0.001, 0.01) ; wet = feedback(dry + fb |> delay(0.05) |> gain(0.5)) ; dry + wet }";

fn compiled_voice(source: &str, token: &str) -> orpheus_dsp::GraphVoice {
    let Value::Voice(voice) = eval_voice(source) else {
        panic!("expected a voice value");
    };
    let mut compiled = voice.to_spec(token).unwrap().build_voice(48_000.0);
    compiled.prepare();
    compiled
}

#[test]
fn voice_feedback_echo_produces_repeating_decaying_onsets() {
    let Value::Voice(voice) = eval_voice(ECHO) else {
        panic!("expected a voice value");
    };
    // The `release = 0.5` pragma floors the release tail above the 0.01 s
    // envelope so the echo train survives the note end.
    assert!((voice.release_seconds() - 0.5).abs() < 1e-6);

    let mut compiled = voice.to_spec("echo").unwrap().build_voice(48_000.0);
    compiled.prepare();

    // A 2 ms blip; the 0.05 s loop delay repeats it every 2400 frames,
    // decaying by 0.5 per pass.
    let mut frames = Vec::with_capacity(9_600);
    for frame in 0..9_600_u32 {
        let gate = if frame < 96 { 1.0 } else { 0.0 };
        frames.push(compiled.process_frame(gate, 220.0, 1.0, 0.0));
    }
    let energy = |range: std::ops::Range<usize>| {
        frames[range]
            .iter()
            .map(|&(left, _): &(f32, f32)| left.abs())
            .sum::<f32>()
    };

    let dry = energy(0..600);
    let quiet = energy(1_200..2_350);
    let first_echo = energy(2_400..3_100);
    let second_echo = energy(4_800..5_500);
    let third_echo = energy(7_200..7_900);

    assert!(dry > 1.0, "direct blip should be audible, got {dry}");
    assert!(quiet < dry * 0.01, "pre-echo gap should be silent: {quiet}");
    assert!(first_echo > dry * 0.2, "first echo missing: {first_echo}");
    assert!(
        second_echo > dry * 0.05 && second_echo < first_echo,
        "second echo should be quieter: {second_echo} vs {first_echo}"
    );
    assert!(third_echo < second_echo, "echo train must decay");
}

#[test]
fn voice_fan_matches_manually_summed_parallel_branches() {
    let mut fanned = compiled_voice(
        "bank = voice { osc = saw(freq) ; fan(osc, lowpass(500, 0.2), lowpass(3000, 0.2)) * ar(gate, 0.001, 0.05) }",
        "bank",
    );
    let mut piped = compiled_voice(
        "bank = voice { osc = saw(freq) ; mix = osc |> fan(lowpass(500, 0.2), lowpass(3000, 0.2)) ; mix * ar(gate, 0.001, 0.05) }",
        "bank",
    );
    let mut manual = compiled_voice(
        "bank = voice { osc = saw(freq) ; low = osc |> lowpass(500, 0.2) ; high = osc |> lowpass(3000, 0.2) ; (low + high) * ar(gate, 0.001, 0.05) }",
        "bank",
    );

    let mut audible = false;
    for _ in 0..2_048 {
        let (fl, fr) = fanned.process_frame(1.0, 110.0, 0.8, 0.0);
        let (pl, pr) = piped.process_frame(1.0, 110.0, 0.8, 0.0);
        let (ml, mr) = manual.process_frame(1.0, 110.0, 0.8, 0.0);
        assert!((fl - ml).abs() < 1e-6 && (fr - mr).abs() < 1e-6);
        assert!((pl - ml).abs() < 1e-6 && (pr - mr).abs() < 1e-6);
        audible |= fl.abs() > 0.01;
    }
    assert!(audible, "the parallel filter bank should be audible");
}

#[test]
fn voice_poly_pragma_sets_program_pool_size() {
    let Value::Voice(voice) =
        eval_voice("lead = voice { poly = 4 ; sine(freq) * ar(gate, 0.001, 0.05) }")
    else {
        panic!("expected a voice value");
    };
    assert_eq!(voice.to_spec("lead").unwrap().polyphony(), 4);

    // Without the pragma the ADR 0009 default of 8 holds.
    let Value::Voice(voice) = eval_voice(PLUCK) else {
        panic!("expected a voice value");
    };
    assert_eq!(voice.to_spec("pluck").unwrap().polyphony(), 8);
}

#[test]
fn voice_steal_pragma_sets_pool_exhaustion_policy() {
    use orpheus_dsp::StealPolicy;

    let Value::Voice(voice) =
        eval_voice("lead = voice { steal = off ; sine(freq) * ar(gate, 0.001, 0.05) }")
    else {
        panic!("expected a voice value");
    };
    assert_eq!(
        voice.to_spec("lead").unwrap().steal_policy(),
        StealPolicy::Off
    );

    let Value::Voice(voice) =
        eval_voice("lead = voice { steal = oldest ; sine(freq) * ar(gate, 0.001, 0.05) }")
    else {
        panic!("expected a voice value");
    };
    assert_eq!(
        voice.to_spec("lead").unwrap().steal_policy(),
        StealPolicy::Oldest
    );

    // Without the pragma, stealing is on by default (ADR 0009 addendum).
    let Value::Voice(voice) = eval_voice(PLUCK) else {
        panic!("expected a voice value");
    };
    assert_eq!(voice.steal(), None);
    assert_eq!(
        voice.to_spec("pluck").unwrap().steal_policy(),
        StealPolicy::Oldest
    );
}

#[test]
fn voice_steal_pragma_rejects_invalid_values() {
    for source in [
        "bad = voice { steal = 5 ; sine(freq) }",
        "bad = voice { steal = newest ; sine(freq) }",
    ] {
        let message = eval_error(source);
        assert!(
            message.contains("steal") && message.contains("oldest") && message.contains("off"),
            "unexpected error: {message}"
        );
    }

    let message = eval_error("bad = voice { steal = off ; steal = oldest ; sine(freq) }");
    assert!(
        message.contains("steal") && message.contains("twice"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_param_ramp_pragma_sets_the_steal_ramp_window() {
    let Value::Voice(voice) =
        eval_voice("lead = voice { param_ramp = 0.05 ; sine(freq) * ar(gate, 0.001, 0.05) }")
    else {
        panic!("expected a voice value");
    };
    assert_eq!(voice.param_ramp(), Some(0.05));
    let spec = voice.to_spec("lead").unwrap();
    assert!((spec.param_ramp_seconds() - 0.05).abs() < 1e-6);

    // Without the pragma the engine default applies: the 2 ms gain/pan
    // steal-ramp precedent.
    let Value::Voice(voice) = eval_voice(PLUCK) else {
        panic!("expected a voice value");
    };
    assert_eq!(voice.param_ramp(), None);
    let spec = voice.to_spec("pluck").unwrap();
    assert!((spec.param_ramp_seconds() - orpheus_dsp::DEFAULT_PARAM_RAMP_SECONDS).abs() < 1e-9);
}

#[test]
fn voice_param_ramp_pragma_enforces_bounds() {
    for source in [
        "bad = voice { param_ramp = -0.1 ; sine(freq) }",
        "bad = voice { param_ramp = 1.5 ; sine(freq) }",
        "bad = voice { param_ramp = fast ; sine(freq) }",
    ] {
        let message = eval_error(source);
        assert!(
            message.contains("param_ramp") && message.contains('1'),
            "unexpected error: {message}"
        );
    }

    let message = eval_error("bad = voice { param_ramp = 0.1 ; param_ramp = 0.2 ; sine(freq) }");
    assert!(
        message.contains("param_ramp") && message.contains("twice"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_release_pragma_floors_but_never_shortens_the_tail() {
    let Value::Voice(voice) =
        eval_voice("pad = voice { release = 0.25 ; sine(freq) * ar(gate, 0.001, 0.05) }")
    else {
        panic!("expected a voice value");
    };
    assert!((voice.release_seconds() - 0.25).abs() < 1e-6);

    // A longer envelope release still wins over a smaller floor.
    let Value::Voice(voice) =
        eval_voice("pad = voice { release = 0.01 ; sine(freq) * ar(gate, 0.001, 0.3) }")
    else {
        panic!("expected a voice value");
    };
    assert!((voice.release_seconds() - 0.3).abs() < 1e-6);
}

#[test]
fn voice_body_rejects_fb_outside_feedback_loops() {
    let message = eval_error("bad = voice { sine(freq) + fb }");
    assert!(
        message.contains("fb") && message.contains("feedback"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_body_reserves_the_fb_name() {
    let message = eval_error("bad = voice { fb = sine(freq) ; fb }");
    assert!(message.contains("fb"), "unexpected error: {message}");
}

#[test]
fn voice_feedback_requires_a_processing_stage_in_the_loop() {
    let message = eval_error("bad = voice { feedback(gate) }");
    assert!(
        message.contains("feedback") && message.contains("stage"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_feedback_rejects_extra_arguments() {
    let message = eval_error("bad = voice { feedback(sine(freq), 2) }");
    assert!(message.contains("feedback"), "unexpected error: {message}");
}

#[test]
fn voice_fan_requires_at_least_two_branches() {
    let message = eval_error("bad = voice { fan(sine(freq)) }");
    assert!(
        message.contains("fan") && message.contains("branch"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_fan_rejects_non_stage_branches() {
    let message = eval_error("bad = voice { fan(sine(freq), 2, 3) }");
    assert!(message.contains("fan"), "unexpected error: {message}");
}

#[test]
fn voice_fixed_delay_rejects_literal_times_beyond_the_cap() {
    let message = eval_error("bad = voice { sine(freq) |> delay(30) }");
    assert!(message.contains("delay"), "unexpected error: {message}");
}

#[test]
fn voice_delay_accepts_a_signal_time_for_modulation() {
    // A bound LFO signal may drive the delay time (`delay(x, lfo)`): the
    // stage lowers onto the fractional delay line, so the output audibly
    // moves relative to the same patch with a fixed literal time.
    let mut modulated = compiled_voice(
        "flange = voice { lfo = sine(2) * 0.002 + 0.005 ; \
         dry = sine(freq) * ar(gate, 0.001, 0.05) ; \
         wet = dry |> delay(lfo) ; dry + wet }",
        "flange",
    );
    let mut fixed = compiled_voice(
        "flange = voice { dry = sine(freq) * ar(gate, 0.001, 0.05) ; \
         wet = dry |> delay(0.005) ; dry + wet }",
        "flange",
    );

    let mut difference = 0.0_f32;
    let mut energy = 0.0_f32;
    for _ in 0..9_600 {
        let (ml, _) = modulated.process_frame(1.0, 220.0, 1.0, 0.0);
        let (fl, _) = fixed.process_frame(1.0, 220.0, 1.0, 0.0);
        assert!(ml.is_finite(), "modulated voice output must be finite");
        difference += (ml - fl).abs();
        energy += ml.abs();
    }
    assert!(energy > 1.0, "the flanged voice must be audible");
    assert!(
        difference > 1.0,
        "an LFO-driven delay time must move the output away from the fixed \
         delay (total |diff| = {difference})"
    );
}

#[test]
fn voice_poly_pragma_enforces_bounds_and_integrality() {
    for source in [
        "bad = voice { poly = 0 ; sine(freq) }",
        "bad = voice { poly = 100 ; sine(freq) }",
        "bad = voice { poly = 2.5 ; sine(freq) }",
    ] {
        let message = eval_error(source);
        assert!(message.contains("poly"), "unexpected error: {message}");
    }

    let message = eval_error("bad = voice { poly = 2 ; poly = 4 ; sine(freq) }");
    assert!(
        message.contains("poly") && message.contains("twice"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_release_pragma_rejects_invalid_values() {
    let message = eval_error("bad = voice { release = -1 ; sine(freq) }");
    assert!(message.contains("release"), "unexpected error: {message}");

    let message = eval_error("bad = voice { release = 0.1 ; release = 0.2 ; sine(freq) }");
    assert!(
        message.contains("release") && message.contains("twice"),
        "unexpected error: {message}"
    );
}

// ---------------------------------------------------------------------------
// `sample("name")` source stage: one-shot playback of a preloaded
// sample-bank buffer inside a voice body (hybrid sample+synth instruments).
// ---------------------------------------------------------------------------

#[test]
fn voice_sample_stage_plays_the_bank_buffer_exactly() {
    // `eval_module` resolves sample names against the built-in bank
    // ("bd"/"sn"/"cp"/"hh"); the session variant uses its live bank.
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample("bd") }"#) else {
        panic!("expected a voice value");
    };

    let bank = orpheus_dsp::SampleBank::load_builtin();
    let bd = bank.get_by_token("bd").expect("bd is built in");
    // Built-in assets decode at the render rate, so rate 1.0 is
    // sample-exact through the voice's centre-panned output.
    assert_eq!(bd.sample_rate_hz(), 48_000);

    let mut compiled = voice.to_spec("kit").unwrap().build_voice(48_000.0);
    compiled.prepare();
    let center = std::f32::consts::FRAC_1_SQRT_2;
    for (i, &expected) in bd.frames().iter().enumerate() {
        let (left, right) = compiled.process_frame(1.0, 220.0, 1.0, 0.0);
        let want = expected * center;
        assert!(
            (left - want).abs() < 1e-5 && (right - want).abs() < 1e-5,
            "frame {i}: ({left}, {right}) vs expected {want}"
        );
    }
    let (left, right) = compiled.process_frame(1.0, 220.0, 1.0, 0.0);
    assert!(
        left.abs() < 1e-6 && right.abs() < 1e-6,
        "the one-shot must end silent at the buffer end"
    );
}

#[test]
fn voice_sample_stage_is_shaped_by_the_envelope_and_extends_the_release() {
    let Value::Voice(voice) =
        eval_voice(r#"kit = voice { s = sample("bd") ; s * ar(gate, 0.001, 0.2) }"#)
    else {
        panic!("expected a voice value");
    };

    // The release tail covers both the envelope release and the sample's
    // duration at native rate, so short gates never cut the one-shot.
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let bd = bank.get_by_token("bd").expect("bd is built in");
    #[allow(clippy::cast_precision_loss)]
    let bd_seconds = (bd.frames().len() as f32 / bd.sample_rate_hz() as f32).min(30.0);
    assert!(
        voice.release_seconds() + 1e-6 >= bd_seconds && voice.release_seconds() >= 0.2,
        "release tail {} must cover the envelope release (0.2 s) and the \
         sample length ({bd_seconds} s)",
        voice.release_seconds()
    );

    // The envelope multiplies the sample, so every enveloped frame is
    // bounded by the raw playback of the same frame, and the whole take is
    // attenuated (the 1 ms attack is still ramping while the buffer plays).
    let mut enveloped = voice.to_spec("kit").unwrap().build_voice(48_000.0);
    enveloped.prepare();
    let Value::Voice(raw_voice) = eval_voice(r#"kit = voice { sample("bd") }"#) else {
        panic!("expected a voice value");
    };
    let mut raw = raw_voice.to_spec("kit").unwrap().build_voice(48_000.0);
    raw.prepare();

    let mut enveloped_energy = 0.0_f32;
    let mut raw_energy = 0.0_f32;
    for i in 0..bd.frames().len() {
        let (env_left, _) = enveloped.process_frame(1.0, 220.0, 1.0, 0.0);
        let (raw_left, _) = raw.process_frame(1.0, 220.0, 1.0, 0.0);
        assert!(
            env_left.abs() <= raw_left.abs() + 1e-6,
            "frame {i}: the envelope must only attenuate ({env_left} vs {raw_left})"
        );
        enveloped_energy += env_left.abs();
        raw_energy += raw_left.abs();
    }
    assert!(enveloped_energy > 0.0, "the shaped sample must be audible");
    assert!(
        enveloped_energy < raw_energy,
        "the attack ramp must attenuate the take ({enveloped_energy} vs {raw_energy})"
    );
}

#[test]
fn voice_sample_stage_layers_with_synth_stages() {
    // The original goal: hybrid instruments — a sample through the same
    // filter/envelope chain as a synth oscillator.
    let Value::Voice(voice) = eval_voice(
        r#"hybrid = voice { s = sample("bd") ; body = s + sine(freq) * 0.2 ; body |> lowpass(2000, 0.1) |> gain(ar(gate, 0.001, 0.1)) }"#,
    ) else {
        panic!("expected a voice value");
    };
    let mut compiled = voice.to_spec("hybrid").unwrap().build_voice(48_000.0);
    compiled.prepare();
    let mut energy = 0.0_f32;
    for _ in 0..2_400 {
        let (left, right) = compiled.process_frame(1.0, 220.0, 0.8, 0.0);
        assert!(left.is_finite() && right.is_finite());
        energy += left.abs() + right.abs();
    }
    assert!(energy > 1.0, "the hybrid voice should be audible");
}

#[test]
fn voice_sample_stage_accepts_a_rate_signal() {
    let Value::Voice(voice) =
        eval_voice(r#"chip = voice { sample("bd", 2) * ar(gate, 0.001, 0.05) }"#)
    else {
        panic!("expected a voice value");
    };
    assert!(voice.to_spec("chip").is_ok());
}

#[test]
fn voice_sample_stage_rejects_unknown_names_at_definition_time() {
    let message = eval_error(r#"bad = voice { sample("glitch") }"#);
    assert!(
        message.contains("glitch"),
        "the unknown sample name must be reported: {message}"
    );
    assert!(
        message.contains("bd"),
        "the error should list the loaded samples: {message}"
    );
}

#[test]
fn voice_sample_stage_requires_a_string_literal_name() {
    let message = eval_error("bad = voice { sample(freq) }");
    assert!(
        message.contains("sample") && message.contains("literal"),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_sample_stage_rejects_pipes_and_wrong_arity() {
    let message = eval_error(r#"bad = voice { sine(freq) |> sample("bd") }"#);
    assert!(message.contains("sample"), "unexpected error: {message}");

    let message = eval_error(r#"bad = voice { sample("bd", 1, 2) }"#);
    assert!(message.contains("sample"), "unexpected error: {message}");

    let message = eval_error("bad = voice { sample() }");
    assert!(message.contains("sample"), "unexpected error: {message}");
}

#[test]
fn session_plays_sample_voice_from_pattern_token() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    // No envelope: the built-in test assets are only a few frames long, so
    // an attack ramp would drop them below the audibility threshold.
    session
        .eval_line(r#"kit = voice { sample("bd") }"#)
        .unwrap();
    let _ = session.render_test_block_for_tui(1);
    session.eval_line("hits = kit ~ ~ ~").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(
        rendered.iter().any(|sample| sample.abs() > 0.01),
        "a sample-playing voice should be audible from a pattern token"
    );
}

#[test]
fn session_keeps_plain_sample_tokens_unchanged_alongside_sample_voices() {
    // Regression: defining a sample-playing voice must not disturb the
    // engine's ordinary sample-token path.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session
        .eval_line(r#"kit = voice { s = sample("bd") ; s * ar(gate, 0.001, 0.2) }"#)
        .unwrap();
    session.eval_line("drums = bd sn").unwrap();
    let _ = session.render_test_block_for_tui(1);
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
    let rendered = session.render_test_block_for_tui(9_600);
    assert!(
        rendered.iter().any(|sample| sample.abs() > 0.01),
        "sample-bank tokens must keep rendering alongside sample voices"
    );
}

#[test]
fn session_plays_echo_voice_from_pattern_token() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session.eval_line(ECHO).unwrap();
    let _ = session.render_test_block_for_tui(1);
    session.eval_line("melody = echo ~ ~ ~").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(
        rendered.iter().any(|sample| sample.abs() > 0.01),
        "echo voice should be audible from a pattern token"
    );
}

// ---------------------------------------------------------------------------
// SVF and peaking-EQ filter stages (ADR 0010 addendum): `svf_lp`/`svf_hp`/
// `svf_bp`/`svf_notch` expose the TPT state-variable filter (per-sample
// coefficients, so cutoff/Q may be modulated by bound signals) and
// `eq_peak(x, freq, q, gain_db)` exposes the RBJ peaking biquad.
// ---------------------------------------------------------------------------

/// Renders `frames` left-channel samples of a fully gated compiled voice.
fn render_gated_left(source: &str, token: &str, frames: usize) -> Vec<f32> {
    let mut voice = compiled_voice(source, token);
    (0..frames)
        .map(|_| voice.process_frame(1.0, 110.0, 0.8, 0.0).0)
        .collect()
}

/// Mean absolute second difference — a high-frequency-content proxy. The
/// second difference weights a component by frequency squared, so a saw's
/// high harmonics dominate the measure while its fundamental's steady ramp
/// (which dominates a plain first difference) contributes little.
#[allow(clippy::cast_precision_loss)]
fn brightness(samples: &[f32]) -> f32 {
    let curvature: f32 = samples
        .windows(3)
        .map(|w| 2.0f32.mul_add(-w[1], w[2] + w[0]).abs())
        .sum();
    curvature / (samples.len() as f32 - 2.0)
}

#[test]
fn voice_svf_stages_compile_in_all_four_modes() {
    for stage in ["svf_lp", "svf_hp", "svf_bp", "svf_notch"] {
        let source = format!(
            "v = voice {{ f = saw(freq) |> {stage}(1200, 0.7) ; f * ar(gate, 0.001, 0.05) }}"
        );
        let Value::Voice(voice) = eval_voice(&source) else {
            panic!("expected a voice value for stage {stage}");
        };
        assert!(voice.to_spec("v").is_ok(), "{stage} spec must validate");
    }
}

#[test]
fn voice_svf_lowpass_darkens_a_bright_saw() {
    let raw = render_gated_left(
        "v = voice { saw(freq) * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );
    // Direct-call form (no pipe): the input is the first argument.
    let filtered = render_gated_left(
        "v = voice { osc = saw(freq) ; svf_lp(osc, 300, 0.7) * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );

    assert!(filtered.iter().map(|s| s.abs()).sum::<f32>() > 1.0);
    assert!(
        brightness(&filtered) < brightness(&raw) * 0.5,
        "a 300 Hz SVF lowpass must strip the saw's highs: {} vs {}",
        brightness(&filtered),
        brightness(&raw)
    );
}

#[test]
fn voice_eq_peak_boosts_the_centered_band() {
    // The carrier sits exactly on the peaking filter's center frequency, so
    // a +12 dB bell boosts it audibly over the 0 dB (identity) bell.
    let flat = render_gated_left(
        "v = voice { s = sine(freq) |> eq_peak(110, 1, 0) ; s * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );
    let boosted = render_gated_left(
        "v = voice { s = sine(freq) |> eq_peak(110, 1, 12) ; s * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );

    let flat_energy: f32 = flat.iter().map(|s| s.abs()).sum();
    let boosted_energy: f32 = boosted.iter().map(|s| s.abs()).sum();
    assert!(flat_energy > 1.0, "the 0 dB voice must be audible");
    assert!(
        boosted_energy > flat_energy * 2.0,
        "+12 dB at the carrier frequency must boost the band: {boosted_energy} vs {flat_energy}"
    );
}

#[test]
fn voice_eq_shelf_stages_boost_their_side_of_the_corner() {
    // The 110 Hz carrier sits below a 500 Hz low-shelf corner and above a
    // 30 Hz high-shelf corner, so +12 dB on either shelf boosts it audibly
    // over the 0 dB (identity) render.
    let flat = render_gated_left(
        "v = voice { s = sine(freq) |> eq_low_shelf(500, 0.7, 0) ; s * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );
    let flat_energy: f32 = flat.iter().map(|s| s.abs()).sum();
    assert!(flat_energy > 1.0, "the 0 dB voice must be audible");

    let low = render_gated_left(
        "v = voice { s = sine(freq) |> eq_low_shelf(500, 0.7, 12) ; s * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );
    let low_energy: f32 = low.iter().map(|s| s.abs()).sum();
    assert!(
        low_energy > flat_energy * 2.0,
        "+12 dB low shelf above the carrier must boost it: {low_energy} vs {flat_energy}"
    );

    let high = render_gated_left(
        "v = voice { s = sine(freq) |> eq_high_shelf(30, 0.7, 12) ; s * ar(gate, 0.001, 0.05) }",
        "v",
        4_096,
    );
    let high_energy: f32 = high.iter().map(|s| s.abs()).sum();
    assert!(
        high_energy > flat_energy * 2.0,
        "+12 dB high shelf below the carrier must boost it: {high_energy} vs {flat_energy}"
    );
}

#[test]
fn voice_eq_shelf_stages_reject_bad_literal_ranges_and_arity() {
    // Same definition-time range checks as `eq_peak`.
    let message = eval_error("bad = voice { sine(freq) |> eq_low_shelf(800, 1) }");
    assert!(
        message.contains("eq_low_shelf"),
        "unexpected error: {message}"
    );

    let message = eval_error("bad = voice { sine(freq) |> eq_high_shelf(800, 1, 100) }");
    assert!(
        message.contains("eq_high_shelf") && message.contains("gain"),
        "unexpected error: {message}"
    );

    let message = eval_error("bad = voice { sine(freq) |> eq_low_shelf(0, 1, 6) }");
    assert!(
        message.contains("eq_low_shelf"),
        "unexpected error: {message}"
    );

    let message = eval_error("bad = voice { sine(freq) |> eq_high_shelf(800, 500, 6) }");
    assert!(
        message.contains("eq_high_shelf") && message.contains('Q'),
        "unexpected error: {message}"
    );
}

#[test]
fn voice_svf_cutoff_accepts_a_bound_lfo_signal() {
    // Params are signals: an LFO sweeping the cutoff must render finite
    // audio that audibly moves relative to the fixed-cutoff patch.
    let swept = render_gated_left(
        "v = voice { lfo = sine(2) * 400 + 800 ; f = saw(freq) |> svf_lp(lfo, 0.7) ; \
         f * ar(gate, 0.001, 0.05) }",
        "v",
        9_600,
    );
    let fixed = render_gated_left(
        "v = voice { f = saw(freq) |> svf_lp(800, 0.7) ; f * ar(gate, 0.001, 0.05) }",
        "v",
        9_600,
    );

    assert!(
        swept.iter().all(|s| s.is_finite()),
        "LFO-modulated cutoff must never produce NaN/inf"
    );
    assert!(swept.iter().map(|s| s.abs()).sum::<f32>() > 1.0);
    let difference: f32 = swept.iter().zip(&fixed).map(|(a, b)| (a - b).abs()).sum();
    assert!(
        difference > 1.0,
        "a swept cutoff must move the output away from the fixed one ({difference})"
    );
}

#[test]
fn voice_svf_stages_reject_wrong_arity() {
    let message = eval_error("bad = voice { saw(freq) |> svf_lp(800) }");
    assert!(message.contains("svf_lp"), "unexpected error: {message}");

    let message = eval_error("bad = voice { svf_hp(saw(freq), 800, 0.7, 1) }");
    assert!(message.contains("svf_hp"), "unexpected error: {message}");
}

#[test]
fn voice_svf_stages_reject_bad_literal_ranges_at_definition_time() {
    // Cutoff below the filter's 1 Hz floor.
    let message = eval_error("bad = voice { saw(freq) |> svf_lp(0, 0.7) }");
    assert!(
        message.contains("svf_lp") && message.contains("cutoff"),
        "unexpected error: {message}"
    );

    let message = eval_error("bad = voice { saw(freq) |> svf_bp(-100, 0.7) }");
    assert!(message.contains("svf_bp"), "unexpected error: {message}");

    // Q outside the [0.05, 100] clamp bounds.
    let message = eval_error("bad = voice { saw(freq) |> svf_lp(800, 500) }");
    assert!(
        message.contains("svf_lp") && message.contains('Q'),
        "unexpected error: {message}"
    );
    let message = eval_error("bad = voice { saw(freq) |> svf_notch(800, 0.001) }");
    assert!(message.contains("svf_notch"), "unexpected error: {message}");
}

#[test]
fn voice_eq_peak_rejects_bad_literal_ranges_and_arity() {
    let message = eval_error("bad = voice { sine(freq) |> eq_peak(800, 1) }");
    assert!(message.contains("eq_peak"), "unexpected error: {message}");

    // Gain outside the +/-40 dB clamp bounds.
    let message = eval_error("bad = voice { sine(freq) |> eq_peak(800, 1, 100) }");
    assert!(
        message.contains("eq_peak") && message.contains("gain"),
        "unexpected error: {message}"
    );

    let message = eval_error("bad = voice { sine(freq) |> eq_peak(0, 1, 6) }");
    assert!(message.contains("eq_peak"), "unexpected error: {message}");
}

#[test]
fn voice_unknown_stage_error_lists_the_filter_stages() {
    let message = eval_error("bad = voice { warble(freq) }");
    assert!(
        message.contains("svf_lp")
            && message.contains("eq_peak")
            && message.contains("eq_low_shelf"),
        "the available-stage list should include the filter stages: {message}"
    );
}

// ---------------------------------------------------------------------------
// Pattern-side control signals (ADR 0010 addendum): the ambient `p1`..`p4`
// per-note parameters. Patterns set them with the `p1`..`p4` controls
// (`melody |> p1(<200 800>)`); voice bodies read them as signals sampled at
// trigger time and held for the note. Unset parameters read 0.
// ---------------------------------------------------------------------------

const PARAM_ACID: &str = "acid = voice { f = saw(freq) |> svf_lp(p1, 0.7) ; \
                          f * ar(gate, 0.001, 0.05) }";

#[test]
fn voice_body_reads_ambient_pattern_params() {
    // The deferred-item shape: a pattern parameter driving a filter cutoff.
    let value = eval_voice("v = voice { osc = saw(freq) ; osc |> svf_lp(p1, 0.7) }");
    let Value::Voice(voice) = value else {
        panic!("expected a voice value, got {}", value.kind_name());
    };
    assert!(voice.to_spec("v").is_ok());

    // All four parameters resolve, in any signal position.
    let value = eval_voice(
        "v = voice { osc = pulse(freq + p2, 0.5) ; env = ar(gate, 0.001, 0.05) ; \
         osc |> svf_lp(p1 + 100, 0.7) |> drive(p3 + 1) |> gain(p4 + 0.5) |> gain(env) }",
    );
    let Value::Voice(voice) = value else {
        panic!("expected a voice value, got {}", value.kind_name());
    };
    assert!(voice.to_spec("v").is_ok());
}

#[test]
fn voice_body_still_rejects_unknown_ambient_names() {
    let message = eval_error("bad = voice { saw(freq) |> svf_lp(p5, 0.7) }");
    assert!(
        message.contains("p5") && message.contains("p1"),
        "unknown names must still error and hint at the params: {message}"
    );
}

#[test]
fn voice_body_reserves_the_param_names() {
    let message = eval_error("bad = voice { p1 = sine(freq) ; p1 }");
    assert!(
        message.contains("p1") && message.contains("built-in"),
        "unexpected error: {message}"
    );
}

#[test]
#[allow(clippy::float_cmp)] // exact control constants pass through unchanged
fn pattern_controls_stamp_voice_params_per_event() {
    let bindings = eval_module("lead = bd sn |> p1(300 4000) |> p3(7)", ReplMode::Strict).unwrap();
    let pattern = bindings
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .clone();
    let events = pattern.query_unit().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].value.voice_params(), [300.0, 0.0, 7.0, 0.0]);
    assert_eq!(events[1].value.voice_params(), [4000.0, 0.0, 7.0, 0.0]);
}

#[test]
#[allow(clippy::float_cmp)] // the default is exactly zero
fn voice_params_default_to_zero_when_the_pattern_never_sets_them() {
    let bindings = eval_module("lead = bd sn", ReplMode::Strict).unwrap();
    let pattern = bindings
        .get("lead")
        .unwrap()
        .as_sample_pattern()
        .unwrap()
        .clone();
    let events = pattern.query_unit().unwrap();
    for event in &events {
        assert_eq!(event.value.voice_params(), [0.0; 4]);
    }
}

#[test]
fn session_pattern_params_filter_each_note_differently() {
    // End-to-end: the pattern hands a different p1 (SVF cutoff) to each
    // note, so the second note is audibly brighter than the first — the
    // second-difference brightness metric from the filter-stage tests.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session.eval_line(PARAM_ACID).unwrap();
    let _ = session.render_test_block_for_tui(1);
    session
        .eval_line("line = acid acid |> p1(300 6000)")
        .unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    // One cycle = 9600 frames: the 300 Hz note owns [0, 4800), the 6000 Hz
    // note [4800, 9600). The bright window starts after the first note's
    // release tail (2400 frames) has rung out.
    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    let left: Vec<f32> = stereo_frames(&rendered)
        .iter()
        .map(|&(left, _)| left)
        .collect();
    let dark = &left[800..4_600];
    let bright = &left[7_400..9_400];

    assert!(
        dark.iter().map(|s| s.abs()).sum::<f32>() > 1.0,
        "the 300 Hz note must still be audible"
    );
    assert!(
        bright.iter().map(|s| s.abs()).sum::<f32>() > 1.0,
        "the 6000 Hz note must be audible"
    );
    assert!(
        brightness(bright) > brightness(dark) * 2.0,
        "a per-note p1 cutoff must brighten the second note: {} vs {}",
        brightness(bright),
        brightness(dark)
    );
}

#[test]
fn session_pattern_without_params_uses_the_zero_default() {
    // A pattern that never sets p1 leaves the SVF cutoff at the documented
    // default of 0 (clamped to the filter's 1 Hz floor), so the voice renders
    // far darker than the same voice driven with an open cutoff.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session.eval_line(PARAM_ACID).unwrap();
    let _ = session.render_test_block_for_tui(1);
    session.eval_line("line = acid acid").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
    let unset = session.render_test_block_for_tui(9_600);
    assert!(unset.iter().all(|sample| sample.is_finite()));

    let mut driven_session = ReplSession::with_engine(EngineHandle::stub());
    driven_session.eval_line(":tempo 1200").unwrap();
    driven_session.eval_line(PARAM_ACID).unwrap();
    let _ = driven_session.render_test_block_for_tui(1);
    driven_session
        .eval_line("line = acid acid |> p1(6000)")
        .unwrap();
    let _ =
        driven_session.render_test_block_for_tui(driven_session.frames_until_boundary_for_tui());
    let driven = driven_session.render_test_block_for_tui(9_600);

    let energy = |samples: &[f32]| samples.iter().map(|s| s.abs()).sum::<f32>();
    assert!(
        energy(&driven) > 1.0,
        "the open-cutoff voice must be audible"
    );
    assert!(
        energy(&unset) < energy(&driven) * 0.25,
        "an unset p1 must leave the cutoff at its zero default (closed filter): \
         {} vs {}",
        energy(&unset),
        energy(&driven)
    );
}

// ---------------------------------------------------------------------------
// Sample-stage follow-ups (parity roadmap / ADR 0010 addendum):
// `sample_loop("name"[, rate])` wraps at the buffer end instead of stopping,
// `sample_pitched("name"[, reference_hz])` derives the playback rate from the
// note's `freq` ambient (native at the reference, default 220 Hz — the
// engine's rate-1.0 reference frequency), and `sample("name", p1)` lets the
// pattern side set a per-note rate through the #1418 parameters.
// ---------------------------------------------------------------------------

/// Renders `frames` stereo frames of a compiled voice with the gate held
/// high at the given note frequency.
fn render_voice_frames(source: &str, token: &str, freq_hz: f32, frames: usize) -> Vec<(f32, f32)> {
    let mut compiled = compiled_voice(source, token);
    (0..frames)
        .map(|_| compiled.process_frame(1.0, freq_hz, 1.0, 0.0))
        .collect()
}

/// The audible span (first..=last index with |left| above threshold) inside
/// `window`, or `None` when the window is silent.
fn audible_span(window: &[f32], threshold: f32) -> Option<(usize, usize)> {
    let first = window.iter().position(|s| s.abs() > threshold)?;
    let last = window.iter().rposition(|s| s.abs() > threshold)?;
    Some((first, last))
}

#[test]
#[allow(clippy::float_cmp)] // hard-wrap tiling is deliberately bit-exact
fn voice_sample_loop_stage_tiles_the_buffer_bit_exactly() {
    // Hard-wrap looping at rate 1.0 must reproduce the one-shot's frames
    // tiled end to end: frame k of the loop equals frame k % len of the
    // plain `sample()` take, bit for bit.
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    let one_shot = render_voice_frames(r#"kit = voice { sample("bd") }"#, "kit", 220.0, len);
    let looped = render_voice_frames(
        r#"kit = voice { sample_loop("bd") }"#,
        "kit",
        220.0,
        3 * len,
    );

    for (k, &(left, right)) in looped.iter().enumerate() {
        let (want_left, want_right) = one_shot[k % len];
        assert!(
            left == want_left && right == want_right,
            "frame {k}: looped ({left}, {right}) vs tiled one-shot ({want_left}, {want_right})"
        );
    }
}

#[test]
fn voice_sample_loop_keeps_sounding_where_the_one_shot_is_silent() {
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    let one_shot = render_voice_frames(r#"kit = voice { sample("bd") }"#, "kit", 220.0, 8 * len);
    let looped = render_voice_frames(
        r#"kit = voice { sample_loop("bd") }"#,
        "kit",
        220.0,
        8 * len,
    );

    let late_energy = |frames: &[(f32, f32)]| {
        frames[2 * len..]
            .iter()
            .map(|&(left, _)| left.abs())
            .sum::<f32>()
    };
    assert!(
        late_energy(&one_shot) == 0.0,
        "the one-shot must be silent past the buffer end"
    );
    assert!(
        late_energy(&looped) > 0.1,
        "the loop must keep producing past the buffer end"
    );
}

#[test]
fn voice_sample_loop_extends_the_release_and_accepts_a_rate_signal() {
    // Same release convention as the one-shot stage: the tail covers at
    // least the buffer's duration at native rate.
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample_loop("bd") }"#) else {
        panic!("expected a voice value");
    };
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let bd = bank.get_by_token("bd").expect("bd is built in");
    #[allow(clippy::cast_precision_loss)]
    let bd_seconds = (bd.frames().len() as f32 / bd.sample_rate_hz() as f32).min(30.0);
    assert!(voice.release_seconds() + 1e-6 >= bd_seconds);

    // The optional second argument is a rate signal, like `sample`.
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample_loop("bd", 2) }"#) else {
        panic!("expected a voice value");
    };
    assert!(voice.to_spec("kit").is_ok());

    // Same arity/pipe rules as `sample`.
    let message = eval_error(r#"bad = voice { sine(freq) |> sample_loop("bd") }"#);
    assert!(
        message.contains("sample_loop"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop("bd", 1, 2) }"#);
    assert!(
        message.contains("sample_loop"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop("glitch") }"#);
    assert!(message.contains("glitch"), "unexpected error: {message}");
}

#[test]
fn voice_sample_loop_xf_crossfades_the_wrap_and_validates_like_sample_loop() {
    // `sample_loop_xf` is `sample_loop` with a short crossfade at the loop
    // wrap (5 ms of source material, capped at 10% of the buffer): it must
    // keep sounding past the buffer end like the hard-wrap loop, and it must
    // stamp both loop flags onto the node spec. (The built-in bank's buffers
    // are only 8 frames — under one whole fade sample — so the node falls
    // back to the hard wrap here; the wave-level crossfade behavior is
    // covered by the orpheus-dsp `graph_sample_player` tests with synthetic
    // buffers.)
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample_loop_xf("bd") }"#) else {
        panic!("expected a voice value");
    };
    assert!(
        voice.nodes().iter().any(|node| matches!(
            node,
            orpheus_dsp::VoiceNodeSpec::Sample {
                looped: true,
                loop_crossfade: true,
                pitch_reference_hz: None,
                ..
            }
        )),
        "sample_loop_xf must set both loop flags on the sample node"
    );
    assert!(voice.to_spec("kit").is_ok());

    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();
    let crossfaded = render_voice_frames(
        r#"kit = voice { sample_loop_xf("bd") }"#,
        "kit",
        220.0,
        4 * len,
    );
    let late_energy = crossfaded[2 * len..]
        .iter()
        .map(|&(left, _)| left.abs())
        .sum::<f32>();
    assert!(
        late_energy > 0.1,
        "the crossfaded loop must keep producing past the buffer end"
    );
    assert!(
        crossfaded
            .iter()
            .all(|&(l, r)| l.is_finite() && r.is_finite())
    );

    // Same arity/pipe/name rules as `sample_loop`.
    let message = eval_error(r#"bad = voice { sine(freq) |> sample_loop_xf("bd") }"#);
    assert!(
        message.contains("sample_loop_xf"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop_xf("bd", 1, 2) }"#);
    assert!(
        message.contains("sample_loop_xf"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop_xf("glitch") }"#);
    assert!(message.contains("glitch"), "unexpected error: {message}");
}

#[test]
fn voice_sample_loop_pitched_xf_compiles_and_validates_like_its_siblings() {
    // The pitched loop also composes with the crossfade flag.
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample_loop_pitched_xf("bd") }"#) else {
        panic!("expected a voice value");
    };
    assert!(
        voice.nodes().iter().any(|node| matches!(
            node,
            orpheus_dsp::VoiceNodeSpec::Sample {
                looped: true,
                loop_crossfade: true,
                pitch_reference_hz: Some(_),
                ..
            }
        )),
        "sample_loop_pitched_xf must set the loop, crossfade, and pitch flags"
    );
    assert!(voice.to_spec("kit").is_ok());

    let rendered = render_voice_frames(
        r#"kit = voice { sample_loop_pitched_xf("bd", 440) }"#,
        "kit",
        440.0,
        4_096,
    );
    assert!(
        rendered
            .iter()
            .all(|&(l, r)| l.is_finite() && r.is_finite())
    );
    assert!(
        rendered.iter().map(|&(l, _)| l.abs()).sum::<f32>() > 0.1,
        "the pitched crossfaded loop must be audible at its reference"
    );

    let message = eval_error(r#"bad = voice { sample_loop_pitched_xf("bd", 0) }"#);
    assert!(
        message.contains("sample_loop_pitched_xf") && message.contains("reference"),
        "unexpected error: {message}"
    );
}

#[test]
fn session_sample_loop_voice_sounds_through_the_gate_then_stops_after_release() {
    // End-to-end gate-low semantics: the loop keeps playing while the gate
    // holds AND through the voice's release tail, then the note ends —
    // consistent with the one-shot's play-past-the-gate behavior.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session
        .eval_line(r#"kit = voice { sample_loop("bd") }"#)
        .unwrap();
    let _ = session.render_test_block_for_tui(1);
    // One event per cycle quarter: gate spans [0, 2400) frames; the default
    // 0.02 s release tail ends by frame 2400 + 960 = 3360.
    session.eval_line("hits = kit ~ ~ ~").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    let left: Vec<f32> = stereo_frames(&rendered)
        .iter()
        .map(|&(left, _)| left)
        .collect();
    let energy = |range: std::ops::Range<usize>| left[range].iter().map(|s| s.abs()).sum::<f32>();

    // An 8-frame one-shot would be silent long before frame 1000; the loop
    // must still be audible deep into the gate and through the release.
    assert!(
        energy(1_000..2_400) > 0.5,
        "the loop must sound for the whole gate span"
    );
    assert!(
        energy(2_500..3_300) > 0.1,
        "the loop must keep playing until the release ends"
    );
    assert!(
        energy(4_000..9_500) < 1e-3,
        "the note must go silent once the release tail ends"
    );
}

#[test]
fn voice_sample_pitched_at_the_reference_matches_plain_sample_bit_exactly() {
    // freq == reference (default 220 Hz, the engine's rate-1.0 reference)
    // must give playback rate exactly 1.0.
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    let plain = render_voice_frames(r#"kit = voice { sample("bd") }"#, "kit", 220.0, 2 * len);
    let pitched = render_voice_frames(
        r#"kit = voice { sample_pitched("bd") }"#,
        "kit",
        220.0,
        2 * len,
    );
    assert_eq!(
        plain, pitched,
        "at the reference frequency the pitched stage must be bit-identical"
    );
}

#[test]
fn voice_sample_pitched_an_octave_up_matches_double_rate() {
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    // An octave above the reference doubles the playback rate exactly
    // (440 / 220 == 2.0), so it must match the explicit-rate one-shot.
    let doubled = render_voice_frames(r#"kit = voice { sample("bd", 2) }"#, "kit", 220.0, 2 * len);
    let pitched = render_voice_frames(
        r#"kit = voice { sample_pitched("bd") }"#,
        "kit",
        440.0,
        2 * len,
    );
    assert_eq!(
        doubled, pitched,
        "an octave above the reference must double the playback rate"
    );
}

#[test]
fn voice_sample_pitched_accepts_a_custom_reference_literal() {
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    // With the reference moved to 440 Hz, a 440 Hz note plays natively.
    let plain = render_voice_frames(r#"kit = voice { sample("bd") }"#, "kit", 220.0, 2 * len);
    let pitched = render_voice_frames(
        r#"kit = voice { sample_pitched("bd", 440) }"#,
        "kit",
        440.0,
        2 * len,
    );
    assert_eq!(plain, pitched);
}

#[test]
fn voice_sample_pitched_rejects_bad_references_and_arity() {
    // The reference must be a positive finite number literal, resolved at
    // definition time.
    let message = eval_error(r#"bad = voice { sample_pitched("bd", 0) }"#);
    assert!(
        message.contains("sample_pitched") && message.contains("reference"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_pitched("bd", freq) }"#);
    assert!(
        message.contains("sample_pitched") && message.contains("literal"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_pitched("bd", 220, 1) }"#);
    assert!(
        message.contains("sample_pitched"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sine(freq) |> sample_pitched("bd") }"#);
    assert!(
        message.contains("sample_pitched"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_pitched("glitch") }"#);
    assert!(message.contains("glitch"), "unexpected error: {message}");
}

#[test]
fn session_pitched_sample_voice_tracks_pattern_notes() {
    // End-to-end: a pattern of named pitches drives the sample's playback
    // rate. c4 maps to 220 x 2^(60/12) = 7040 Hz under the engine's
    // convention, so a 7040 Hz reference plays c4 natively and c5 (an
    // octave up) at exactly double rate — the take lasts half as long.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session
        .eval_line(r#"kit = voice { sample_pitched("bd", 7040) }"#)
        .unwrap();
    let _ = session.render_test_block_for_tui(1);
    session.eval_line("line = kit kit |> pitch(c4 c5)").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    let left: Vec<f32> = stereo_frames(&rendered)
        .iter()
        .map(|&(left, _)| left)
        .collect();

    // Two events per cycle: c4 owns [0, 4800), c5 owns [4800, 9600).
    let (native_first, native_last) =
        audible_span(&left[..4_800], 1e-3).expect("the c4 note must be audible");
    let (fast_first, fast_last) =
        audible_span(&left[4_800..], 1e-3).expect("the c5 note must be audible");
    let native_len = native_last - native_first + 1;
    let fast_len = fast_last - fast_first + 1;
    assert!(
        native_len >= 2 * fast_len - 1 && native_len <= 2 * fast_len + 1,
        "an octave up must halve the take: native {native_len} frames vs octave-up {fast_len}"
    );
}

#[test]
fn voice_sample_rate_can_be_a_pattern_param() {
    // The #1418 params are ordinary signals, so `sample("bd", p1)` binds the
    // per-note parameter straight into the player's rate input.
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample("bd", p1) }"#) else {
        panic!("expected a voice value");
    };
    assert!(voice.to_spec("kit").is_ok());
}

#[test]
fn session_pattern_param_drives_per_note_sample_rate() {
    // End-to-end: `p1` sets each note's playback rate from the pattern side,
    // so the rate-1 note lasts twice as long as the rate-2 note.
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session
        .eval_line(r#"kit = voice { sample("bd", p1) }"#)
        .unwrap();
    let _ = session.render_test_block_for_tui(1);
    session.eval_line("hits = kit kit |> p1(1 2)").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    let left: Vec<f32> = stereo_frames(&rendered)
        .iter()
        .map(|&(left, _)| left)
        .collect();

    let (slow_first, slow_last) =
        audible_span(&left[..4_800], 1e-3).expect("the rate-1 note must be audible");
    let (fast_first, fast_last) =
        audible_span(&left[4_800..], 1e-3).expect("the rate-2 note must be audible");
    let slow_len = slow_last - slow_first + 1;
    let fast_len = fast_last - fast_first + 1;
    assert!(
        slow_len >= 2 * fast_len - 1 && slow_len <= 2 * fast_len + 1,
        "p1 must set the per-note rate: rate-1 {slow_len} frames vs rate-2 {fast_len}"
    );
}

#[test]
#[allow(clippy::float_cmp)] // hard-wrap tiling is deliberately bit-exact
fn voice_sample_loop_pitched_tiles_at_the_reference_and_wraps_faster_above_it() {
    // The combined stage: loop semantics AND pitch tracking. At the default
    // 220 Hz reference it must tile the one-shot's frames bit-exactly, like
    // `sample_loop`; an octave above the reference the playback rate doubles
    // exactly, so each loop wraps in half the frames — bit-identical to the
    // explicit-rate loop stage AND to the tiled one-shot read at 2x.
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    let one_shot = render_voice_frames(r#"kit = voice { sample("bd") }"#, "kit", 220.0, len);
    let at_reference = render_voice_frames(
        r#"kit = voice { sample_loop_pitched("bd") }"#,
        "kit",
        220.0,
        3 * len,
    );
    for (k, &(left, right)) in at_reference.iter().enumerate() {
        let (want_left, want_right) = one_shot[k % len];
        assert!(
            left == want_left && right == want_right,
            "frame {k}: looped-pitched ({left}, {right}) vs tiled one-shot \
             ({want_left}, {want_right})"
        );
    }

    let doubled = render_voice_frames(r#"kit = voice { sample_loop("bd", 2) }"#, "kit", 220.0, len);
    let octave_up = render_voice_frames(
        r#"kit = voice { sample_loop_pitched("bd") }"#,
        "kit",
        440.0,
        len,
    );
    assert_eq!(
        doubled, octave_up,
        "an octave above the reference must loop at exactly double rate"
    );
    for (k, &(left, _)) in octave_up.iter().enumerate() {
        let (want_left, _) = one_shot[(2 * k) % len];
        assert!(
            left == want_left,
            "frame {k}: at double rate each loop must wrap in half the frames"
        );
    }
}

#[test]
fn voice_sample_loop_pitched_accepts_a_custom_reference_literal() {
    // With the reference re-anchored to 440 Hz, a 440 Hz note loops at the
    // native rate — bit-identical to the plain loop stage at its default.
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let len = bank
        .get_by_token("bd")
        .expect("bd is built in")
        .frames()
        .len();

    let plain_loop = render_voice_frames(
        r#"kit = voice { sample_loop("bd") }"#,
        "kit",
        220.0,
        2 * len,
    );
    let pitched = render_voice_frames(
        r#"kit = voice { sample_loop_pitched("bd", 440) }"#,
        "kit",
        440.0,
        2 * len,
    );
    assert_eq!(plain_loop, pitched);
}

#[test]
fn voice_sample_loop_pitched_extends_release_and_validates_like_its_siblings() {
    // Same release convention as the other sample stages: the tail covers at
    // least the buffer's duration at native rate.
    let Value::Voice(voice) = eval_voice(r#"kit = voice { sample_loop_pitched("bd") }"#) else {
        panic!("expected a voice value");
    };
    let bank = orpheus_dsp::SampleBank::load_builtin();
    let bd = bank.get_by_token("bd").expect("bd is built in");
    #[allow(clippy::cast_precision_loss)]
    let bd_seconds = (bd.frames().len() as f32 / bd.sample_rate_hz() as f32).min(30.0);
    assert!(voice.release_seconds() + 1e-6 >= bd_seconds);
    assert!(voice.to_spec("kit").is_ok());

    // Same validation as `sample_pitched`: positive finite number-literal
    // reference, no pipes, at most one optional argument, known names only.
    let message = eval_error(r#"bad = voice { sample_loop_pitched("bd", 0) }"#);
    assert!(
        message.contains("sample_loop_pitched") && message.contains("reference"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop_pitched("bd", freq) }"#);
    assert!(
        message.contains("sample_loop_pitched") && message.contains("literal"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop_pitched("bd", 220, 1) }"#);
    assert!(
        message.contains("sample_loop_pitched"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sine(freq) |> sample_loop_pitched("bd") }"#);
    assert!(
        message.contains("sample_loop_pitched"),
        "unexpected error: {message}"
    );
    let message = eval_error(r#"bad = voice { sample_loop_pitched("glitch") }"#);
    assert!(message.contains("glitch"), "unexpected error: {message}");
}

#[test]
fn session_plays_svf_filtered_voice_from_pattern_token() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session.eval_line(":tempo 1200").unwrap();
    session
        .eval_line(
            "acid = voice { f = saw(freq) |> svf_lp(900, 0.6) |> eq_peak(500, 1.5, 6) ; \
             f * ar(gate, 0.001, 0.05) }",
        )
        .unwrap();
    let _ = session.render_test_block_for_tui(1);
    session.eval_line("line = acid ~ ~ ~").unwrap();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let rendered = session.render_test_block_for_tui(9_600);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(
        rendered.iter().any(|sample| sample.abs() > 0.01),
        "the SVF-filtered voice should be audible from a pattern token"
    );
}
