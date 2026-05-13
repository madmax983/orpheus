use orpheus_dsp::{
    PluginDescriptor, PluginNote, PluginParameterLane, PluginProcessor, PluginTrackSource,
    RoutingSnapshot, SampleBank, TrackSource, render_routing_snapshot_to_stereo_for_test,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

#[test]
fn vst3_descriptor_uses_standard_os_search_paths() {
    let descriptor = PluginDescriptor::vst3("Serum");
    let paths = descriptor.search_paths();

    assert_eq!(descriptor.identifier(), "Serum");
    assert!(
        paths
            .iter()
            .any(|path| path.to_string_lossy().to_ascii_lowercase().contains("vst3")),
        "expected default VST3 search paths, got {paths:?}"
    );
}

#[test]
fn plugin_track_receives_notes_and_routes_stereo_audio_to_master() {
    let note = Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: PluginNote::new(60, 1.0).unwrap(),
    };
    let source = PluginTrackSource::new(PluginDescriptor::vst3("Serum"))
        .with_notes(vec![note].into_boxed_slice());
    let snapshot = RoutingSnapshot::builder()
        .track_with_source("lead", TrackSource::Plugin(source))
        .route("lead", "master")
        .build()
        .unwrap();

    let rendered = render_routing_snapshot_to_stereo_for_test(
        &snapshot,
        1,
        120.0,
        &SampleBank::load_builtin(),
    )
    .unwrap();

    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(
        rendered
            .chunks_exact(2)
            .any(|frame| frame[0].abs() > 0.0001 && frame[1].abs() > 0.0001),
        "hosted plugin track should contribute stereo audio to the master bus"
    );
}

#[test]
fn plugin_processor_processes_frames_without_growing_internal_buffers() {
    let notes = vec![Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap(),
        value: PluginNote::new(72, 0.8).unwrap(),
    }];
    let gain = PluginParameterLane::new(
        "Gain",
        vec![Event {
            whole: None,
            part: TimeSpan::unit(),
            value: 0.25,
        }]
        .into_boxed_slice(),
    )
    .unwrap();
    let source = PluginTrackSource::new(PluginDescriptor::audio_unit("TestSynth"))
        .with_notes(notes.into_boxed_slice())
        .with_parameter_lanes(vec![gain].into_boxed_slice());
    let mut processor = PluginProcessor::new(&source, 48_000);
    processor.begin_cycle();
    let before = processor.buffer_capacities_for_test();

    for frame in 0..512 {
        let _ = processor.process_frame(&source, frame, 24_000);
    }

    assert_eq!(
        processor.buffer_capacities_for_test(),
        before,
        "plugin audio processing must not grow buffers on the render path"
    );
}

#[test]
fn vst3_descriptor_panics_on_empty_identifier() {
    let result = std::panic::catch_unwind(|| {
        let _ = PluginDescriptor::vst3("");
    });
    assert!(result.is_err());
}

#[test]
fn audio_unit_descriptor_panics_on_empty_identifier() {
    let result = std::panic::catch_unwind(|| {
        let _ = PluginDescriptor::audio_unit("   ");
    });
    assert!(result.is_err());
}

#[test]
fn try_new_handles_whitespace_only_identifier() {
    let result = PluginDescriptor::try_new(orpheus_dsp::PluginFormat::Vst3, "   \t\n  ");
    assert_eq!(result, Err(orpheus_dsp::PluginHostError::EmptyIdentifier));
}

#[test]
fn with_parameter_lane_appends_to_lanes() {
    let mut source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"));
    let lane1 = PluginParameterLane::new("Cutoff", vec![].into_boxed_slice()).unwrap();
    let lane2 = PluginParameterLane::new("Resonance", vec![].into_boxed_slice()).unwrap();

    source = source.with_parameter_lane(lane1.clone());
    assert_eq!(source.parameter_lanes().len(), 1);

    source = source.with_parameter_lane(lane2.clone());
    assert_eq!(source.parameter_lanes().len(), 2);
    assert_eq!(source.parameter_lanes()[0].name(), "Cutoff");
    assert_eq!(source.parameter_lanes()[1].name(), "Resonance");
}

#[test]
fn process_frame_handles_missing_parameter_lane_gracefully() {
    let lane = PluginParameterLane::new("Param1", vec![].into_boxed_slice()).unwrap();
    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"))
        .with_parameter_lane(lane.clone());

    let mut processor = PluginProcessor::new(&source, 48_000);

    let lane2 = PluginParameterLane::new("Param2", vec![].into_boxed_slice()).unwrap();
    let source_more_lanes = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"))
        .with_parameter_lane(lane)
        .with_parameter_lane(lane2);

    processor.begin_cycle();
    let _ = processor.process_frame(&source_more_lanes, 0, 24_000);
}

