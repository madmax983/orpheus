use orpheus_dsp::{
    PluginDescriptor, PluginNote, PluginParameterLane, PluginProcessor, PluginTrackSource,
    RoutingSnapshot, SampleBank, TrackSource, render_routing_snapshot_to_stereo_for_test,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

#[test]
#[serial_test::serial]
fn vst3_descriptor_uses_standard_os_search_paths() {
    let descriptor = PluginDescriptor::vst3("Serum");
    let paths = descriptor.search_paths();

    assert_eq!(descriptor.identifier(), "Serum");
    assert!(
        paths
            .iter()
            .any(|path| path.to_string_lossy().to_lowercase().contains("vst3")),
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
#[serial_test::serial]
fn audio_unit_descriptor_uses_standard_os_search_paths() {
    let descriptor = PluginDescriptor::audio_unit("TestSynth");
    let paths = descriptor.search_paths();

    assert_eq!(descriptor.identifier(), "TestSynth");
    if cfg!(target_os = "macos") {
        assert!(
            paths
                .iter()
                .any(|path| path.to_string_lossy().to_lowercase().contains("components")),
            "expected default Audio Unit search paths on macOS, got {paths:?}"
        );
    } else {
        assert!(
            paths.is_empty(),
            "expected no Audio Unit search paths on non-macOS, got {paths:?}"
        );
    }
}



#[test]
#[serial_test::serial]
fn default_vst3_paths_handles_windows() {
    if cfg!(target_os = "windows") {
        let descriptor = PluginDescriptor::vst3("TestSynth");
        let paths = descriptor.search_paths();
        assert!(paths.iter().any(|p| {
            p.to_string_lossy()
                .contains(r"C:\Program Files\Common Files\VST3")
        }));
    }
}

#[test]
#[serial_test::serial]
fn default_vst3_paths_handles_macos() {
    if cfg!(target_os = "macos") {
        let descriptor = PluginDescriptor::vst3("TestSynth");
        let paths = descriptor.search_paths();
        assert!(
            paths
                .iter()
                .any(|p| p.to_string_lossy().contains("/Library/Audio/Plug-Ins/VST3"))
        );
    }
}

#[test]
#[serial_test::serial]
fn default_vst3_paths_handles_linux() {
    if !cfg!(target_os = "windows") && !cfg!(target_os = "macos") {
        let descriptor = PluginDescriptor::vst3("TestSynth");
        let paths = descriptor.search_paths();
        assert!(
            paths
                .iter()
                .any(|p| p.to_string_lossy().contains("/usr/lib/vst3"))
        );
    }
}

#[test]
fn plugin_descriptor_try_new_rejects_empty_identifier() {
    use orpheus_dsp::PluginFormat;
    let result = PluginDescriptor::try_new(PluginFormat::Vst3, "");
    assert!(result.is_err());
    let result = PluginDescriptor::try_new(PluginFormat::AudioUnit, "   ");
    assert!(result.is_err());
}

#[test]
fn plugin_note_rejects_invalid_velocity() {
    let result = PluginNote::new(60, 1.5);
    assert!(result.is_err());
    let result = PluginNote::new(60, -0.1);
    assert!(result.is_err());
    let result = PluginNote::new(60, f32::NAN);
    assert!(result.is_err());
}

#[test]
fn plugin_note_accessors() {
    let note = PluginNote::new(60, 0.5).unwrap();
    assert_eq!(note.note_number(), 60);
    assert_eq!(note.velocity(), 0.5);
    assert_eq!(note.channel(), 0);
}

#[test]
fn plugin_parameter_lane_rejects_empty_name() {
    let result = PluginParameterLane::new("", vec![].into_boxed_slice());
    assert!(result.is_err());
}

#[test]
fn plugin_parameter_lane_rejects_invalid_values() {
    let result = PluginParameterLane::new(
        "Gain",
        vec![Event {
            whole: None,
            part: TimeSpan::unit(),
            value: 1.5,
        }]
        .into_boxed_slice(),
    );
    assert!(result.is_err());
}

#[test]
fn plugin_parameter_lane_accessors() {
    let lane = PluginParameterLane::new("Gain", vec![].into_boxed_slice()).unwrap();
    assert_eq!(lane.name(), "Gain");
    assert_eq!(lane.events().len(), 0);
}

#[test]
fn plugin_processor_drops_completed_voices() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth")).with_notes(
        vec![Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 48000).unwrap()).unwrap(), // Note lasting 1 sample frame
            value: PluginNote::new(60, 1.0).unwrap(),
        }]
        .into_boxed_slice(),
    );

    let mut processor = PluginProcessor::new(&source, 48_000);
    let _ = processor.process_frame(&source, 0, 48_000);
    let _ = processor.process_frame(&source, 1, 48_000);
}

