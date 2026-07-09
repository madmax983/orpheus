//! Integration tests for event triggering and rational time scheduling against raw audio frames, verifying tight timing accuracy.
use orpheus_dsp::{SampleTrigger, Scheduler, TrackId};
use orpheus_pattern::{Event, Rational, TimeSpan};

#[test]
fn scheduler_emits_due_events_in_order() {
    let mut scheduler = Scheduler::new_for_test();
    scheduler.push_test_event(0, "bd");
    scheduler.push_test_event(32, "sn");

    let due = scheduler.drain_due_events(32);

    assert_eq!(due, vec!["bd", "sn"]);
}

#[test]
fn schedule_cycle_events_converts_rational_offsets_to_sample_frames() {
    let mut scheduler = Scheduler::new_for_test();
    let quarter = Rational::new(1, 4).unwrap();
    let half = Rational::new(1, 2).unwrap();
    let part = TimeSpan::new(quarter, half).unwrap();
    let trigger = SampleTrigger::named("bd");

    scheduler
        .schedule_cycle_events(
            TrackId::new(0),
            100,
            64,
            [&Event {
                whole: None,
                part,
                value: trigger,
            }],
        )
        .unwrap();

    assert!(scheduler.drain_due_events(115).is_empty());
    assert_eq!(scheduler.drain_due_events(116), vec!["bd"]);
}

#[test]
fn schedule_cycle_events_is_atomic_on_error() {
    let mut scheduler = Scheduler::new_for_test();
    let good_trigger = SampleTrigger::named("bd");
    let bad_trigger = SampleTrigger::named("vox_ah");
    let good = Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: good_trigger,
    };
    let bad = Event {
        whole: None,
        part: TimeSpan::new(Rational::new(-1, 4).unwrap(), Rational::zero()).unwrap(),
        value: bad_trigger,
    };

    assert!(
        scheduler
            .schedule_cycle_events(TrackId::new(0), 0, 64, [&good, &bad])
            .is_err()
    );
    assert!(scheduler.drain_due_events(u64::MAX).is_empty());
}

#[test]
fn schedule_cycle_events_accepts_custom_sample_tokens() {
    let mut scheduler = Scheduler::new_for_test();
    let quarter = Rational::new(1, 4).unwrap();
    let half = Rational::new(1, 2).unwrap();
    let trigger = SampleTrigger::named("vox_ah");

    scheduler
        .schedule_cycle_events(
            TrackId::new(0),
            0,
            64,
            [&Event {
                whole: None,
                part: TimeSpan::new(quarter, half).unwrap(),
                value: trigger,
            }],
        )
        .unwrap();

    assert_eq!(scheduler.drain_due_events(16), vec!["vox_ah"]);
}
