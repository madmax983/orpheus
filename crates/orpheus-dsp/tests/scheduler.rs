use orpheus_dsp::Scheduler;
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

    scheduler
        .schedule_cycle_events(
            100,
            64,
            [Event {
                whole: None,
                part,
                value: "bd",
            }],
        )
        .unwrap();

    assert!(scheduler.drain_due_events(115).is_empty());
    assert_eq!(scheduler.drain_due_events(116), vec!["bd"]);
}

#[test]
fn schedule_cycle_events_is_atomic_on_error() {
    let mut scheduler = Scheduler::new_for_test();
    let good = Event {
        whole: None,
        part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
        value: "bd",
    };
    let bad = Event {
        whole: None,
        part: TimeSpan::new(Rational::new(1, 2).unwrap(), Rational::new(3, 4).unwrap()).unwrap(),
        value: "???",
    };

    assert!(scheduler.schedule_cycle_events(0, 64, [good, bad]).is_err());
    assert!(scheduler.drain_due_events(u64::MAX).is_empty());
}
