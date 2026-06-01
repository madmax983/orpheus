//! Tests for the asynchronous `EngineCommand` messaging system.
//!
//! Verifies thread-safe operations like loading samples, triggering patterns,
//! handling snapshots, and dynamic DSP updates on the engine thread.

use orpheus_dsp::{
    EngineCommand, EngineError, EngineHandle, PatternUpdate, PedalProgram, RoutingSnapshot,
    SampleTrigger, TrackSource, load_builtin_sample_for_test, load_sample_bank_from_directory,
    render_routing_snapshot_to_stereo_for_test,
};
use orpheus_pattern::{Event, Rational, TimeSpan};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn pattern_swap_is_deferred_until_cycle_boundary() {
    let mut engine = EngineHandle::stub();

    engine
        .enqueue(EngineCommand::SwapPattern("verse".into()))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test().saturating_sub(1));

    assert_eq!(engine.active_pattern_name_for_test(), None);
    assert!(!engine.swap_applied_before_boundary());
}

#[test]
fn pattern_swap_applies_at_cycle_boundary() {
    let mut engine = EngineHandle::stub();

    engine
        .enqueue(EngineCommand::SwapPattern("verse".into()))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    assert_eq!(engine.active_pattern_name_for_test(), Some("verse"));
}

#[test]
fn built_in_voice_trigger_renders_non_silent_audio() {
    let mut engine = EngineHandle::stub();

    engine.schedule_test_trigger(0, "bd");
    let rendered = engine.render_test_block(128);

    assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
}

#[test]
fn analog_saw_token_renders_non_silent_audio() {
    let mut engine = EngineHandle::stub();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let pattern = PatternUpdate::new(
        "lead",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("saw"),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let rendered = engine.render_test_block(64);

    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
}

#[test]
fn analog_saw_token_sustains_across_its_event_span() {
    let mut engine = EngineHandle::stub();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let pattern = PatternUpdate::new(
        "lead",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("saw"),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let rendered = engine.render_test_block(64);

    assert!(
        rendered[..16]
            .iter()
            .any(|sample| sample.abs() > f32::EPSILON)
    );
    assert!(
        rendered[80..96]
            .iter()
            .any(|sample| sample.abs() > f32::EPSILON)
    );
}

#[test]
fn sample_trigger_carries_pedal_program() {
    let pedal_program = Arc::new(PedalProgram::new(
        "graph { wet = input |> clip(model=silicon_hard); wet |> output }",
        "signal_kind=Audio\nbinding wet: Audio clip(input, model=silicon_hard)\nresult: Audio output(wet)",
    ));
    let trigger = SampleTrigger::named("bd").with_pedal_program(pedal_program.clone());

    assert!(Arc::ptr_eq(
        trigger
            .pedal_program()
            .expect("sample trigger should expose the pedal program"),
        &pedal_program
    ));
    assert!(
        trigger
            .pedal_program()
            .unwrap()
            .explain()
            .contains("clip(input, model=silicon_hard)")
    );
}

#[test]
fn analog_saw_track_feeds_shared_delay_bus() {
    let mut engine = EngineHandle::stub();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let snapshot = RoutingSnapshot::builder()
        .track_with_source("lead", single_hit_track_source("saw"))
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 8).unwrap(), 0.5, 1.0)
        .send("lead", "dub", 1.0)
        .build()
        .unwrap();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
        .unwrap();

    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let rendered = engine.render_test_block(128);

    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(rendered[120..].iter().any(|sample| sample.abs() > 1.0e-6));
}

#[test]
fn built_in_bd_trigger_prefers_embedded_wav_frames() {
    let mut engine = EngineHandle::stub();
    let sample = load_builtin_sample_for_test("bd").unwrap();

    engine.schedule_test_trigger(0, "bd");
    let rendered = engine.render_test_block(4);
    let total_output_frames =
        u32::try_from(sample.frames.len()).unwrap_or_else(|_| panic!("sample too large for test"));

    let expected = vec![
        sample.frames[0] * edge_envelope(0, total_output_frames),
        sample.frames[0] * edge_envelope(0, total_output_frames),
        sample.frames[1] * edge_envelope(1, total_output_frames),
        sample.frames[1] * edge_envelope(1, total_output_frames),
        sample.frames[2] * edge_envelope(2, total_output_frames),
        sample.frames[2] * edge_envelope(2, total_output_frames),
        sample.frames[3] * edge_envelope(3, total_output_frames),
        sample.frames[3] * edge_envelope(3, total_output_frames),
    ];

    assert_samples_close(&rendered, &expected);
}

