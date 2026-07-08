//! User-defined graph voice programs (ADR 0009 follow-up, ADR 0010).
//!
//! A `GraphVoiceSpec` is a declarative, engine-independent description of a
//! playable voice built from the graph vocabulary (oscillators, envelopes,
//! filters). Specs compile off-thread into pooled voices; the engine swaps a
//! fully built bank in at the next cycle boundary via
//! `EngineCommand::ReplaceGraphVoicePrograms`.

use orpheus_dsp::{
    EngineCommand, EngineHandle, GraphVoiceBank, GraphVoiceSpec, GraphVoiceSpecError,
    PatternUpdate, SampleTrigger, VoiceNodeSpec, VoiceSignalRef,
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

// ---------------------------------------------------------------------------
// Richer voice vocabulary: feedback taps, fixed delays, n-ary merge, and
// per-program polyphony (ADR 0010 addendum).
// ---------------------------------------------------------------------------

/// A feedback echo: a short sine blip plus a 50 ms feedback delay decaying by
/// 0.5 per repeat. `Feedback(6)` closes the loop by reading node 6's output
/// one sample late (lowered onto the `Rec` combinator).
fn echo_spec(token: &str) -> GraphVoiceSpec {
    GraphVoiceSpec::new(
        token,
        0.3,
        vec![
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Ar {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                release_s: 0.01,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Node(1),
            },
            VoiceNodeSpec::Constant { value: 0.5 },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Feedback(6),
            },
            VoiceNodeSpec::Delay {
                input: VoiceSignalRef::Node(4),
                seconds: 0.05,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(5),
                right: VoiceSignalRef::Node(3),
            },
            VoiceNodeSpec::Add {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(6),
            },
        ],
        VoiceSignalRef::Node(7),
    )
    .expect("echo spec should validate")
}

fn window_energy(samples: &[(f32, f32)], range: std::ops::Range<usize>) -> f32 {
    samples[range]
        .iter()
        .map(|&(left, _)| left.abs())
        .sum::<f32>()
}

#[test]
fn feedback_delay_spec_produces_repeating_decaying_echoes() {
    let spec = echo_spec("echo");
    let mut voice = spec.build_voice(SR);
    voice.prepare();

    // A 2 ms blip, then let the echo train ring out. Delay = 0.05 s = 2400
    // frames at 48 kHz.
    let mut frames = Vec::with_capacity(9_600);
    for frame in 0..9_600_u32 {
        let gate = if frame < 96 { 1.0 } else { 0.0 };
        frames.push(voice.process_frame(gate, 220.0, 1.0, 0.0));
    }

    let dry = window_energy(&frames, 0..600);
    let quiet = window_energy(&frames, 1_200..2_350);
    let first_echo = window_energy(&frames, 2_400..3_100);
    let second_echo = window_energy(&frames, 4_800..5_500);
    let third_echo = window_energy(&frames, 7_200..7_900);

    assert!(dry > 1.0, "direct blip should be audible, got {dry}");
    assert!(
        quiet < dry * 0.01,
        "the gap before the first echo should be near-silent, got {quiet} vs dry {dry}"
    );
    assert!(
        first_echo > dry * 0.2,
        "first echo should repeat the blip, got {first_echo} vs dry {dry}"
    );
    assert!(
        second_echo > dry * 0.05 && second_echo < first_echo,
        "second echo should be audible but quieter, got {second_echo} vs {first_echo}"
    );
    assert!(
        third_echo < second_echo,
        "echo train must decay, got {third_echo} vs {second_echo}"
    );
}

/// The same parallel filter bank expressed once through an n-ary `Merge`
/// (lowered onto the `Mrg` combinator) and once through chained `Add` nodes.
fn filter_bank_nodes() -> Vec<VoiceNodeSpec> {
    vec![
        VoiceNodeSpec::Saw {
            freq: VoiceSignalRef::Freq,
        },
        VoiceNodeSpec::Constant { value: 500.0 },
        VoiceNodeSpec::Constant { value: 0.2 },
        VoiceNodeSpec::Constant { value: 3_000.0 },
        VoiceNodeSpec::Constant { value: 0.2 },
        VoiceNodeSpec::Lowpass {
            input: VoiceSignalRef::Node(0),
            cutoff_hz: VoiceSignalRef::Node(1),
            resonance: VoiceSignalRef::Node(2),
        },
        VoiceNodeSpec::Lowpass {
            input: VoiceSignalRef::Node(0),
            cutoff_hz: VoiceSignalRef::Node(3),
            resonance: VoiceSignalRef::Node(4),
        },
    ]
}

