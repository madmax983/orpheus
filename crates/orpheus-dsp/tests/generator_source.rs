//! Behavior tests for the engine-side generator track source (ADR 0009).
//!
//! A generator track (`TrackSource::Generator`) is fed one pre-materialized
//! cycle at a time over the lock-free command ring
//! (`EngineCommand::PushGeneratorCycle`). The engine adopts the pending
//! buffer at each cycle boundary and schedules it exactly like a sample
//! pattern; when no fresh buffer has arrived it re-schedules the last one
//! (graceful degradation, matching ADR 0008's failure mode). Scheduled
//! trigger durations honor the event's `whole` extent, so notes crossing a
//! cycle boundary sustain into the next cycle instead of clamping.

use orpheus_dsp::{
    EngineCommand, EngineHandle, GeneratorCycle, GeneratorId, MAX_GENERATORS, RoutingSnapshot,
    SampleTrigger, Scheduler, TrackId, TrackSource,
};
use orpheus_pattern::{Event, Rational, TimeSpan};

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn span(start: (i64, i64), end: (i64, i64)) -> TimeSpan {
    TimeSpan::new(rational(start.0, start.1), rational(end.0, end.1)).expect("valid span")
}

fn event(token: &str, part: TimeSpan, whole: Option<TimeSpan>) -> Event<SampleTrigger> {
    Event {
        whole,
        part,
        value: SampleTrigger::named(token),
    }
}

fn generator_snapshot(id: GeneratorId) -> RoutingSnapshot {
    RoutingSnapshot::builder()
        .track_with_source("main", TrackSource::Generator(id))
        .route("main", "master")
        .build()
        .expect("generator snapshot is valid")
}

/// Sets a fast tempo so one cycle is 240 frames, then renders up to the
/// first boundary so subsequent renders start exactly on a cycle.
fn engine_with_short_cycles() -> EngineHandle {
    let mut engine = EngineHandle::stub();
    engine.enqueue(EngineCommand::SetTempo(48_000.0)).unwrap();
    let _ = engine.render_test_block(1);
    engine
}

fn has_audio(samples: &[f32]) -> bool {
    samples.iter().any(|sample| sample.abs() > f32::EPSILON)
}

// ---------------------------------------------------------------------------
// Scheduler: durations come from the whole extent (cross-cycle sustain).
// ---------------------------------------------------------------------------

#[test]
fn scheduler_duration_uses_whole_extent_past_the_cycle_end() {
    // part [14/16, 1) clipped at the cycle end, whole [14/16, 22/16): the
    // trigger fires at 14/16 of the cycle but must sound for 8/16 of a
    // cycle, sustaining half a cycle past the boundary.
    let mut scheduler = Scheduler::new_for_test();
    scheduler
        .schedule_cycle_events(
            TrackId::new(0),
            0,
            1_600,
            [&event(
                "tri",
                span((14, 16), (1, 1)),
                Some(span((14, 16), (22, 16))),
            )],
        )
        .expect("schedules");
    let trigger = scheduler.pop_due(1_400).expect("trigger is due");
    assert_eq!(trigger.frame, 1_400);
    assert_eq!(
        trigger.duration_frames, 800,
        "duration derives from whole (8/16 of 1600 frames), not the clipped part"
    );
}

#[test]
fn scheduler_duration_without_whole_still_uses_part() {
    let mut scheduler = Scheduler::new_for_test();
    scheduler
        .schedule_cycle_events(
            TrackId::new(0),
            0,
            1_600,
            [&event("tri", span((0, 4), (1, 4)), None)],
        )
        .expect("schedules");
    let trigger = scheduler.pop_due(0).expect("trigger is due");
    assert_eq!(trigger.duration_frames, 400);
}

#[test]
fn scheduler_duration_ignores_whole_extending_before_the_trigger() {
    // An event clipped at the *start* of the window (whole begins in the
    // previous cycle) must not stretch its duration backward: the voice
    // starts at part.start and sounds until whole.end.
    let mut scheduler = Scheduler::new_for_test();
    scheduler
        .schedule_cycle_events(
            TrackId::new(0),
            0,
            1_600,
            [&event(
                "tri",
                span((0, 1), (1, 4)),
                Some(span((-1i64, 4), (1, 4))),
            )],
        )
        .expect("schedules");
    let trigger = scheduler.pop_due(0).expect("trigger is due");
    assert_eq!(
        trigger.duration_frames, 400,
        "duration runs from the trigger frame to whole.end"
    );
}

#[test]
fn scheduler_duration_saturates_instead_of_overflowing() {
    // A pathologically long whole (u32::MAX frames is ~24 hours at 48 kHz)
    // saturates rather than erroring the audio thread.
    let mut scheduler = Scheduler::new_for_test();
    scheduler
        .schedule_cycle_events(
            TrackId::new(0),
            0,
            96_000,
            [&event(
                "tri",
                span((0, 1), (1, 1)),
                Some(span((0, 1), (100_000, 1))),
            )],
        )
        .expect("schedules");
    let trigger = scheduler.pop_due(0).expect("trigger is due");
    assert_eq!(trigger.duration_frames, u32::MAX);
}