#[test]
fn tempo_command_updates_cycle_length() {
    let mut engine = EngineHandle::stub();
    let before = engine.frames_per_cycle_for_test();

    engine.enqueue(EngineCommand::SetTempo(60.0)).unwrap();
    let _ = engine.render_test_block(1);

    assert!(engine.frames_per_cycle_for_test() > before);
}

#[test]
fn split_engine_allows_commands_to_cross_into_renderer() {
    let (mut handle, mut renderer) = EngineHandle::split_for_test();

    handle
        .enqueue(EngineCommand::SwapPattern("verse".into()))
        .unwrap();
    let _ = renderer.render_test_block(renderer.frames_until_boundary_for_test());

    assert_eq!(renderer.active_pattern_name_for_test(), Some("verse"));
}

#[test]
fn engine_preserves_main_track_compatibility_path() {
    let mut engine = EngineHandle::stub();

    engine
        .enqueue(EngineCommand::LoadPattern(single_hit_pattern(
            "drums", "bd",
        )))
        .unwrap();

    let rendered = engine.render_test_block(256);

    assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
}

#[test]
fn engine_mixes_two_tracks_routed_to_master() {
    let mut single = EngineHandle::stub();
    let mut doubled = EngineHandle::stub();
    let directory = temp_directory("routing-two-tracks");
    write_wav(directory.join("pulse.wav"), &[0.1, 0.0, 0.0, 0.0]);
    single
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    doubled
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();

    let single_snapshot = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .route("drums", "master")
        .build()
        .unwrap();
    let doubled_snapshot = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .track_with_source("bass", single_hit_track_source("pulse"))
        .route("drums", "master")
        .route("bass", "master")
        .build()
        .unwrap();

    single
        .enqueue(EngineCommand::SwapRoutingSnapshot(single_snapshot))
        .unwrap();
    doubled
        .enqueue(EngineCommand::SwapRoutingSnapshot(doubled_snapshot))
        .unwrap();

    let _ = single.render_test_block(single.frames_until_boundary_for_test());
    let _ = doubled.render_test_block(doubled.frames_until_boundary_for_test());

    let single_rendered = single.render_test_block(4);
    let doubled_rendered = doubled.render_test_block(4);
    let expected = single_rendered
        .iter()
        .map(|sample| sample * 2.0)
        .collect::<Vec<_>>();

    assert_samples_approx(&doubled_rendered, &expected, 1.0e-6);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn engine_applies_track_send_to_dry_bus() {
    let mut direct = EngineHandle::stub();
    let mut with_send = EngineHandle::stub();
    let directory = temp_directory("routing-dry-send");
    write_wav(directory.join("pulse.wav"), &[0.1, 0.0, 0.0, 0.0]);
    direct
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    with_send
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();

    let direct_snapshot = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .route("drums", "master")
        .build()
        .unwrap();
    let send_snapshot = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .bus("verb")
        .route("drums", "master")
        .send("drums", "verb", 1.0)
        .build()
        .unwrap();

    direct
        .enqueue(EngineCommand::SwapRoutingSnapshot(direct_snapshot))
        .unwrap();
    with_send
        .enqueue(EngineCommand::SwapRoutingSnapshot(send_snapshot))
        .unwrap();

    let _ = direct.render_test_block(direct.frames_until_boundary_for_test());
    let _ = with_send.render_test_block(with_send.frames_until_boundary_for_test());

    let direct_rendered = direct.render_test_block(4);
    let with_send_rendered = with_send.render_test_block(4);
    let expected = direct_rendered
        .iter()
        .map(|sample| sample * 2.0)
        .collect::<Vec<_>>();

    assert_samples_approx(&with_send_rendered, &expected, 1.0e-6);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn engine_renders_shared_delay_bus_repeats() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("routing-shared-delay");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let snapshot = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 8).unwrap(), 0.5, 1.0)
        .send("drums", "dub", 1.0)
        .build()
        .unwrap();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let rendered = engine.render_test_block(64);

    assert!(
        rendered[..60]
            .iter()
            .all(|sample| sample.abs() <= f32::EPSILON)
    );
    assert!((rendered[60] - 0.25).abs() <= 1.0e-6);
    assert!((rendered[61] - 0.25).abs() <= 1.0e-6);
    assert!((rendered[120] - 0.125).abs() <= 1.0e-6);
    assert!((rendered[121] - 0.125).abs() <= 1.0e-6);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn engine_renders_shared_reverb_bus_tail() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("routing-shared-reverb");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let snapshot = RoutingSnapshot::builder()
        .track_with_source("pad", single_hit_track_source("pulse"))
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .send("pad", "verb", 1.0)
        .build()
        .unwrap();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let rendered = engine.render_test_block(512);

    assert!(
        rendered[..128]
            .iter()
            .all(|sample| sample.abs() <= f32::EPSILON)
    );
    assert!(rendered[320..].iter().any(|sample| sample.abs() > 1.0e-6));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn bus_effect_changes_adopt_only_at_cycle_boundary() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("routing-delay-boundary");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let first = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 8).unwrap(), 0.0, 1.0)
        .send("drums", "dub", 1.0)
        .build()
        .unwrap();
    let second = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 16).unwrap(), 0.0, 1.0)
        .send("drums", "dub", 1.0)
        .build()
        .unwrap();

    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(first))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(second))
        .unwrap();

    let before_old_delay = engine.render_test_block(20);
    assert!(
        before_old_delay
            .iter()
            .all(|sample| sample.abs() <= f32::EPSILON)
    );

    let old_delay = engine.render_test_block(11);
    assert!((old_delay[20] - 0.25).abs() <= 1.0e-6);
    assert!((old_delay[21] - 0.25).abs() <= 1.0e-6);

    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let next_cycle = engine.render_test_block(20);
    assert!(
        next_cycle[..30]
            .iter()
            .all(|sample| sample.abs() <= f32::EPSILON)
    );
    assert!((next_cycle[30] - 0.25).abs() <= 1.0e-6);
    assert!((next_cycle[31] - 0.25).abs() <= 1.0e-6);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reverb_effect_changes_adopt_only_at_cycle_boundary() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("routing-reverb-boundary");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let first = RoutingSnapshot::builder()
        .track_with_source("pad", single_hit_track_source("pulse"))
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .send("pad", "verb", 1.0)
        .build()
        .unwrap();
    let second = RoutingSnapshot::builder()
        .track_with_source("pad", single_hit_track_source("pulse"))
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 0.0)
        .send("pad", "verb", 1.0)
        .build()
        .unwrap();

    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(first))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(second))
        .unwrap();

    let old_reverb = engine.render_test_block(engine.frames_until_boundary_for_test());
    assert!(old_reverb.iter().any(|sample| sample.abs() > 1.0e-6));

    let next_cycle = engine.render_test_block(engine.frames_until_boundary_for_test());
    assert!(next_cycle.iter().all(|sample| sample.abs() <= f32::EPSILON));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unchanged_bus_effect_state_survives_unrelated_send_update() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("routing-delay-tail-preserve");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let initial = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 8).unwrap(), 0.5, 1.0)
        .send("drums", "dub", 1.0)
        .build()
        .unwrap();
    let updated_send = RoutingSnapshot::builder()
        .track_with_source("drums", single_hit_track_source("pulse"))
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 8).unwrap(), 0.5, 1.0)
        .send("drums", "dub", 0.5)
        .build()
        .unwrap();

    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(initial))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(updated_send))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let first_frame_next_cycle = engine.render_test_block(1);
    assert!((first_frame_next_cycle[0] - 0.001_953_125).abs() <= 1.0e-6);
    assert!((first_frame_next_cycle[1] - 0.001_953_125).abs() <= 1.0e-6);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unchanged_reverb_state_survives_unrelated_send_update() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("routing-reverb-tail-preserve");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(
            load_sample_bank_from_directory(&directory).unwrap(),
        ))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);

    let initial = RoutingSnapshot::builder()
        .track_with_source("pad", single_hit_track_source("pulse"))
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .send("pad", "verb", 1.0)
        .build()
        .unwrap();
    let updated_send = RoutingSnapshot::builder()
        .track_with_source("pad", single_hit_track_source("pulse"))
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .send("pad", "verb", 0.5)
        .build()
        .unwrap();

    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(initial))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let _ = engine.render_test_block(200);
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(updated_send))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let next_cycle = engine.render_test_block(64);
    assert!(next_cycle.iter().any(|sample| sample.abs() > 1.0e-6));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn routing_snapshot_activates_only_at_cycle_boundary() {
    let mut engine = EngineHandle::stub();

    let first = RoutingSnapshot::builder()
        .track_with_source("kick", single_hit_track_source("bd"))
        .route("kick", "master")
        .build()
        .unwrap();
    let second = RoutingSnapshot::builder()
        .track_with_source("snare", single_hit_track_source("sn"))
        .route("snare", "master")
        .build()
        .unwrap();

    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(first))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    assert_eq!(engine.active_track_names_for_test(), ["kick"]);

    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(second))
        .unwrap();
    let _ = engine.render_test_block(1);

    let snapshot = engine.transport_snapshot();
    assert!(snapshot.has_pending_routing());
    assert_eq!(engine.active_track_names_for_test(), ["kick"]);

    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let snapshot = engine.transport_snapshot();
    assert!(!snapshot.has_pending_routing());
    assert_eq!(engine.active_track_names_for_test(), ["snare"]);
}

