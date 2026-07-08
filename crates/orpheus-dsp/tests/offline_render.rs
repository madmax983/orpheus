#![allow(clippy::needless_range_loop)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use orpheus_dsp::{
    EngineCommand, EngineHandle, GeneratorCycleSpec, GeneratorId, NodeRef, PedalNode, PedalProgram,
    PedalStage, RoutingSnapshot, SampleBank, SampleTrigger, TrackSource,
    load_sample_bank_from_directory, render_events_to_file_with_bank,
    render_routing_snapshot_to_stem_wavs, render_routing_snapshot_to_stereo_for_test,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn offline_render_applies_sample_gain_rate_and_slice() {
    let directory = temp_directory("sample-params-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.2, 0.4, 0.6, 0.8]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah")
            .with_gain(0.5)
            .with_rate(2.0)
            .with_slice(0.25, 1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(4)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = vec![
        pcm16(0.2 * edge_envelope(0, 2)),
        pcm16(0.2 * edge_envelope(0, 2)),
        pcm16(0.4 * edge_envelope(1, 2)),
        pcm16(0.4 * edge_envelope(1, 2)),
    ];

    assert_eq!(samples.len(), 4);
    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_uses_manifest_region_defaults_and_composes_explicit_slice() {
    let directory = temp_directory("sample-region-offline");
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
            "      start: 0.25,\n",
            "      end: 0.75,\n",
            "    ),\n",
            "  },\n",
            ")\n"
        ),
    )
    .unwrap();
    write_wav(
        directory.join("amen.wav"),
        &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
    );
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("amen_tail").with_slice(0.5, 1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(4)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = vec![
        pcm16(0.5 * edge_envelope(0, 2)),
        pcm16(0.5 * edge_envelope(0, 2)),
        pcm16(0.6 * edge_envelope(1, 2)),
        pcm16(0.6 * edge_envelope(1, 2)),
    ];

    assert_eq!(samples.len(), 4);
    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_sample_pan_balance() {
    let directory = temp_directory("sample-pan-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.5, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah").with_pan(-1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(4)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(samples, vec![pcm16(0.5 * edge_envelope(0, 4)), 0, 0, 0]);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_sample_low_pass_filter() {
    let directory = temp_directory("sample-lpf-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();
    let cutoff_hz = 1_200.0;

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah").with_lpf_cutoff_hz(cutoff_hz),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let filtered = apply_one_pole_low_pass(&[1.0, 0.0, 0.0, 0.0], cutoff_hz, 48_000);
    let expected = vec![
        pcm16(filtered[0] * edge_envelope(0, 4)),
        pcm16(filtered[0] * edge_envelope(0, 4)),
        pcm16(filtered[1] * edge_envelope(1, 4)),
        pcm16(filtered[1] * edge_envelope(1, 4)),
        pcm16(filtered[2] * edge_envelope(2, 4)),
        pcm16(filtered[2] * edge_envelope(2, 4)),
        pcm16(filtered[3] * edge_envelope(3, 4)),
        pcm16(filtered[3] * edge_envelope(3, 4)),
    ];

    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_edge_ramps_to_sample_playback() {
    let directory = temp_directory("sample-ramp-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 1.0, 1.0, 1.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah"),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let left = [samples[0], samples[2], samples[4], samples[6]];

    assert_eq!(samples.len(), 8);
    assert!(left[0] > 0);
    assert!(left[0] < left[1]);
    assert_eq!(left[1], left[2]);
    assert!(left[3] > 0);
    assert!(left[3] < left[2]);
    assert_eq!(samples[0], samples[1]);
    assert_eq!(samples[6], samples[7]);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_supports_negative_rate_reverse_playback() {
    let directory = temp_directory("sample-reverse-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.1, 0.2, 0.3, 0.4]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("vox_ah").with_rate(-1.0),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = vec![
        pcm16(0.4 * edge_envelope(0, 4)),
        pcm16(0.4 * edge_envelope(0, 4)),
        pcm16(0.3 * edge_envelope(1, 4)),
        pcm16(0.3 * edge_envelope(1, 4)),
        pcm16(0.2 * edge_envelope(2, 4)),
        pcm16(0.2 * edge_envelope(2, 4)),
        pcm16(0.1 * edge_envelope(3, 4)),
        pcm16(0.1 * edge_envelope(3, 4)),
    ];

    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_uses_detected_transient_slices() {
    let directory = temp_directory("sample-onset-offline");
    write_wav(directory.join("loop.wav"), &transient_loop_frames());
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let path = temp_wav_path();

    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("loop").with_onset(1),
    }];

    render_events_to_file_with_bank(&path, &events, 1, &bank).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(8)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = vec![
        pcm16(0.6 * edge_envelope(0, 100)),
        pcm16(0.6 * edge_envelope(0, 100)),
        pcm16(0.6 * edge_envelope(1, 100)),
        pcm16(0.6 * edge_envelope(1, 100)),
        pcm16(0.6 * edge_envelope(2, 100)),
        pcm16(0.6 * edge_envelope(2, 100)),
        pcm16(0.6 * edge_envelope(3, 100)),
        pcm16(0.6 * edge_envelope(3, 100)),
    ];

    assert_eq!(samples, expected);

    let _ = fs::remove_file(path);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_matches_live_shared_delay_bus() {
    let directory = temp_directory("shared-delay-offline-parity");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "drums",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("pulse"),
                }]
                .into_boxed_slice(),
            ),
        )
        .bus("dub")
        .bus_effect_delay("dub", Rational::new(1, 8).unwrap(), 0.5, 1.0)
        .send("drums", "dub", 1.0)
        .build()
        .unwrap();

    let offline =
        render_routing_snapshot_to_stereo_for_test(&snapshot, 1, 48_000.0, &bank).unwrap();

    let mut live = EngineHandle::stub();
    live.enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();
    live.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = live.render_test_block(1);
    live.enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
        .unwrap();
    let _ = live.render_test_block(live.frames_until_boundary_for_test());
    let live_block = live.render_test_block(64);

    assert_eq!(&offline[..live_block.len()], live_block.as_slice());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn analog_offline_render_renders_non_silent_audio() {
    let events = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("saw"),
    }];
    let path = temp_wav_path();

    render_events_to_file_with_bank(&path, &events, 1, &SampleBank::load_builtin()).unwrap();

    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples = reader
        .samples::<i16>()
        .take(64)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert!(samples.iter().any(|sample| *sample != 0));

    let _ = fs::remove_file(path);
}

#[test]
fn sample_voice_renders_through_pedal_program() {
    let directory = temp_directory("sample-pedal-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[0.8, 0.6, 0.4, 0.2]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();

    let dry_snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "vox",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("vox_ah"),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("vox", "master")
        .build()
        .unwrap();
    let wet_snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "vox",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("vox_ah")
                        .with_pedal_program(level_pedal_program(0.25)),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("vox", "master")
        .build()
        .unwrap();

    let dry =
        render_routing_snapshot_to_stereo_for_test(&dry_snapshot, 1, 48_000.0, &bank).unwrap();
    let wet =
        render_routing_snapshot_to_stereo_for_test(&wet_snapshot, 1, 48_000.0, &bank).unwrap();

    assert!(wet.iter().any(|sample| sample.abs() > 1.0e-4));
    assert_ne!(dry, wet);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn analog_voice_renders_through_pedal_program() {
    let bank = SampleBank::load_builtin();
    let dry_snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "lead",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("saw"),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("lead", "master")
        .build()
        .unwrap();
    let wet_snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "lead",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("saw").with_pedal_program(level_pedal_program(0.1)),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("lead", "master")
        .build()
        .unwrap();

    let dry =
        render_routing_snapshot_to_stereo_for_test(&dry_snapshot, 1, 48_000.0, &bank).unwrap();
    let wet =
        render_routing_snapshot_to_stereo_for_test(&wet_snapshot, 1, 48_000.0, &bank).unwrap();

    assert!(dry.iter().any(|sample| sample.abs() > 1.0e-4));
    assert!(wet.iter().any(|sample| sample.abs() > 1.0e-4));
    assert_ne!(dry, wet);
}

#[test]
fn offline_render_matches_live_shared_reverb_bus() {
    let directory = temp_directory("shared-reverb-offline-parity");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "pad",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("pulse"),
                }]
                .into_boxed_slice(),
            ),
        )
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .send("pad", "verb", 1.0)
        .build()
        .unwrap();

    let offline =
        render_routing_snapshot_to_stereo_for_test(&snapshot, 4, 48_000.0, &bank).unwrap();

    let mut live = EngineHandle::stub();
    live.enqueue(EngineCommand::ReplaceSampleBank(bank))
        .unwrap();
    live.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = live.render_test_block(1);
    live.enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
        .unwrap();
    let _ = live.render_test_block(live.frames_until_boundary_for_test());
    let live_block = live.render_test_block(960);

    assert_eq!(&offline[..live_block.len()], live_block.as_slice());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_insert_delay_tail() {
    let directory = temp_directory("insert-delay-offline");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "lead",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("pulse")
                        .with_delay_mix(1.0)
                        .with_delay_time(0.125)
                        .with_delay_feedback(0.0),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("lead", "master")
        .build()
        .unwrap();

    let rendered =
        render_routing_snapshot_to_stereo_for_test(&snapshot, 1, 48_000.0, &bank).unwrap();

    assert!(
        rendered[..16]
            .iter()
            .all(|sample| sample.abs() <= f32::EPSILON)
    );
    assert!(rendered[60..80].iter().any(|sample| sample.abs() > 1.0e-4));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_applies_insert_reverb_tail() {
    let directory = temp_directory("insert-reverb-offline");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.0, 0.0, 0.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "pad",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("pulse")
                        .with_reverb_mix(1.0)
                        .with_reverb_room(0.9)
                        .with_reverb_damp(0.2),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("pad", "master")
        .build()
        .unwrap();

    let rendered =
        render_routing_snapshot_to_stereo_for_test(&snapshot, 1, 48_000.0, &bank).unwrap();

    assert!(
        rendered[320..420]
            .iter()
            .any(|sample| sample.abs() > 1.0e-5)
    );

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_insert_chorus_creates_stereo_difference() {
    let directory = temp_directory("insert-chorus-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    let sustained = vec![1.0_f32; 64];
    write_wav(directory.join("vox.wav"), &sustained);
    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "vox",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("vox_ah")
                        .with_chorus_mix(1.0)
                        .with_chorus_depth(0.8)
                        .with_chorus_rate(0.6),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("vox", "master")
        .build()
        .unwrap();

    let rendered =
        render_routing_snapshot_to_stereo_for_test(&snapshot, 1, 48_000.0, &bank).unwrap();

    let stereo_diverged = rendered
        .chunks_exact(2)
        .take(96)
        .any(|frame| (frame[0] - frame[1]).abs() > 1.0e-4);
    assert!(stereo_diverged);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn offline_render_insert_compressor_reduces_peak_level() {
    let directory = temp_directory("insert-compressor-offline");
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
    )
    .unwrap();
    write_wav(directory.join("vox.wav"), &[1.0, 1.0, 1.0, 1.0]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();

    let dry_snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "vox",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("vox_ah"),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("vox", "master")
        .build()
        .unwrap();
    let compressed_snapshot = RoutingSnapshot::builder()
        .track_with_source(
            "vox",
            TrackSource::SamplePattern(
                vec![Event {
                    whole: None,
                    part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                    value: SampleTrigger::named("vox_ah")
                        .with_compressor_mix(1.0)
                        .with_compressor_threshold(0.2)
                        .with_compressor_ratio(8.0),
                }]
                .into_boxed_slice(),
            ),
        )
        .route("vox", "master")
        .build()
        .unwrap();

    let dry =
        render_routing_snapshot_to_stereo_for_test(&dry_snapshot, 1, 48_000.0, &bank).unwrap();
    let compressed =
        render_routing_snapshot_to_stereo_for_test(&compressed_snapshot, 1, 48_000.0, &bank)
            .unwrap();

    let dry_peak = dry
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let compressed_peak = compressed
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));

    assert!(compressed_peak < dry_peak);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn stem_export_renders_sample_builtin_synth_and_graph_voice_stems() {
    let directory = temp_directory("stem-generator-sources");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.5, 0.25, 0.125]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();

    let one_shot = |token: &str| {
        TrackSource::SamplePattern(
            vec![Event {
                whole: None,
                part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
                value: SampleTrigger::named(token),
            }]
            .into_boxed_slice(),
        )
    };
    let snapshot = RoutingSnapshot::builder()
        .track_with_source("drums", one_shot("pulse"))
        .track_with_source("lead", one_shot("saw"))
        .track_with_source("pad", one_shot("gsine"))
        .route("drums", "master")
        .route("lead", "master")
        .route("pad", "master")
        .build()
        .unwrap();

    let output_dir = temp_directory("stem-generator-sources-out");
    let written = render_routing_snapshot_to_stem_wavs(
        &snapshot,
        1,
        480.0,
        &bank,
        &[],
        &[],
        &output_dir,
        false,
    )
    .unwrap();

    for stem in ["drums", "lead", "pad"] {
        let stem_path = output_dir.join(format!("{stem}.wav"));
        assert!(
            written.contains(&stem_path),
            "expected `{stem}.wav` in written stems {written:?}"
        );
        let mut reader = hound::WavReader::open(&stem_path).unwrap();
        let samples = reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        // Every trigger fires at the cycle start and spans the first quarter
        // of the cycle, so audio must appear in the first half of the stem.
        let window = &samples[..samples.len() / 2];
        assert!(
            window.iter().any(|sample| *sample != 0),
            "stem `{stem}` should contain nonzero audio in the event window"
        );
    }

    fs::remove_dir_all(output_dir).unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn stem_export_renders_generator_track_from_pre_materialized_cycles() {
    let directory = temp_directory("stem-generator-cycles");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.5, 0.25, 0.125]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();

    let generator_id = GeneratorId::new(0);
    let snapshot = RoutingSnapshot::builder()
        .track_with_source("orca", TrackSource::Generator(generator_id))
        .route("orca", "master")
        .build()
        .unwrap();

    let cycle = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: SampleTrigger::named("pulse"),
    }]
    .into_boxed_slice();
    let generator_cycles = vec![GeneratorCycleSpec {
        generator_id,
        cycles: vec![cycle],
    }];

    let output_dir = temp_directory("stem-generator-cycles-out");
    let written = render_routing_snapshot_to_stem_wavs(
        &snapshot,
        1,
        480.0,
        &bank,
        &[],
        &generator_cycles,
        &output_dir,
        false,
    )
    .unwrap();

    let stem_path = output_dir.join("orca.wav");
    assert!(
        written.contains(&stem_path),
        "expected `orca.wav` in written stems {written:?}"
    );
    let mut reader = hound::WavReader::open(&stem_path).unwrap();
    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let window = &samples[..samples.len() / 2];
    assert!(
        window.iter().any(|sample| *sample != 0),
        "generator stem should contain audio from its pre-materialized cycle"
    );

    fs::remove_dir_all(output_dir).unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn stem_export_renders_generator_track_silent_without_cycles() {
    // Documents the seam / old behavior: with no matching generator spec the
    // generator track produces a silent stem (its buffers live only in the RT
    // engine, never in the snapshot).
    let directory = temp_directory("stem-generator-empty");
    write_wav(directory.join("pulse.wav"), &[1.0, 0.5, 0.25, 0.125]);
    let bank = load_sample_bank_from_directory(&directory).unwrap();

    let snapshot = RoutingSnapshot::builder()
        .track_with_source("orca", TrackSource::Generator(GeneratorId::new(0)))
        .route("orca", "master")
        .build()
        .unwrap();

    let output_dir = temp_directory("stem-generator-empty-out");
    let written = render_routing_snapshot_to_stem_wavs(
        &snapshot,
        1,
        480.0,
        &bank,
        &[],
        &[],
        &output_dir,
        false,
    )
    .unwrap();

    let stem_path = output_dir.join("orca.wav");
    assert!(written.contains(&stem_path));
    let mut reader = hound::WavReader::open(&stem_path).unwrap();
    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        samples.iter().all(|sample| *sample == 0),
        "a generator track with no supplied cycles renders silent"
    );

    fs::remove_dir_all(output_dir).unwrap();
    fs::remove_dir_all(directory).unwrap();
}

fn temp_directory(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("orpheus-dsp-{}-{name}", unique_temp_suffix()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn level_pedal_program(amount: f32) -> Arc<PedalProgram> {
    Arc::new(
        PedalProgram::new("graph { input |> level(amount) |> output }", "level pedal").with_graph(
            vec![
                PedalNode::constant(amount),
                PedalNode::stage(PedalStage::Level {
                    input: NodeRef::Input,
                    amount: NodeRef::node(0),
                }),
            ],
            NodeRef::node(1),
        ),
    )
}

fn temp_wav_path() -> PathBuf {
    std::env::temp_dir().join(format!("orpheus-dsp-render-{}.wav", unique_temp_suffix()))
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

fn transient_loop_frames() -> Vec<f32> {
    let mut frames = vec![0.0_f32; 300];
    for index in 10..14 {
        frames[index] = 1.0;
    }
    for index in 110..114 {
        frames[index] = 0.6;
    }
    for index in 210..214 {
        frames[index] = 0.3;
    }
    frames
}

#[allow(clippy::cast_possible_truncation)]
fn apply_one_pole_low_pass(input: &[f32], cutoff_hz: f64, sample_rate_hz: u32) -> Vec<f32> {
    let alpha = low_pass_alpha(cutoff_hz, sample_rate_hz);
    let mut state = 0.0_f64;
    input
        .iter()
        .map(|sample| {
            state += alpha * (f64::from(*sample) - state);
            state as f32
        })
        .collect()
}

fn low_pass_alpha(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let cutoff_hz = normalized_cutoff_hz(cutoff_hz, sample_rate_hz);
    let omega = (std::f64::consts::TAU * cutoff_hz) / f64::from(sample_rate_hz);
    omega / (1.0 + omega)
}

fn normalized_cutoff_hz(cutoff_hz: f64, sample_rate_hz: u32) -> f64 {
    let nyquist = (f64::from(sample_rate_hz) / 2.0) - 1.0;
    cutoff_hz.clamp(1.0, nyquist.max(1.0))
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

#[allow(clippy::cast_possible_truncation)]
fn pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}