#[test]
fn merge_spec_matches_manually_summed_branches() {
    let mut merged_nodes = filter_bank_nodes();
    merged_nodes.push(VoiceNodeSpec::Merge {
        inputs: vec![VoiceSignalRef::Node(5), VoiceSignalRef::Node(6)],
    });
    let merged = GraphVoiceSpec::new("bank", 0.02, merged_nodes, VoiceSignalRef::Node(7))
        .expect("merge spec should validate");

    let mut summed_nodes = filter_bank_nodes();
    summed_nodes.push(VoiceNodeSpec::Add {
        left: VoiceSignalRef::Node(5),
        right: VoiceSignalRef::Node(6),
    });
    let summed = GraphVoiceSpec::new("bank", 0.02, summed_nodes, VoiceSignalRef::Node(7))
        .expect("summed spec should validate");

    let mut merged_voice = merged.build_voice(SR);
    let mut summed_voice = summed.build_voice(SR);
    merged_voice.prepare();
    summed_voice.prepare();

    let mut audible = false;
    for _ in 0..2_048 {
        let (ml, mr) = merged_voice.process_frame(1.0, 110.0, 0.8, 0.1);
        let (sl, sr) = summed_voice.process_frame(1.0, 110.0, 0.8, 0.1);
        assert!((ml - sl).abs() < 1e-6 && (mr - sr).abs() < 1e-6);
        audible |= ml.abs() > 0.01;
    }
    assert!(audible, "the filter bank should produce audio while gated");
}

#[test]
fn spec_validation_rejects_bad_feedback_merge_and_delay_shapes() {
    // Feedback reference beyond the node list.
    let error = GraphVoiceSpec::new(
        "fb",
        0.02,
        vec![VoiceNodeSpec::Add {
            left: VoiceSignalRef::Gate,
            right: VoiceSignalRef::Feedback(5),
        }],
        VoiceSignalRef::Node(0),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GraphVoiceSpecError::FeedbackOutOfRange {
                node: 0,
                reference: 5
            }
        ),
        "unexpected error: {error}"
    );

    // Self-feedback is a valid loop (one-sample delay).
    assert!(
        GraphVoiceSpec::new(
            "selffb",
            0.02,
            vec![VoiceNodeSpec::Add {
                left: VoiceSignalRef::Gate,
                right: VoiceSignalRef::Feedback(0),
            }],
            VoiceSignalRef::Node(0),
        )
        .is_ok()
    );

    // A merge node must list at least one input.
    let error = GraphVoiceSpec::new(
        "mrg",
        0.02,
        vec![VoiceNodeSpec::Merge { inputs: Vec::new() }],
        VoiceSignalRef::Node(0),
    )
    .unwrap_err();
    assert!(
        matches!(error, GraphVoiceSpecError::EmptyMerge { node: 0 }),
        "unexpected error: {error}"
    );

    // Delay lengths are capped so pool construction stays bounded.
    let error = GraphVoiceSpec::new(
        "slap",
        0.02,
        vec![VoiceNodeSpec::Delay {
            input: VoiceSignalRef::Gate,
            seconds: 60.0,
        }],
        VoiceSignalRef::Node(0),
    )
    .unwrap_err();
    assert!(
        matches!(error, GraphVoiceSpecError::DelayTooLong { node: 0 }),
        "unexpected error: {error}"
    );

    // Negative delays are invalid parameters.
    assert!(
        GraphVoiceSpec::new(
            "neg",
            0.02,
            vec![VoiceNodeSpec::Delay {
                input: VoiceSignalRef::Gate,
                seconds: -0.1,
            }],
            VoiceSignalRef::Node(0),
        )
        .is_err()
    );
}

#[test]
fn spec_polyphony_defaults_and_validates_bounds() {
    let spec = pluck_spec("pluck");
    assert_eq!(spec.polyphony(), 8, "default polyphony follows ADR 0009");

    let spec = pluck_spec("pluck").with_polyphony(4).unwrap();
    assert_eq!(spec.polyphony(), 4);

    assert!(matches!(
        pluck_spec("pluck").with_polyphony(0),
        Err(GraphVoiceSpecError::InvalidPolyphony { requested: 0 })
    ));
    assert!(matches!(
        pluck_spec("pluck").with_polyphony(65),
        Err(GraphVoiceSpecError::InvalidPolyphony { requested: 65 })
    ));
}

#[test]
fn bank_pools_follow_per_program_polyphony() {
    let duo = pluck_spec("duo").with_polyphony(2).unwrap();
    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![duo]);

    let track = orpheus_dsp::TrackId::new(0);
    assert!(bank.trigger("duo", track, 10, 220.0, 0.5, 0.0));
    assert!(bank.trigger("duo", track, 10, 220.0, 0.5, 0.0));
    assert!(
        !bank.trigger("duo", track, 10, 220.0, 0.5, 0.0),
        "a poly-2 program must drop its third simultaneous note"
    );

    // Built-in programs keep the default pool of 8.
    for _ in 0..8 {
        assert!(bank.trigger("gsine", track, 10, 220.0, 0.5, 0.0));
    }
    assert!(!bank.trigger("gsine", track, 10, 220.0, 0.5, 0.0));
}