#[test]
fn set_reference_frequency_defaults_to_analog_base_hz() {
    let mut engine = EngineHandle::stub();
    assert!(
        (engine.reference_frequency_hz_for_test() - 220.0).abs() < f32::EPSILON,
        "default should be 220 Hz (A3)"
    );
}

#[test]
fn set_reference_frequency_updates_engine_state() {
    let mut engine = EngineHandle::stub();
    engine
        .enqueue(EngineCommand::SetReferenceFrequency(432.0))
        .unwrap();
    let _ = engine.render_test_block(1);
    assert!((engine.reference_frequency_hz_for_test() - 432.0).abs() < f32::EPSILON);

    engine
        .enqueue(EngineCommand::SetReferenceFrequency(220.0))
        .unwrap();
    let _ = engine.render_test_block(1);
    assert!((engine.reference_frequency_hz_for_test() - 220.0).abs() < f32::EPSILON);
}

#[test]
fn set_reference_frequency_rejects_non_positive_values() {
    let (mut handle, mut renderer) = EngineHandle::split_for_test();
    handle
        .enqueue(EngineCommand::SetReferenceFrequency(0.0))
        .unwrap();
    let mut output = [0.0_f32; 2];
    let error = renderer.render_into_interleaved(&mut output).unwrap_err();
    assert!(
        matches!(error, EngineError::InvalidReferenceFrequency),
        "zero reference frequency should surface as InvalidReferenceFrequency, got {error:?}"
    );
}

