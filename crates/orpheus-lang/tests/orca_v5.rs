//! Behavior tests for Orca v5: the grid as a first-class engine generator
//! source (ADR 0009, superseding the per-cycle re-publish of ADR 0008).
//!
//! The session now ships each materialized grid cycle to a dedicated
//! `TrackSource::Generator` over `EngineCommand::PushGeneratorCycle` instead
//! of re-publishing a whole pattern binding per cycle. Because the engine's
//! scheduler derives trigger durations from the event's `whole` extent, a
//! note whose length crosses the cycle boundary now sustains into the next
//! cycle instead of clamping (the documented v4 limitation of
//! `docs/design/orca-surface.md` section 10.2, resolved in section 11).

use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;
use orpheus_lang::SampleEvent;
use orpheus_lang::{
    MidiNote, ORCA_GENERATOR_ID, ORCA_PATTERN_NAME, OrcaEngine, OrcaEvent, OrcaIoEvent,
    OrcaPublisher, materialize_cycle, sample_event_from_orca,
};
use orpheus_pattern::{Event, Rational};

type SampleEventBatch = Vec<Event<SampleEvent>>;

fn session() -> ReplSession {
    ReplSession::with_engine(EngineHandle::stub())
}

fn engine(rows: &[&str]) -> OrcaEngine {
    OrcaEngine::from_rows(rows).expect("test grids are well-formed")
}

const fn note_event(velocity: u8, length: u8) -> OrcaEvent {
    OrcaEvent {
        frame: 0,
        x: 0,
        y: 0,
        io: OrcaIoEvent::Midi(MidiNote {
            channel: 0,
            octave: 3,
            note: 'C',
            velocity,
            length,
        }),
    }
}

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

/// A single note fired at grid frame 14 of 16 with length 8: part clipped
/// at the cycle end, whole extending to 22/16.
fn boundary_crossing_batch() -> SampleEventBatch {
    let event = sample_event_from_orca(&note_event(15, 8), 14, 16, "tri")
        .expect("valid span")
        .expect("note event");
    vec![event]
}

fn has_audio(samples: &[f32]) -> bool {
    samples.iter().any(|sample| sample.abs() > f32::EPSILON)
}

// ---------------------------------------------------------------------------
// Session generator seam.
// ---------------------------------------------------------------------------

#[test]
fn start_generator_source_registers_the_binding_and_sounds() {
    let mut session = session();
    let mut orca = engine(&[".D1...", "..:04c"]);
    let events = materialize_cycle(&mut orca, 4, "tri").expect("materializes");
    assert!(!events.is_empty(), "D1 fires every frame");

    session
        .start_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID, events)
        .expect("generator start publishes");

    assert!(
        session
            .binding_summaries()
            .iter()
            .any(|summary| summary == "orca: Pattern<Sample>"),
        "the generator registers a session binding like any pattern"
    );

    // A fresh engine primes the generator routing immediately, so the first
    // grid cycle is audible without waiting a full cycle.
    let rendered = session.render_test_block_for_tui(4_096);
    assert!(has_audio(&rendered), "the first grid cycle sounds");
}

#[test]
fn push_generator_cycle_keeps_consecutive_cycles_sounding() {
    let mut session = session();
    let mut orca = engine(&[".D1...", "..:04c"]);
    let cycle0 = materialize_cycle(&mut orca, 4, "tri").expect("materializes");
    session
        .start_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID, cycle0)
        .expect("generator start publishes");

    let frames_per_cycle = session.transport_snapshot().frames_per_cycle();
    let _ = session.render_test_block_for_tui(frames_per_cycle / 2);

    // Deliver cycle 1 while cycle 0 plays, exactly as the TUI poll does.
    let cycle1 = materialize_cycle(&mut orca, 4, "tri").expect("materializes");
    session
        .push_generator_cycle(ORCA_GENERATOR_ID, cycle1)
        .expect("cycle push enqueues");
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let next_cycle = session.render_test_block_for_tui(frames_per_cycle / 2);
    assert!(
        has_audio(&next_cycle),
        "cycle 1 plays back-to-back after cycle 0"
    );
}