#[test]
fn plugin_processor_drops_parameter_events_for_unmapped_lanes() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"));
    let mut processor = PluginProcessor::new(&source, 48_000);

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

    let source_with_lane = source.with_parameter_lane(gain);
    for frame in 0..512 {
        let _ = processor.process_frame(&source_with_lane, frame, 24_000);
    }
}

#[test]
fn plugin_processor_ignores_notes_with_negative_rational_start() {
    let bad_event = Event {
        whole: None,
        part: TimeSpan::new(Rational::new(-1, 4).unwrap(), Rational::new(1, 4).unwrap()).unwrap(),
        value: PluginNote::new(60, 1.0).unwrap(),
    };

    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"))
        .with_notes(vec![bad_event].into_boxed_slice());

    let mut processor = PluginProcessor::new(&source, 48_000);
    for frame in 0..512 {
        let _ = processor.process_frame(&source, frame, 24_000);
    }
}

#[test]
fn plugin_processor_ignores_out_of_bounds_gain_lane() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"));

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

    let mut processor = PluginProcessor::new(&source, 48_000);
    let source_with_lane = source.with_parameter_lane(gain);
    for frame in 0..512 {
        let _ = processor.process_frame(&source_with_lane, frame, 24_000);
    }
}

#[test]
fn plugin_processor_ignores_out_of_bounds_parameter_lane() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"));

    let cutoff = PluginParameterLane::new(
        "Cutoff",
        vec![Event {
            whole: None,
            part: TimeSpan::unit(),
            value: 0.25,
        }]
        .into_boxed_slice(),
    )
    .unwrap();

    let mut processor = PluginProcessor::new(&source, 48_000);
    let source_with_lane = source.with_parameter_lane(cutoff);
    for frame in 0..512 {
        let _ = processor.process_frame(&source_with_lane, frame, 24_000);
    }
}

#[test]
fn plugin_processor_ignores_parameter_events_with_negative_rational_start() {
    let bad_event = Event {
        whole: None,
        part: TimeSpan::new(Rational::new(-1, 4).unwrap(), Rational::new(1, 4).unwrap()).unwrap(),
        value: 0.25,
    };

    let gain = PluginParameterLane::new("Gain", vec![bad_event].into_boxed_slice()).unwrap();

    let source =
        PluginTrackSource::new(PluginDescriptor::vst3("TestSynth")).with_parameter_lane(gain);

    let mut processor = PluginProcessor::new(&source, 48_000);
    for frame in 0..512 {
        let _ = processor.process_frame(&source, frame, 24_000);
    }
}

#[test]
fn note_duration_frames_caps_at_u32_max() {
    let long_event = Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(100_000_000, 1).unwrap()).unwrap(),
        value: PluginNote::new(60, 1.0).unwrap(),
    };

    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"))
        .with_notes(vec![long_event].into_boxed_slice());

    let mut processor = PluginProcessor::new(&source, 48_000);
    let _ = processor.process_frame(&source, 0, 480_000_000);
}

