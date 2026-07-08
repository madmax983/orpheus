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