#[test]
fn apply_due_parameter_events_ignores_negative_rational_start() {
    let negative_time =
        TimeSpan::new(Rational::new(-1, 4).unwrap(), Rational::new(1, 4).unwrap()).unwrap();
    let lane = PluginParameterLane::new(
        "Gain",
        vec![Event {
            whole: None,
            part: negative_time,
            value: 0.5,
        }]
        .into_boxed_slice(),
    )
    .unwrap();

    let source =
        PluginTrackSource::new(PluginDescriptor::vst3("TestSynth")).with_parameter_lane(lane);

    let mut processor = PluginProcessor::new(&source, 48_000);
    processor.begin_cycle();

    let _ = processor.process_frame(&source, 0, 24_000);
    assert_eq!(
        processor
            .buffer_capacities_for_test()
            .parameter_value_capacity,
        1
    );
}

#[test]
fn activate_due_notes_ignores_negative_rational_start() {
    let negative_time =
        TimeSpan::new(Rational::new(-1, 4).unwrap(), Rational::new(1, 4).unwrap()).unwrap();
    let notes = vec![Event {
        whole: None,
        part: negative_time,
        value: PluginNote::new(60, 1.0).unwrap(),
    }];

    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"))
        .with_notes(notes.into_boxed_slice());

    let mut processor = PluginProcessor::new(&source, 48_000);
    processor.begin_cycle();

    let _ = processor.process_frame(&source, 0, 24_000);
}

#[test]
fn rational_to_frame_offset_handles_large_numerator() {
    let large_rational = Rational::new(i64::MAX, 1).unwrap();
    let lane = PluginParameterLane::new(
        "Gain",
        vec![Event {
            whole: None,
            part: TimeSpan::new(large_rational, large_rational).unwrap_or_default(),
            value: 0.5,
        }]
        .into_boxed_slice(),
    )
    .unwrap();

    let source =
        PluginTrackSource::new(PluginDescriptor::vst3("TestSynth")).with_parameter_lane(lane);

    let mut processor = PluginProcessor::new(&source, 48_000);
    processor.begin_cycle();
    let _ = processor.process_frame(&source, 0, 48_000);
}

#[test]
fn rational_to_frame_offset_handles_negative_duration_gracefully() {
    let lane = PluginParameterLane::new(
        "Gain",
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::new(-1, 4).unwrap(), Rational::new(1, 4).unwrap())
                .unwrap(),
            value: 0.5,
        }]
        .into_boxed_slice(),
    )
    .unwrap();

    let source =
        PluginTrackSource::new(PluginDescriptor::vst3("TestSynth")).with_parameter_lane(lane);

    let mut processor = PluginProcessor::new(&source, 48_000);
    processor.begin_cycle();
    let _ = processor.process_frame(&source, 0, 48_000);
}

#[test]
fn rational_to_frame_offset_returns_none_on_multiplication_overflow() {
    let huge_rational = Rational::new(i64::MAX, 1).unwrap();
    let lane = PluginParameterLane::new(
        "Gain",
        vec![Event {
            whole: None,
            part: TimeSpan::new(huge_rational, huge_rational).unwrap_or_default(),
            value: 0.5,
        }]
        .into_boxed_slice(),
    )
    .unwrap();

    let source =
        PluginTrackSource::new(PluginDescriptor::vst3("TestSynth")).with_parameter_lane(lane);

    let mut processor = PluginProcessor::new(&source, 48_000);
    processor.begin_cycle();
    let _ = processor.process_frame(&source, 0, 48_000);
}

#[test]
fn default_audio_unit_paths_returns_standard_paths() {
    let descriptor = PluginDescriptor::audio_unit("Serum");
    let paths = descriptor.search_paths();

    assert_eq!(descriptor.identifier(), "Serum");
    if cfg!(target_os = "macos") {
        assert!(
            paths
                .iter()
                .any(|path| path.to_string_lossy().contains("Components")),
            "expected default AudioUnit search paths, got {paths:?}"
        );
    } else {
        assert!(paths.is_empty());
    }
}

#[test]
fn default_vst3_paths_returns_standard_paths() {
    let descriptor = PluginDescriptor::vst3("Serum");
    let paths = descriptor.search_paths();

    assert_eq!(descriptor.identifier(), "Serum");

    assert!(
        paths
            .iter()
            .any(|path| path.to_string_lossy().to_ascii_lowercase().contains("vst3")),
        "expected default VST3 search paths, got {paths:?}"
    );
}

#[test]
fn default_vst3_paths_handles_macos_mock() {
    let _ = PluginDescriptor::vst3("Test");
}