#[test]
fn enqueue_reports_backpressure_instead_of_panicking() {
    let (mut handle, _renderer) = EngineHandle::split_for_test();

    for _ in 0..64 {
        handle.enqueue(EngineCommand::SetTempo(120.0)).unwrap();
    }

    let error = handle.enqueue(EngineCommand::SetTempo(120.0)).unwrap_err();
    assert!(matches!(error, EngineError::CommandQueueFull));
}

#[test]
fn tiny_positive_tempo_returns_error_instead_of_panicking() {
    let (mut handle, mut renderer) = EngineHandle::split_for_test();

    handle
        .enqueue(EngineCommand::SetTempo(f32::MIN_POSITIVE))
        .unwrap();

    let mut output = [0.0_f32; 2];
    let error = renderer.render_into_interleaved(&mut output).unwrap_err();
    assert!(matches!(error, EngineError::FrameOverflow));
}

#[test]
fn invalid_tempo_does_not_poison_future_transport_snapshots() {
    let (mut handle, mut renderer) = EngineHandle::split_for_test();

    handle
        .enqueue(EngineCommand::SetTempo(f32::MIN_POSITIVE))
        .unwrap();

    let mut output = [0.0_f32; 2];
    let error = renderer.render_into_interleaved(&mut output).unwrap_err();
    assert!(matches!(error, EngineError::FrameOverflow));

    renderer
        .render_into_interleaved(&mut output)
        .unwrap_or_else(|error| panic!("subsequent render should succeed: {error}"));

    assert_eq!(
        handle.transport_snapshot().tempo_bpm().to_bits(),
        120.0_f32.to_bits()
    );
}

