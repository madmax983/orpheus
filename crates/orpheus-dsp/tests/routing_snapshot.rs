//! Tests for audio graph routing snapshots, validating serialization and deserialization of connections.

use orpheus_dsp::{BusEffectSpec, RoutingError, RoutingSnapshot};
use orpheus_pattern::Rational;

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

#[test]
fn routing_snapshot_accepts_main_track_to_master() {
    let snapshot = RoutingSnapshot::builder().main_track().build().unwrap();
    assert_eq!(snapshot.track_count(), 1);
}

#[test]
fn routing_snapshot_rejects_bus_to_bus_edges() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .bus("verb")
        .bus("delay")
        .send("drums", "verb", 0.5)
        .route("verb", "delay")
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::BusToBusRoute {
            from: "verb".into(),
            to: "delay".into(),
        }
    );
}

#[test]
fn routing_snapshot_rejects_track_to_track_edges() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .track("bass")
        .route("drums", "bass")
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::TrackToTrackRoute {
            from: "drums".into(),
            to: "bass".into(),
        }
    );
}

#[test]
fn routing_snapshot_rejects_duplicate_track_names() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .track("drums")
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::DuplicateName {
            name: "drums".into(),
        }
    );
}

#[test]
fn routing_snapshot_rejects_send_to_unknown_bus() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .send("drums", "verb", 0.5)
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::UnknownBus {
            name: "verb".into(),
        }
    );
}

#[test]
fn routing_snapshot_rejects_invalid_send_levels() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .bus("verb")
        .send("drums", "verb", 1.5)
        .build()
        .unwrap_err();
    assert_eq!(error, RoutingError::InvalidLevel { level: 1.5 });
}

#[test]
fn routing_snapshot_rejects_track_to_bus_routes_that_skip_send() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .bus("verb")
        .route("drums", "verb")
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::TrackToBusRouteRequiresSend {
            from: "drums".into(),
            to: "verb".into(),
        }
    );
}

#[test]
fn routing_snapshot_rejects_duplicate_sends_to_same_bus() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .bus("verb")
        .send("drums", "verb", 0.4)
        .send("drums", "verb", 0.7)
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::DuplicateSend {
            track: "drums".into(),
            bus: "verb".into(),
        }
    );
}

#[test]
fn routing_snapshot_allows_unbound_tracks_but_keeps_them_silent() {
    let snapshot = RoutingSnapshot::builder().track("drums").build().unwrap();
    assert!(snapshot.track("drums").unwrap().is_phase1_silent());
}

#[test]
fn routing_snapshot_accepts_bus_routed_to_master() {
    let snapshot = RoutingSnapshot::builder()
        .track("drums")
        .bus("verb")
        .send("drums", "verb", 0.35)
        .build()
        .unwrap();
    assert!(snapshot.bus("verb").unwrap().routes_to_master());
}

#[test]
fn routing_snapshot_accepts_delay_effect_on_named_bus() {
    let snapshot = RoutingSnapshot::builder()
        .track("drums")
        .bus("dub")
        .bus_effect_delay("dub", rational(3, 16), 0.45, 1.0)
        .send("drums", "dub", 0.35)
        .build()
        .unwrap();

    assert!(snapshot.bus("dub").unwrap().effect().is_some());
}

#[test]
fn routing_snapshot_accepts_reverb_effect_on_named_bus() {
    let snapshot = RoutingSnapshot::builder()
        .track("pad")
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .send("pad", "verb", 0.45)
        .build()
        .unwrap();

    assert!(matches!(
        snapshot.bus("verb").unwrap().effect(),
        Some(BusEffectSpec::Reverb(_))
    ));
}

#[test]
fn routing_snapshot_rejects_invalid_delay_feedback() {
    let error = RoutingSnapshot::builder()
        .bus("dub")
        .bus_effect_delay("dub", rational(1, 8), 1.5, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("feedback"));
}

#[test]
fn routing_snapshot_rejects_invalid_delay_wet() {
    let error = RoutingSnapshot::builder()
        .bus("dub")
        .bus_effect_delay("dub", rational(1, 8), 0.45, 1.5)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("wet"));
}

#[test]
fn routing_snapshot_rejects_non_positive_delay_time() {
    let error = RoutingSnapshot::builder()
        .bus("dub")
        .bus_effect_delay("dub", rational(0, 1), 0.45, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("delay time"));
}

#[test]
fn routing_snapshot_rejects_invalid_reverb_size() {
    let error = RoutingSnapshot::builder()
        .bus("verb")
        .bus_effect_reverb("verb", 1.5, 0.35, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("size"));
}

#[test]
fn routing_snapshot_rejects_invalid_reverb_damp() {
    let error = RoutingSnapshot::builder()
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, -0.1, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("damp"));
}

#[test]
fn routing_snapshot_rejects_invalid_reverb_wet() {
    let error = RoutingSnapshot::builder()
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.5)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("wet"));
}

#[test]
fn routing_snapshot_rejects_delay_for_unknown_bus() {
    let error = RoutingSnapshot::builder()
        .bus_effect_delay("dub", rational(1, 8), 0.45, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("unknown bus"));
}

#[test]
fn routing_snapshot_rejects_reverb_for_unknown_bus() {
    let error = RoutingSnapshot::builder()
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("unknown bus"));
}

#[test]
fn routing_snapshot_rejects_duplicate_hosted_bus_effects() {
    let error = RoutingSnapshot::builder()
        .bus("dub")
        .bus_effect_delay("dub", rational(1, 8), 0.45, 1.0)
        .bus_effect_delay("dub", rational(3, 16), 0.25, 0.5)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("effect"));
}

#[test]
fn routing_snapshot_rejects_mixed_hosted_bus_effects_on_same_bus() {
    let error = RoutingSnapshot::builder()
        .bus("space")
        .bus_effect_delay("space", rational(1, 8), 0.45, 1.0)
        .bus_effect_reverb("space", 0.75, 0.35, 1.0)
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("effect"));
}
