use orpheus_dsp::{
    EngineCommand, EngineError, EngineHandle, PatternUpdate, SampleTrigger,
    load_builtin_sample_for_test, load_sample_bank_from_directory,
};
use orpheus_pattern::{Event, Rational, TimeSpan};
use std::fs;
use std::path::{Path, PathBuf};
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
fn built_in_bd_trigger_prefers_embedded_wav_frames() {
    let mut engine = EngineHandle::stub();
    let sample = load_builtin_sample_for_test("bd").unwrap();

    engine.schedule_test_trigger(0, "bd");
    let rendered = engine.render_test_block(4);
    let total_output_frames =
        u32::try_from(sample.frames().len()).unwrap_or_else(|_| panic!("sample too large for test"));

    let expected = vec![
        sample.frames()[0] * edge_envelope(0, total_output_frames),
        sample.frames()[0] * edge_envelope(0, total_output_frames),
        sample.frames()[1] * edge_envelope(1, total_output_frames),
        sample.frames()[1] * edge_envelope(1, total_output_frames),
        sample.frames()[2] * edge_envelope(2, total_output_frames),
        sample.frames()[2] * edge_envelope(2, total_output_frames),
        sample.frames()[3] * edge_envelope(3, total_output_frames),
        sample.frames()[3] * edge_envelope(3, total_output_frames),
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
            part: TimeSpan::new(Rational::zero(), quarter.clone()).unwrap(),
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
            part: TimeSpan::new(Rational::zero(), quarter.clone()).unwrap(),
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
fn temp_directory_uses_system_temp_directory() {
    let directory = temp_directory("system-temp-check");

    assert!(directory.starts_with(std::env::temp_dir()));

    fs::remove_dir_all(directory).unwrap();
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
