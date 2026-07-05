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
        paths.iter().any(|path| {
            let s = path.to_string_lossy().to_uppercase();
            s.contains("VST3")
        }),
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
