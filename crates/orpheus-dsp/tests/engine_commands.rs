use orpheus_dsp::{EngineCommand, EngineError, EngineHandle, PatternUpdate};
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