#[test]
fn stop_command_rewinds_transport_and_silences_output() {
    let mut engine = EngineHandle::stub();
    let quarter = Rational::new(1, 4).unwrap();
    let pattern = PatternUpdate::new(
        "drums",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("bd"),
        }],
    );

    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();
    let _ = engine.render_test_block(256);
    let _ = engine.render_test_block(engine.frames_per_cycle_for_test() / 2);
    engine.enqueue(EngineCommand::StopTransport).unwrap();

    let rendered = engine.render_test_block(128);
    let snapshot = engine.transport_snapshot();

    assert!(!snapshot.is_playing());
    assert_eq!(snapshot.current_frame(), 0);
    assert_eq!(snapshot.current_cycle_start_frame(), 0);
    assert!(rendered.iter().all(|sample| sample.abs() <= f32::EPSILON));
}

#[test]
fn play_command_restarts_pattern_from_cycle_start_after_stop() {
    let mut engine = EngineHandle::stub();
    let quarter = Rational::new(1, 4).unwrap();
    let pattern = PatternUpdate::new(
        "drums",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("bd"),
        }],
    );

    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();
    let _ = engine.render_test_block(256);
    let _ = engine.render_test_block(engine.frames_per_cycle_for_test() / 2);
    engine.enqueue(EngineCommand::StopTransport).unwrap();
    let _ = engine.render_test_block(64);
    engine.enqueue(EngineCommand::PlayTransport).unwrap();

    let rendered = engine.render_test_block(256);
    let snapshot = engine.transport_snapshot();

    assert!(snapshot.is_playing());
    assert!(snapshot.current_frame() > 0);
    assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
}

#[test]
fn split_handle_equality_is_reflexive() {
    let (handle, _renderer) = EngineHandle::split_for_test();

    assert!(handle == handle);
}

#[test]
fn loaded_pattern_hot_swap_waits_for_the_next_cycle_boundary() {
    let mut engine = EngineHandle::stub();
    let quarter = Rational::new(1, 4).unwrap();
    let intro = PatternUpdate::new(
        "drums",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("bd"),
        }],
    );
    let backbeat = PatternUpdate::new(
        "backbeat",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("sn"),
        }],
    );

    engine.enqueue(EngineCommand::LoadPattern(intro)).unwrap();
    let _ = engine.render_test_block(256);
    engine
        .enqueue(EngineCommand::LoadPattern(backbeat))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test().saturating_sub(1));

    assert_eq!(engine.active_pattern_name_for_test(), Some("drums"));

    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    assert_eq!(engine.active_pattern_name_for_test(), Some("backbeat"));
}

#[test]
fn transport_snapshot_reports_pending_pattern_until_boundary() {
    let mut engine = EngineHandle::stub();
    let quarter = Rational::new(1, 4).unwrap();
    let intro = PatternUpdate::new(
        "drums",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("bd"),
        }],
    );
    let backbeat = PatternUpdate::new(
        "backbeat",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("sn"),
        }],
    );

    engine.enqueue(EngineCommand::LoadPattern(intro)).unwrap();
    let _ = engine.render_test_block(256);

    engine
        .enqueue(EngineCommand::LoadPattern(backbeat))
        .unwrap();
    let _ = engine.render_test_block(1);

    let snapshot = engine.transport_snapshot();
    assert!(snapshot.is_playing());
    assert!(snapshot.has_pending_pattern());
    assert_eq!(engine.active_pattern_name_for_test(), Some("drums"));

    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let snapshot = engine.transport_snapshot();
    assert!(snapshot.is_playing());
    assert!(!snapshot.has_pending_pattern());
    assert_eq!(engine.active_pattern_name_for_test(), Some("backbeat"));
}

#[test]
fn initial_loaded_pattern_is_audible_in_the_first_render_block() {
    let mut engine = EngineHandle::stub();
    let quarter = Rational::new(1, 4).unwrap();
    let pattern = PatternUpdate::new(
        "drums",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: SampleTrigger::named("bd"),
        }],
    );

    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();
    let rendered = engine.render_test_block(256);

    assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
}

#[test]
fn tempo_change_mid_cycle_does_not_strand_future_pattern_swaps() {
    let mut engine = EngineHandle::stub();

    let _ = engine.render_test_block(4_096);
    engine.enqueue(EngineCommand::SetTempo(10_000.0)).unwrap();
    engine
        .enqueue(EngineCommand::SwapPattern("bridge".into()))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test() + 256);

    assert_eq!(engine.active_pattern_name_for_test(), Some("bridge"));
}