#[test]
fn plugin_processor_processes_multiple_parameter_events_in_same_frame() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("TestSynth"));

    let gain = PluginParameterLane::new(
        "Gain",
        vec![
            Event {
                whole: None,
                part: TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap(),
                value: 0.25,
            },
            Event {
                // Same frame
                whole: None,
                part: TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap(),
                value: 0.75,
            },
        ]
        .into_boxed_slice(),
    )
    .unwrap();

    let source_with_lane = source.with_parameter_lane(gain);
    let mut processor = PluginProcessor::new(&source_with_lane, 48_000);

    for frame in 0..512 {
        let _ = processor.process_frame(&source_with_lane, frame, 24_000);
    }
}

#[test]
fn plugin_processor_debug_derive() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("Test"));
    let processor = PluginProcessor::new(&source, 48000);
    let debug_str = format!("{:?}", processor);
    assert!(debug_str.contains("PluginProcessor"));
}

#[test]
fn plugin_voice_debug_derive() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("Test")).with_notes(
        vec![Event {
            whole: None,
            part: TimeSpan::unit(),
            value: PluginNote::new(60, 1.0).unwrap(),
        }]
        .into_boxed_slice(),
    );
    let mut processor = PluginProcessor::new(&source, 48000);
    let _ = processor.process_frame(&source, 0, 48000);
    let debug_str = format!("{:?}", processor);
    assert!(debug_str.contains("PluginVoice"));
}

#[test]
fn plugin_track_source_derived_traits() {
    let source = PluginTrackSource::new(PluginDescriptor::vst3("Test"));
    let source2 = source.clone();
    assert_eq!(source, source2);
    let debug_str = format!("{:?}", source);
    assert!(debug_str.contains("PluginTrackSource"));
}

#[test]
fn plugin_parameter_lane_derived_traits() {
    let lane = PluginParameterLane::new("Test", vec![].into_boxed_slice()).unwrap();
    let lane2 = lane.clone();
    assert_eq!(lane, lane2);
    let debug_str = format!("{:?}", lane);
    assert!(debug_str.contains("PluginParameterLane"));
}

#[test]
fn plugin_note_derived_traits() {
    let note = PluginNote::new(60, 0.5).unwrap();
    let note2 = note.clone();
    assert_eq!(note, note2);
    let debug_str = format!("{:?}", note);
    assert!(debug_str.contains("PluginNote"));
}

#[test]
fn plugin_host_error_derived_traits() {
    use orpheus_dsp::PluginHostError;
    let err = PluginHostError::EmptyIdentifier;
    let err2 = err.clone();
    assert_eq!(err, err2);
}

#[test]
fn plugin_descriptor_derived_traits() {
    let desc = PluginDescriptor::vst3("Test");
    let desc2 = desc.clone();
    assert_eq!(desc, desc2);
    let debug_str = format!("{:?}", desc);
    assert!(debug_str.contains("PluginDescriptor"));
}

#[test]
fn plugin_buffer_capacities_derived_traits() {
    use orpheus_dsp::PluginBufferCapacities;
    let cap1 = PluginBufferCapacities {
        voice_capacity: 10,
        parameter_cursor_capacity: 10,
        parameter_value_capacity: 10,
    };
    let cap2 = cap1.clone();
    assert_eq!(cap1, cap2);
    assert_eq!(
        format!("{:?}", cap1),
        "PluginBufferCapacities { voice_capacity: 10, parameter_cursor_capacity: 10, parameter_value_capacity: 10 }"
    );
}

#[test]
fn plugin_processor_processes_frames_without_growing_internal_buffers() {
    let notes = vec![
        Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap(),
            value: PluginNote::new(72, 0.8).unwrap(),
        },
        Event {
            // Note past cycle end
            whole: None,
            part: TimeSpan::new(Rational::new(2, 1).unwrap(), Rational::new(3, 1).unwrap())
                .unwrap(),
            value: PluginNote::new(72, 0.8).unwrap(),
        },
    ];
    let gain = PluginParameterLane::new(
        "Gain",
        vec![
            Event {
                whole: None,
                part: TimeSpan::unit(),
                value: 0.25,
            },
            Event {
                // Event past cycle end
                whole: None,
                part: TimeSpan::new(Rational::new(2, 1).unwrap(), Rational::new(3, 1).unwrap())
                    .unwrap(),
                value: 0.5,
            },
        ]
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