// ---------------------------------------------------------------------------
// Generator track source: per-cycle buffers over the command ring.
// ---------------------------------------------------------------------------

#[test]
fn generator_cycle_is_scheduled_at_the_next_boundary() {
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 4)), None)],
        )))
        .unwrap();

    // Render to the boundary where the snapshot and buffer are adopted.
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let rendered = engine.render_test_block(64);
    assert!(
        has_audio(&rendered),
        "the pushed cycle must sound after the adoption boundary"
    );
}

#[test]
fn generator_cycles_play_back_to_back_without_gaps() {
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    let frames_per_cycle = engine.frames_per_cycle_for_test();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 16)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    // Cycle k sounds; push cycle k+1 while k plays.
    let cycle_k = engine.render_test_block(frames_per_cycle / 2);
    assert!(has_audio(&cycle_k), "cycle k plays its trigger");
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 16)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let cycle_k1 = engine.render_test_block(frames_per_cycle / 2);
    assert!(
        has_audio(&cycle_k1),
        "cycle k+1 plays back-to-back with no silent cycle in between"
    );
}

#[test]
fn generator_without_a_fresh_buffer_loops_the_last_cycle() {
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    let frames_per_cycle = engine.frames_per_cycle_for_test();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 16)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let _ = engine.render_test_block(frames_per_cycle);

    // No new push: the engine re-schedules the last delivered buffer, the
    // same graceful degradation as a stalled re-publish under ADR 0008.
    let looped = engine.render_test_block(frames_per_cycle / 2);
    assert!(
        has_audio(&looped),
        "a starved generator loops its last cycle instead of falling silent"
    );
}

#[test]
fn empty_generator_cycle_silences_at_the_next_boundary() {
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    let frames_per_cycle = engine.frames_per_cycle_for_test();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 16)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let sounding = engine.render_test_block(frames_per_cycle / 2);
    assert!(has_audio(&sounding));

    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::silent(
            id,
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    let silent = engine.render_test_block(frames_per_cycle);
    assert!(
        !has_audio(&silent),
        "an empty cycle buffer silences the generator at the boundary"
    );
}

#[test]
fn generator_note_sustains_across_the_cycle_boundary() {
    // A saw note triggered at 3/4 of a cycle with whole extending to 5/4
    // must still sound in the first quarter of the *next* cycle. The next
    // cycle's buffer is empty, so any audio there comes from the sustained
    // voice, not from a re-trigger.
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    let frames_per_cycle = engine.frames_per_cycle_for_test();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event(
                "saw",
                span((3, 4), (1, 1)),
                Some(span((3, 4), (5, 4))),
            )],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::silent(
            id,
        )))
        .unwrap();
    let _ = engine.render_test_block(frames_per_cycle);

    // Now exactly at the boundary: the note's tail extends 1/4 cycle in.
    let tail = engine.render_test_block(frames_per_cycle / 4);
    assert!(
        has_audio(&tail),
        "the note tail sustains past the cycle boundary"
    );
    let after_tail = engine.render_test_block(frames_per_cycle / 2);
    assert!(
        !has_audio(&after_tail),
        "the voice ends once the whole extent has elapsed"
    );
}

#[test]
fn clamped_note_without_whole_still_ends_at_the_boundary() {
    // Control for the sustain test: the same note with no whole extent is
    // cut at the cycle end, proving the sustain comes from honoring whole.
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    let frames_per_cycle = engine.frames_per_cycle_for_test();
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("saw", span((3, 4), (1, 1)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());

    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::silent(
            id,
        )))
        .unwrap();
    let _ = engine.render_test_block(frames_per_cycle);

    let tail = engine.render_test_block(frames_per_cycle / 4);
    assert!(
        !has_audio(&tail),
        "without a whole extent the voice stops at the cycle end"
    );
}

#[test]
#[should_panic(expected = "render test block failed: generator id must be below 8")]
fn generator_id_out_of_range_is_rejected() {
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(u32::try_from(MAX_GENERATORS).expect("small constant"));
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 4)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(1);
}

#[test]
fn stop_transport_rewinds_and_replays_the_active_generator_cycle() {
    let mut engine = engine_with_short_cycles();
    let id = GeneratorId::new(0);
    engine
        .enqueue(EngineCommand::SwapRoutingSnapshot(generator_snapshot(id)))
        .unwrap();
    engine
        .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
            id,
            vec![event("bd", span((0, 1), (1, 16)), None)],
        )))
        .unwrap();
    let _ = engine.render_test_block(engine.frames_until_boundary_for_test());
    let _ = engine.render_test_block(32);

    engine.enqueue(EngineCommand::StopTransport).unwrap();
    let stopped = engine.render_test_block(64);
    assert!(!has_audio(&stopped), "stopped transport renders silence");

    engine.enqueue(EngineCommand::PlayTransport).unwrap();
    let resumed = engine.render_test_block(64);
    assert!(
        has_audio(&resumed),
        "play retains the active generator buffer, like a sample pattern"
    );
}