#[test]
fn transport_snapshot_tracks_current_cycle_progress() {
    let mut engine = EngineHandle::stub();
    let frames_per_cycle = engine.frames_per_cycle_for_test();

    let snapshot = engine.transport_snapshot();
    assert_eq!(snapshot.current_frame(), 0);
    assert_eq!(snapshot.current_cycle_start_frame(), 0);
    assert_eq!(snapshot.frames_per_cycle(), frames_per_cycle);

    let _ = engine.render_test_block(frames_per_cycle + (frames_per_cycle / 4));

    let snapshot = engine.transport_snapshot();
    assert_eq!(snapshot.frames_per_cycle(), frames_per_cycle);
    assert_eq!(snapshot.current_cycle_start_frame(), frames_per_cycle);
    assert_eq!(
        snapshot.current_frame(),
        frames_per_cycle + (frames_per_cycle / 4)
    );
}

#[test]
fn transport_snapshot_tracks_tempo_changes() {
    let mut engine = EngineHandle::stub();

    assert_eq!(
        engine.transport_snapshot().tempo_bpm().to_bits(),
        120.0_f32.to_bits()
    );

    engine.enqueue(EngineCommand::SetTempo(90.0)).unwrap();
    let _ = engine.render_test_block(1);

    let snapshot = engine.transport_snapshot();
    assert_eq!(snapshot.tempo_bpm().to_bits(), 90.0_f32.to_bits());
    assert_eq!(
        snapshot.frames_per_cycle(),
        engine.frames_per_cycle_for_test()
    );
}

