//! Tests for the per-track lock-free level meters (ADR 0013).

use orpheus_dsp::{EngineHandle, MAX_METERED_TRACKS, decayed_peak, meter_decay_per_frame};

#[test]
fn decayed_peak_rises_instantly_to_a_louder_sample() {
    // A fresh, louder magnitude wins immediately via the `max`.
    let peak = decayed_peak(0.1, 0.8, 0.99);
    assert!((peak - 0.8).abs() < f32::EPSILON, "peak was {peak}");
}

#[test]
fn decayed_peak_holds_a_constant_level() {
    // With a sustained magnitude the meter sits at that level: decay pulls the
    // held value down but the `max` with the live sample pins it back up.
    let peak = decayed_peak(0.5, 0.5, 0.9);
    assert!((peak - 0.5).abs() < f32::EPSILON, "peak was {peak}");
}

#[test]
fn decayed_peak_falls_when_signal_stops() {
    // Silence (magnitude 0) lets the held peak decay by exactly the factor.
    let peak = decayed_peak(0.8, 0.0, 0.5);
    assert!((peak - 0.4).abs() < f32::EPSILON, "peak was {peak}");
}

#[test]
fn meter_decay_per_frame_is_between_zero_and_one() {
    let decay = meter_decay_per_frame(48_000);
    assert!(decay > 0.99 && decay < 1.0, "decay was {decay}");
    // Higher sample rates decay less per frame (more frames per second).
    assert!(meter_decay_per_frame(96_000) > meter_decay_per_frame(48_000));
}

#[test]
#[allow(clippy::float_cmp)]
fn silent_engine_reports_zero_peaks() {
    let mut engine = EngineHandle::stub();
    let _ = engine.render_test_block(256);
    let snapshot = engine.meter_snapshot();
    assert_eq!(snapshot.track_peak(0), 0.0);
    assert_eq!(snapshot.peaks().len(), MAX_METERED_TRACKS);
}

#[test]
fn triggered_track_reports_non_zero_peak() {
    let mut engine = EngineHandle::stub();
    engine.schedule_test_trigger(0, "bd");
    let _ = engine.render_test_block(256);

    let snapshot = engine.meter_snapshot();
    assert!(
        snapshot.track_peak(0) > 0.0,
        "expected a live peak on track 0, got {}",
        snapshot.track_peak(0)
    );
}

#[test]
#[allow(clippy::float_cmp)]
fn out_of_range_track_peak_is_zero() {
    let engine = EngineHandle::stub();
    let snapshot = engine.meter_snapshot();
    assert_eq!(snapshot.track_peak(MAX_METERED_TRACKS), 0.0);
    assert_eq!(snapshot.track_peak(usize::MAX), 0.0);
}
