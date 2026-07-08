//! User-defined graph voice programs (ADR 0009 follow-up, ADR 0010).
//!
//! A `GraphVoiceSpec` is a declarative, engine-independent description of a
//! playable voice built from the graph vocabulary (oscillators, envelopes,
//! filters). Specs compile off-thread into pooled voices; the engine swaps a
//! fully built bank in at the next cycle boundary via
//! `EngineCommand::ReplaceGraphVoicePrograms`.

use orpheus_dsp::{
    EngineCommand, EngineHandle, GraphVoiceBank, GraphVoiceSpec, PatternUpdate, SampleTrigger,
    VoiceNodeSpec, VoiceSignalRef,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

const SR: f32 = 48_000.0;

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

/// A user-style pluck: saw carrier x gate-driven ADSR envelope.
fn pluck_spec(token: &str) -> GraphVoiceSpec {
    GraphVoiceSpec::new(
        token,
        0.05,
        vec![
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Adsr {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                decay_s: 0.02,
                sustain: 0.5,
                release_s: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Node(1),
            },
        ],
        VoiceSignalRef::Node(2),
    )
    .expect("pluck spec should validate")
}

/// A richer spec exercising the filter path: saw -> ladder lowpass, enveloped.
fn filtered_spec(token: &str) -> GraphVoiceSpec {
    GraphVoiceSpec::new(
        token,
        0.03,
        vec![
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Constant { value: 1_200.0 },
            VoiceNodeSpec::Constant { value: 0.3 },
            VoiceNodeSpec::Lowpass {
                input: VoiceSignalRef::Node(0),
                cutoff_hz: VoiceSignalRef::Node(1),
                resonance: VoiceSignalRef::Node(2),
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.03,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(3),
                right: VoiceSignalRef::Node(4),
            },
        ],
        VoiceSignalRef::Node(5),
    )
    .expect("filtered spec should validate")
}

#[test]
fn spec_validation_rejects_bad_shapes() {
    // Empty token.
    assert!(
        GraphVoiceSpec::new(
            "",
            0.02,
            vec![VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq
            }],
            VoiceSignalRef::Node(0),
        )
        .is_err()
    );

    // Whitespace in the pattern token.
    assert!(
        GraphVoiceSpec::new(
            "two words",
            0.02,
            vec![VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq
            }],
            VoiceSignalRef::Node(0),
        )
        .is_err()
    );

    // Empty body.
    assert!(GraphVoiceSpec::new("empty", 0.02, Vec::new(), VoiceSignalRef::Gate).is_err());

    // Forward reference: node 0 references node 0.
    assert!(
        GraphVoiceSpec::new(
            "fwd",
            0.02,
            vec![VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Gate,
            }],
            VoiceSignalRef::Node(0),
        )
        .is_err()
    );

    // Output reference out of range.
    assert!(
        GraphVoiceSpec::new(
            "range",
            0.02,
            vec![VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq
            }],
            VoiceSignalRef::Node(1),
        )
        .is_err()
    );

    // Non-finite parameter.
    assert!(
        GraphVoiceSpec::new(
            "nan",
            f32::NAN,
            vec![VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq
            }],
            VoiceSignalRef::Node(0),
        )
        .is_err()
    );
}

#[test]
fn user_spec_voice_renders_audio_then_decays() {
    let spec = pluck_spec("pluck");
    let mut voice = spec.build_voice(SR);
    voice.prepare();

    let mut gated_energy = 0.0_f32;
    for _ in 0..2_400 {
        let (left, right) = voice.process_frame(1.0, 220.0, 0.5, 0.0);
        gated_energy += left.abs() + right.abs();
    }
    assert!(gated_energy > 1.0, "gated user voice should be audible");

    // Run the release out (0.05 s = 2400 frames), then check near-silence.
    for _ in 0..4_800 {
        let _ = voice.process_frame(0.0, 220.0, 0.5, 0.0);
    }
    let (left, right) = voice.process_frame(0.0, 220.0, 0.5, 0.0);
    assert!(left.abs() < 1e-3 && right.abs() < 1e-3);
}

#[test]
fn bank_hosts_user_programs_alongside_builtins() {
    let bank = GraphVoiceBank::with_user_programs(SR, vec![pluck_spec("pluck")]);
    assert!(bank.has_program("pluck"));
    assert!(bank.has_program("gsine"), "builtins must stay available");
    assert!(!bank.has_program("nosuch"));
}

#[test]
fn user_program_with_same_token_shadows_builtin() {
    let bank = GraphVoiceBank::with_user_programs(SR, vec![pluck_spec("gsine")]);
    assert!(bank.has_program("gsine"));
    assert_eq!(bank.user_specs().len(), 1);
}

#[test]
fn bank_equality_follows_user_specs() {
    let left = GraphVoiceBank::with_user_programs(SR, vec![pluck_spec("pluck")]);
    let right = GraphVoiceBank::with_user_programs(SR, vec![pluck_spec("pluck")]);
    let other = GraphVoiceBank::with_user_programs(SR, vec![filtered_spec("acid")]);
    assert_eq!(left, right);
    assert_ne!(left, other);

    // Clone rebuilds an equivalent bank (off-thread only; it allocates).
    let cloned = left.clone();
    assert_eq!(left, cloned);
}

fn engine_with_programs_and_pattern(
    specs: Vec<GraphVoiceSpec>,
    pattern: PatternUpdate,
) -> EngineHandle {
    let mut engine = EngineHandle::stub();
    let bank = GraphVoiceBank::with_user_programs(engine.sample_rate_hz(), specs);
    engine
        .enqueue(EngineCommand::ReplaceGraphVoicePrograms(bank))
        .unwrap();
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
fn engine_triggers_user_program_from_pattern_token() {
    // Tempo 1200 BPM -> frames_per_cycle = 9600 at 48 kHz; event = 1200 gate frames.
    let pattern = PatternUpdate::new("lead", vec![event("pluck", span(0, 1, 8))]);
    let mut engine =
        engine_with_programs_and_pattern(vec![pluck_spec("pluck"), filtered_spec("acid")], pattern);

    let rendered = engine.render_test_block(9_600);
    let frames = stereo_frames(&rendered);

    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(
        frames[200..1_200]
            .iter()
            .any(|&(left, right)| left.abs() > 0.01 && right.abs() > 0.01),
        "user-defined voice should be audible on both channels during the gate"
    );
    assert!(
        frames[6_000..9_500]
            .iter()
            .all(|&(left, right)| left.abs() < 1e-3 && right.abs() < 1e-3),
        "user-defined voice should decay to near-silence after its release"
    );
}

#[test]
fn engine_keeps_builtin_gsine_after_replacement() {
    let pattern = PatternUpdate::new("lead", vec![event("gsine", span(0, 1, 8))]);
    let mut engine = engine_with_programs_and_pattern(vec![pluck_spec("pluck")], pattern);

    let rendered = engine.render_test_block(9_600);
    assert!(rendered.iter().any(|sample| sample.abs() > 0.01));
}

#[test]
fn engine_ignores_unknown_tokens_after_replacement() {
    let pattern = PatternUpdate::new("ghost", vec![event("nosuchvoice", span(0, 1, 8))]);
    let mut engine = engine_with_programs_and_pattern(vec![pluck_spec("pluck")], pattern);

    let rendered = engine.render_test_block(9_600);
    assert!(rendered.iter().all(|sample| sample.abs() < f32::EPSILON));
}