#[test]
fn note_crossing_the_cycle_boundary_sustains_through_the_session_path() {
    let mut session = session();
    session
        .start_generator_source(
            ORCA_PATTERN_NAME,
            ORCA_GENERATOR_ID,
            boundary_crossing_batch(),
        )
        .expect("generator start publishes");
    let frames_per_cycle = session.transport_snapshot().frames_per_cycle();
    // Let the engine adopt the first cycle before delivering the empty
    // follow-up cycle: whatever sounds after the boundary is then the
    // sustained voice, not a re-trigger.
    let _ = session.render_test_block_for_tui(frames_per_cycle / 2);
    session
        .push_generator_cycle(ORCA_GENERATOR_ID, Vec::new())
        .expect("cycle push enqueues");
    let _ = session.render_test_block_for_tui(frames_per_cycle / 2);

    // Note tail: frames 0..(6/16 cycle) of the next cycle.
    let tail_frames = frames_per_cycle * 6 / 16;
    let tail = session.render_test_block_for_tui(tail_frames * 3 / 4);
    assert!(
        has_audio(&tail),
        "the note sustains past the cycle boundary (v4 clamped here)"
    );

    let _ = session.render_test_block_for_tui(tail_frames - tail_frames * 3 / 4);
    let after_tail = session.render_test_block_for_tui(frames_per_cycle / 4);
    assert!(
        !has_audio(&after_tail),
        "the sustained voice ends once its whole extent has elapsed"
    );
}

#[test]
fn stop_generator_source_silences_at_the_next_boundary() {
    let mut session = session();
    let mut orca = engine(&[".D1...", "..:04c"]);
    let events = materialize_cycle(&mut orca, 4, "tri").expect("materializes");
    session
        .start_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID, events)
        .expect("generator start publishes");

    let frames_per_cycle = session.transport_snapshot().frames_per_cycle();
    let sounding = session.render_test_block_for_tui(frames_per_cycle / 2);
    assert!(has_audio(&sounding));

    session
        .stop_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID)
        .expect("generator stop publishes");
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

    let stopped = session.render_test_block_for_tui(frames_per_cycle);
    assert!(
        !has_audio(&stopped),
        "stopping the generator silences it at the next cycle boundary"
    );
}

// ---------------------------------------------------------------------------
// Driver: grid edits and cycle cadence.
// ---------------------------------------------------------------------------

#[test]
fn grid_edits_between_cycles_take_effect_on_the_following_cycle() {
    // Start from a silent grid; add a bang + note between boundaries and the
    // next materialized cycle picks it up.
    let orca = engine(&["......", "......"]);
    let mut publisher = OrcaPublisher::new(orca, 4, "tri");
    publisher.start();

    let cycle0 = publisher
        .poll(0)
        .expect("materializes")
        .expect("first poll publishes");
    assert!(cycle0.is_empty(), "the empty grid emits nothing");

    for (x, glyph) in [(1, 'D'), (2, '1')] {
        assert!(publisher.engine_mut().grid_mut().set(x, 0, glyph));
    }
    for (x, glyph) in [(2, ':'), (3, '0'), (4, '4'), (5, 'c')] {
        assert!(publisher.engine_mut().grid_mut().set(x, 1, glyph));
    }

    let cycle1 = publisher
        .poll(96_000)
        .expect("materializes")
        .expect("boundary change publishes");
    assert!(
        !cycle1.is_empty(),
        "the edit lands in the very next materialized cycle"
    );
}

#[test]
fn publisher_events_preserve_the_unclipped_whole_extent() {
    // The lang-side bridge no longer truncates what reaches the engine: the
    // event's whole keeps the full extent past the cycle end, which the
    // scheduler converts into a cross-boundary sustain.
    let batch = boundary_crossing_batch();
    assert_eq!(batch[0].part.start(), &rational(14, 16));
    assert_eq!(batch[0].part.end(), &rational(1, 1));
    let whole = batch[0].whole.expect("crossing notes keep their whole");
    assert_eq!(whole.end(), &rational(22, 16));
}

#[test]
fn generator_binding_survives_unrelated_mixer_edits() {
    // Compiling a mixer snapshot while the generator runs must keep the
    // orca track pointed at the generator source, not recompile it as a
    // static sample pattern.
    let mut session = session();
    let mut orca = engine(&[".D1...", "..:04c"]);
    let events = materialize_cycle(&mut orca, 4, "tri").expect("materializes");
    session
        .start_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID, events)
        .expect("generator start publishes");

    session
        .eval_line(":bus new fx1")
        .expect("mixer edits succeed while the generator runs");

    let frames_per_cycle = session.transport_snapshot().frames_per_cycle();
    let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
    let rendered = session.render_test_block_for_tui(frames_per_cycle / 2);
    assert!(
        has_audio(&rendered),
        "the generator keeps sounding (looping its last cycle) after a mixer recompile"
    );
}

// ---------------------------------------------------------------------------
// Event mapping regression: whole/part shapes flow into triggers unchanged.
// ---------------------------------------------------------------------------

#[test]
fn non_crossing_notes_still_publish_without_a_whole() {
    let event: Event<SampleEvent> = sample_event_from_orca(&note_event(15, 2), 4, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert_eq!(event.whole, None);
    assert_eq!(event.part.start(), &rational(4, 16));
    assert_eq!(event.part.end(), &rational(6, 16));
}