#[test]
fn sample_bank_reload_waits_until_cycle_boundary() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("sample-bank-reload");
    write_wav(directory.join("bd.wav"), &[0.1, 0.0, 0.0, 0.0]);
    let first_bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(first_bank))
        .unwrap();

    let half = Rational::new(1, 2).unwrap();
    let pattern = PatternUpdate::new(
        "drums",
        vec![
            Event {
                whole: None,
                part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                value: SampleTrigger::named("bd"),
            },
            Event {
                whole: None,
                part: TimeSpan::new(half, Rational::new(3, 4).unwrap()).unwrap(),
                value: SampleTrigger::named("bd"),
            },
        ],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let first_trigger = engine.render_test_block(4);
    let first_sample = 0.1 * edge_envelope(0, 4);
    assert!((first_trigger[0] - first_sample).abs() < f32::EPSILON);
    assert!((first_trigger[1] - first_sample).abs() < f32::EPSILON);

    write_wav(directory.join("bd.wav"), &[0.9, 0.0, 0.0, 0.0]);
    let second_bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(second_bank))
        .unwrap();

    let frames_per_cycle = engine.transport_snapshot().frames_per_cycle();
    let frames_until_second_trigger = (frames_per_cycle / 2).saturating_sub(4);
    let _ = engine.render_test_block(frames_until_second_trigger);
    let second_trigger_same_cycle = engine.render_test_block(4);
    assert!((second_trigger_same_cycle[0] - first_sample).abs() < f32::EPSILON);
    assert!((second_trigger_same_cycle[1] - first_sample).abs() < f32::EPSILON);

    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let first_trigger_next_cycle = engine.render_test_block(4);
    let reloaded_sample = 0.9 * edge_envelope(0, 4);
    assert!((first_trigger_next_cycle[0] - reloaded_sample).abs() < f32::EPSILON);
    assert!((first_trigger_next_cycle[1] - reloaded_sample).abs() < f32::EPSILON);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn manifest_mapped_sample_token_plays_through_live_engine() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("manifest-token-live");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.42, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();

    let pattern = PatternUpdate::new(
        "vox",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("vox_ah"),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(4);
    let expected = 0.42 * edge_envelope(0, 4);

    assert!((rendered[0] - expected).abs() < f32::EPSILON);
    assert!((rendered[1] - expected).abs() < f32::EPSILON);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn manifest_region_token_plays_through_live_engine_with_default_rate() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("manifest-region-live");
    fs::write(
        directory.join("samples.ron"),
        concat!(
            "(\n",
            "  tokens: {\n",
            "    \"amen\": \"amen.wav\",\n",
            "  },\n",
            "  regions: {\n",
            "    \"amen_tail\": (\n",
            "      token: \"amen\",\n",
            "      start: 0.5,\n",
            "      end: 1.0,\n",
            "      rate: 0.5,\n",
            "    ),\n",
            "  },\n",
            ")\n"
        ),
    )
    .unwrap();
    write_wav(directory.join("amen.wav"), &[0.2, 0.4, 0.6, 0.8]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();

    let pattern = PatternUpdate::new(
        "amen",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("amen_tail"),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(8);
    let expected = vec![
        0.6 * edge_envelope(0, 4),
        0.6 * edge_envelope(0, 4),
        0.6 * edge_envelope(1, 4),
        0.6 * edge_envelope(1, 4),
        0.8 * edge_envelope(2, 4),
        0.8 * edge_envelope(2, 4),
        0.8 * edge_envelope(3, 4),
        0.8 * edge_envelope(3, 4),
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ];

    assert_samples_close(&rendered, &expected);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn live_engine_applies_sample_gain_rate_and_slice() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("sample-params-live");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.2, 0.4, 0.6, 0.8]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();

    let pattern = PatternUpdate::new(
        "vox",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("vox_ah")
                .with_gain(0.5)
                .with_rate(2.0)
                .with_slice(0.25, 1.0),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(4);
    let expected = vec![
        0.2 * edge_envelope(0, 2),
        0.2 * edge_envelope(0, 2),
        0.4 * edge_envelope(1, 2),
        0.4 * edge_envelope(1, 2),
        0.0,
        0.0,
        0.0,
        0.0,
    ];

    assert_samples_close(&rendered, &expected);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn live_engine_applies_sample_pan_balance() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("sample-pan-live");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.5, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();

    let pattern = PatternUpdate::new(
        "vox",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("vox_ah").with_pan(1.0),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(4);
    let expected = vec![0.0, 0.5 * edge_envelope(0, 4), 0.0, 0.0];
    assert_samples_close(&rendered[..4], &expected);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn live_engine_applies_sample_high_pass_filter() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("sample-hpf-live");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 1.0, 1.0, 1.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();
    let cutoff_hz = 1_200.0;

    let pattern = PatternUpdate::new(
        "vox",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("vox_ah").with_hpf_cutoff_hz(cutoff_hz),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(4);
    let filtered = apply_one_pole_high_pass(&[1.0, 1.0, 1.0, 1.0], cutoff_hz, 48_000);
    let expected = vec![
        filtered[0] * edge_envelope(0, 4),
        filtered[0] * edge_envelope(0, 4),
        filtered[1] * edge_envelope(1, 4),
        filtered[1] * edge_envelope(1, 4),
        filtered[2] * edge_envelope(2, 4),
        filtered[2] * edge_envelope(2, 4),
        filtered[3] * edge_envelope(3, 4),
        filtered[3] * edge_envelope(3, 4),
    ];
    assert_samples_approx(&rendered, &expected, 1.0e-6);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn live_engine_applies_edge_ramps_to_sample_playback() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("sample-ramp-live");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 1.0, 1.0, 1.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();

    let pattern = PatternUpdate::new(
        "vox",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("vox_ah"),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(8);
    let left = [rendered[0], rendered[2], rendered[4], rendered[6]];

    assert!(left[0] > 0.0);
    assert!(left[0] < left[1]);
    assert!((left[1] - left[2]).abs() < f32::EPSILON);
    assert!(left[3] > 0.0);
    assert!(left[3] < left[2]);
    assert!((rendered[0] - rendered[1]).abs() < f32::EPSILON);
    assert!((rendered[6] - rendered[7]).abs() < f32::EPSILON);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn live_engine_supports_negative_rate_reverse_playback() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("sample-reverse-live");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.1, 0.2, 0.3, 0.4]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();

    let pattern = PatternUpdate::new(
        "vox",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: SampleTrigger::named("vox_ah").with_rate(-1.0),
        }],
    );
    engine.enqueue(EngineCommand::LoadPattern(pattern)).unwrap();

    let rendered = engine.render_test_block(4);
    let expected = vec![
        0.4 * edge_envelope(0, 4),
        0.4 * edge_envelope(0, 4),
        0.3 * edge_envelope(1, 4),
        0.3 * edge_envelope(1, 4),
        0.2 * edge_envelope(2, 4),
        0.2 * edge_envelope(2, 4),
        0.1 * edge_envelope(3, 4),
        0.1 * edge_envelope(3, 4),
    ];

    assert_samples_close(&rendered, &expected);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn live_engine_matches_offline_insert_effect_chain() {
    let mut engine = EngineHandle::stub();
    let directory = temp_directory("insert-effects-live-parity");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0; 64]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "vox",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("vox_ah")
                        .with_delay_mix(0.35)
                        .with_delay_time(0.125)
                        .with_delay_feedback(0.25)
                        .with_reverb_mix(0.20)
                        .with_reverb_room(0.80)
                        .with_reverb_damp(0.30)
                        .with_chorus_mix(0.45)
                        .with_chorus_depth(0.60)
                        .with_chorus_rate(0.50)
                        .with_compressor_mix(0.75)
                        .with_compressor_threshold(0.25)
                        .with_compressor_ratio(4.0),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("vox", "master")
        .build()
        .unwrap();

    let offline =
        render_routing_snapshot_to_stereo_for_test(&snapshot, 1, 48_000.0, &bank).unwrap();

    engine
        .enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let live = engine.render_test_block(240);

    assert_eq!(&offline[..live.len()], live.as_slice());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn temp_directory_uses_system_temp_directory() {
    let directory = temp_directory("system-temp-check");

    assert!(directory.starts_with(std::env::temp_dir()));

    fs::remove_dir_all(directory).unwrap();
}

fn single_hit_pattern(name: &str, token: &str) -> PatternUpdate {
    PatternUpdate::new(name, vec![single_hit_event(token)])
}

fn single_hit_track_source(token: &str) -> TrackSource {
    TrackSource::SamplePattern(vec![single_hit_event(token)].into_boxed_slice())
}

fn single_hit_event(token: &str) -> Event<SampleTrigger> {
    Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named(token),
    }
}

fn temp_directory(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("orpheus-dsp-{}-{name}", unique_temp_suffix()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn unique_temp_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{counter}")
}

fn write_wav(path: impl AsRef<Path>, frames: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample in frames {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
}

fn edge_envelope(frame_index: u32, total_frames: u32) -> f32 {
    let ramp_frames = total_frames.div_ceil(2).clamp(1, 32);
    let attack = normalized_edge_gain(frame_index, ramp_frames);
    let release = normalized_edge_gain(
        total_frames.saturating_sub(frame_index.saturating_add(1)),
        ramp_frames,
    );
    attack.min(release)
}

#[allow(clippy::cast_precision_loss)]
fn normalized_edge_gain(distance_from_edge: u32, ramp_frames: u32) -> f32 {
    (((distance_from_edge as f32) + 0.5) / (ramp_frames as f32)).min(1.0)
}

fn assert_samples_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (left, right) in actual.iter().zip(expected) {
        assert!((left - right).abs() < f32::EPSILON);
    }
}

fn assert_samples_approx(actual: &[f32], expected: &[f32], tolerance: f32) {
    assert_eq!(actual.len(), expected.len());
    for (left, right) in actual.iter().zip(expected) {
        assert!((left - right).abs() <= tolerance);
    }
}

#[allow(clippy::cast_possible_truncation)]
fn apply_one_pole_high_pass(input: &[f32], cutoff_hz: f64, sample_rate_hz: u32) -> Vec<f32> {
    let alpha = high_pass_alpha(cutoff_hz, sample_rate_hz);
    let mut previous_input = 0.0_f64;
    let mut previous_output = 0.0_f64;
    input
        .iter()
        .map(|sample| {
            let input = f64::from(*sample);
            let output = alpha * (previous_output + input - previous_input);
            previous_input = input;
            previous_output = output;
            output as f32
        })
        .collect()
}

fn high_pass_alpha(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let cutoff_hz = normalized_cutoff_hz(cutoff_hz, sample_rate_hz);
    let omega = (std::f64::consts::TAU * cutoff_hz) / f64::from(sample_rate_hz);
    1.0 / (1.0 + omega)
}

fn normalized_cutoff_hz(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let nyquist = (f64::from(sample_rate_hz) / 2.0) - 1.0;
    cutoff_hz.clamp(1.0, nyquist.max(1.0))
}
