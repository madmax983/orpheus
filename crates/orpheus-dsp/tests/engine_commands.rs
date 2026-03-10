use orpheus_dsp::{
    EngineCommand, EngineError, EngineHandle, PatternUpdate, load_builtin_sample_for_test,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

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
fn built_in_bd_trigger_prefers_embedded_wav_frames() {
    let mut engine = EngineHandle::stub();
    let sample = load_builtin_sample_for_test("bd").unwrap();

    engine.schedule_test_trigger(0, "bd");
    let rendered = engine.render_test_block(4);

    let expected = vec![
        sample.frames[0],
        sample.frames[0],
        sample.frames[1],
        sample.frames[1],
        sample.frames[2],
        sample.frames[2],
        sample.frames[3],
        sample.frames[3],
    ];

    assert_eq!(rendered, expected);
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
            value: Box::<str>::from("bd"),
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
            value: Box::<str>::from("bd"),
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
            part: TimeSpan::new(Rational::zero(), quarter.clone()).unwrap(),
            value: Box::<str>::from("bd"),
        }],
    );
    let backbeat = PatternUpdate::new(
        "backbeat",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: Box::<str>::from("sn"),
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
            part: TimeSpan::new(Rational::zero(), quarter.clone()).unwrap(),
            value: Box::<str>::from("bd"),
        }],
    );
    let backbeat = PatternUpdate::new(
        "backbeat",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), quarter).unwrap(),
            value: Box::<str>::from("sn"),
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
            value: Box::<str>::from("bd"),
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
