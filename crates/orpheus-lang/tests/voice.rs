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
