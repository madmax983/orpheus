use orpheus_dsp::{EngineCommand, EngineHandle};

#[test]
fn pattern_swap_is_deferred_until_cycle_boundary() {
    let mut engine = EngineHandle::stub();

    engine.enqueue(EngineCommand::SwapPattern("verse".into()));

    assert!(!engine.swap_applied_before_boundary());
}

#[test]
fn pattern_swap_applies_at_cycle_boundary() {
    let mut engine = EngineHandle::stub();

    engine.enqueue(EngineCommand::SwapPattern("verse".into()));
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

    engine.enqueue(EngineCommand::SetTempo(60.0));
    let _ = engine.render_test_block(1);

    assert!(engine.frames_per_cycle_for_test() > before);
}
