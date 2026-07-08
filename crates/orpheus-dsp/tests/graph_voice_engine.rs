//! Integration tests wiring graph-based voices into the render engine.
//!
//! The engine exposes graph voice programs through the same token mechanism as
//! built-in synth voices: a pattern event whose token names a graph program
//! (e.g. `gsine`) gates a pooled, pre-allocated graph voice.

use orpheus_dsp::{EngineCommand, EngineHandle, PatternUpdate, SampleTrigger};
use orpheus_pattern::{Event, Rational, TimeSpan};

fn span(start_num: i64, end_num: i64, den: i64) -> TimeSpan {
    TimeSpan::new(
        Rational::new(start_num, den).unwrap(),
        Rational::new(end_num, den).unwrap(),
    )
    .unwrap()
}

fn event(token: &str, part: TimeSpan) -> Event<SampleTrigger> {
    Event {
        whole: None,
        part,
        value: SampleTrigger::named(token),
    }
}

/// Drives the engine exactly like the existing analog-voice tests: set a fast
/// tempo, load a one-event pattern, and render from the cycle boundary.
fn engine_with_pattern(pattern: PatternUpdate) -> EngineHandle {
    let mut engine = EngineHandle::stub();
    engine.enqueue(EngineCommand::SetTempo(1_200.0)).unwrap();
    let _ = engine.render_test_block(1);
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    engine
}

fn stereo_frames(interleaved: &[f32]) -> Vec<(f32, f32)> {
    interleaved
        .chunks_exact(2)
        .map(|frame| (frame[0], frame[1]))
        .collect()
}

#[test]
fn graph_voice_token_renders_stereo_audio_during_gate() {
    // Tempo 1200 BPM -> frames_per_cycle = 9600 at 48 kHz.
    // Event spans 1/8 cycle -> 1200 gate frames.
    let pattern = PatternUpdate::new("graphlead", vec![event("gsine", span(0, 1, 8))]);
    let mut engine = engine_with_pattern(pattern);

    let rendered = engine.render_test_block(9_600);
    let frames = stereo_frames(&rendered);

    assert!(rendered.iter().all(|sample| sample.is_finite()));

    // Audible on BOTH channels while the gate is held (after a short attack).
    let gate_region = &frames[200..1_200];
    assert!(
        gate_region.iter().any(|&(left, _)| left.abs() > 0.01),
        "left channel should be audible during the gate"
    );
    assert!(
        gate_region.iter().any(|&(_, right)| right.abs() > 0.01),
        "right channel should be audible during the gate"
    );
}

#[test]
fn graph_voice_decays_to_near_silence_after_release() {
    let pattern = PatternUpdate::new("graphlead", vec![event("gsine", span(0, 1, 8))]);
    let mut engine = engine_with_pattern(pattern);

    let rendered = engine.render_test_block(9_600);
    let frames = stereo_frames(&rendered);

    // Gate ends at frame 1200; the program's release is well under 2800
    // frames, so the tail region must be near-silent.
    let tail_region = &frames[4_000..9_500];
    assert!(
        tail_region
            .iter()
            .all(|&(left, right)| left.abs() < 1e-3 && right.abs() < 1e-3),
        "graph voice should be near-silent after its release completes"
    );
}

#[test]
fn graph_voice_retriggers_on_subsequent_cycles() {
    let pattern = PatternUpdate::new("graphlead", vec![event("gsine", span(0, 1, 8))]);
    let mut engine = engine_with_pattern(pattern);

    // First cycle plays and fully releases.
    let first = engine.render_test_block(9_600);
    // Second cycle must retrigger from the pooled voices (steady state).
    let second = engine.render_test_block(9_600);
    let second_frames = stereo_frames(&second);

    assert!(first.iter().any(|sample| sample.abs() > 0.01));
    assert!(
        second_frames[200..1_200]
            .iter()
            .any(|&(left, right)| left.abs() > 0.01 || right.abs() > 0.01),
        "graph voice should retrigger on the next cycle"
    );
    assert!(
        second_frames[4_000..9_500]
            .iter()
            .all(|&(left, right)| left.abs() < 1e-3 && right.abs() < 1e-3)
    );
}

#[test]
fn graph_voice_pitch_follows_trigger_rate() {
    // Two engines, identical except for the trigger rate: doubling the rate
    // must raise the pitch (more zero crossings during the gate).
    let base = PatternUpdate::new("graphlead", vec![event("gsine", span(0, 1, 8))]);
    let doubled = PatternUpdate::new(
        "graphlead",
        vec![Event {
            whole: None,
            part: span(0, 1, 8),
            value: SampleTrigger::named("gsine").with_rate(2.0),
        }],
    );

    let mut base_engine = engine_with_pattern(base);
    let mut doubled_engine = engine_with_pattern(doubled);

    let count_crossings = |interleaved: &[f32]| {
        let left: Vec<f32> = stereo_frames(interleaved).iter().map(|&(l, _)| l).collect();
        left[200..1_200]
            .windows(2)
            .filter(|w| w[0].signum() != w[1].signum())
            .count()
    };

    let base_crossings = count_crossings(&base_engine.render_test_block(9_600));
    let doubled_crossings = count_crossings(&doubled_engine.render_test_block(9_600));

    assert!(
        doubled_crossings > base_crossings + base_crossings / 2,
        "rate 2.0 should roughly double zero crossings ({base_crossings} -> {doubled_crossings})"
    );
}

#[test]
fn existing_voice_tokens_are_unaffected_by_graph_voice_support() {
    // The graph voice path must not intercept tokens the sample bank or the
    // built-in synth fallbacks already own: two identically driven engines
    // stay deterministic, and a bd trigger still renders the sample voice.
    let pattern = PatternUpdate::new(
        "drums",
        vec![event("bd", span(0, 1, 4)), event("saw", span(1, 2, 4))],
    );
    let mut first = engine_with_pattern(pattern.clone());
    let mut second = engine_with_pattern(pattern);

    let first_rendered = first.render_test_block(9_600);
    let second_rendered = second.render_test_block(9_600);

    assert!(first_rendered.iter().any(|sample| sample.abs() > 0.01));
    assert_eq!(first_rendered, second_rendered);
}

#[test]
fn unknown_tokens_still_render_silence() {
    let pattern = PatternUpdate::new("ghost", vec![event("nosuchvoice", span(0, 1, 8))]);
    let mut engine = engine_with_pattern(pattern);

    let rendered = engine.render_test_block(9_600);
    assert!(rendered.iter().all(|sample| sample.abs() < f32::EPSILON));
}
