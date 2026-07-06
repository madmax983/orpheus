cat << 'INNER_EOF' > crates/orpheus-dsp/tests/plugin_hosting.rs
use orpheus_dsp::{
    PluginDescriptor, PluginNote, PluginParameterLane, PluginProcessor, PluginTrackSource,
    RoutingSnapshot, SampleBank, TrackSource, render_routing_snapshot_to_stereo_for_test,
    PluginFormat, PluginHostError
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
fn plugin_host_error_display_and_clone() {
    let err = PluginHostError::EmptyIdentifier;
    assert_eq!(err.to_string(), "plugin identifier must not be empty");
    assert_eq!(err.clone(), PluginHostError::EmptyIdentifier);

    let err2 = PluginHostError::InvalidNoteNumber;
    assert_eq!(err2.to_string(), "plugin note number must be within [0, 127]");

    let err3 = PluginHostError::InvalidVelocity;
    assert_eq!(err3.to_string(), "plugin note velocity must be finite and within [0, 1]");

    let err4 = PluginHostError::EmptyParameterName;
    assert_eq!(err4.to_string(), "plugin parameter name must not be empty");
}

#[test]
fn plugin_descriptor_validates_empty_identifier() {
    let res = PluginDescriptor::try_new(PluginFormat::Vst3, "   ");
    assert_eq!(res, Err(PluginHostError::EmptyIdentifier));
}

#[test]
#[should_panic(expected = "invalid VST3 plugin descriptor: plugin identifier must not be empty")]
fn vst3_panics_on_empty_identifier() {
    let _ = PluginDescriptor::vst3("   ");
}

#[test]
#[should_panic(expected = "invalid AU plugin descriptor: plugin identifier must not be empty")]
fn audio_unit_panics_on_empty_identifier() {
    let _ = PluginDescriptor::audio_unit("   ");
}

#[test]
fn audio_unit_descriptor_uses_standard_os_search_paths() {
    let descriptor = PluginDescriptor::audio_unit("Serum");
    let paths = descriptor.search_paths();

    assert_eq!(descriptor.identifier(), "Serum");
    if cfg!(target_os = "macos") {
        assert!(
            paths
                .iter()
                .any(|path| path.to_string_lossy().to_lowercase().contains("components")),
            "expected default AU search paths, got {paths:?}"
        );
    }
}

#[test]
fn plugin_note_validation_and_getters() {
    let note = PluginNote::new(60, 0.5).unwrap();
    assert_eq!(note.note_number(), 60);
    assert_eq!(note.velocity(), 0.5);
    assert_eq!(note.channel(), 0);

    assert_eq!(PluginNote::new(60, -0.1), Err(PluginHostError::InvalidVelocity));
    assert_eq!(PluginNote::new(60, 1.1), Err(PluginHostError::InvalidVelocity));
    assert_eq!(PluginNote::new(60, f32::NAN), Err(PluginHostError::InvalidVelocity));
}

#[test]
fn plugin_parameter_lane_validation() {
    assert_eq!(PluginParameterLane::new("   ", vec![].into_boxed_slice()), Err(PluginHostError::EmptyParameterName));

    let events = vec![Event { whole: None, part: TimeSpan::unit(), value: -0.1 }];
    assert_eq!(PluginParameterLane::new("Gain", events.into_boxed_slice()), Err(PluginHostError::InvalidParameterValue));

    let events = vec![Event { whole: None, part: TimeSpan::unit(), value: f32::NAN }];
    assert_eq!(PluginParameterLane::new("Gain", events.into_boxed_slice()), Err(PluginHostError::InvalidParameterValue));
}

#[test]
fn plugin_track_source_with_parameter_lane_and_getters() {
    let descriptor = PluginDescriptor::vst3("Serum");
    let lane1 = PluginParameterLane::new("Gain", vec![].into_boxed_slice()).unwrap();
    let lane2 = PluginParameterLane::new("Cutoff", vec![].into_boxed_slice()).unwrap();

    let source = PluginTrackSource::new(descriptor.clone())
        .with_parameter_lane(lane1.clone())
        .with_parameter_lane(lane2.clone());

    assert_eq!(source.descriptor(), &descriptor);
    assert_eq!(source.parameter_lanes().len(), 2);
    assert_eq!(source.notes().len(), 0);
}
INNER_EOF
cargo test -p orpheus-dsp --test plugin_hosting
