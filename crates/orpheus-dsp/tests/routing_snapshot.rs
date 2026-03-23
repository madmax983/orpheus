use orpheus_dsp::{RoutingError, RoutingSnapshot};

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
