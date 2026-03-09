use orpheus_dsp::Scheduler;

#[test]
fn scheduler_emits_due_events_in_order() {
    let mut scheduler = Scheduler::new_for_test();
    scheduler.push_test_event(0, "bd");
    scheduler.push_test_event(32, "sn");

    let due = scheduler.drain_due_events(32);

    assert_eq!(due, vec!["bd", "sn"]);
}
