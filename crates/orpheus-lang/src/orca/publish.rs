//! Publish bridge: [`OrcaEvent`]s into session-publishable event batches.
//!
//! This is the v1 seam described in `docs/design/orca-surface.md` section 1.2
//! and ADR 0008: a materialized grid cycle is a unit-cycle
//! `Vec<Event<SampleEvent>>`, exactly the shape the existing
//! `SamplePatternValue::from_events` -> `ReplSession::publish_sample_events`
//! path consumes. Because a running grid is generally *not* cycle-periodic,
//! [`OrcaPublisher::poll`] re-materializes and re-publishes the next cycle's
//! events at every engine cycle boundary instead of publishing once.
//!
//! Note mapping (ADR 0008): an emitted note glyph's base-36 value (0-35) is
//! interpreted as a chromatic semitone offset above the configured sample
//! token's base pitch, applied as the playback-rate multiplier
//! `2^(value / 12)` — a three-octave range with no floats in the time domain.

use orpheus_pattern::{Event, PatternError};

use crate::value::SampleEvent;

use super::engine::{OrcaEngine, OrcaEvent, frame_span};
use super::grid::Grid;

/// Default grid frames per musical cycle: Orca's convention of 16th-note
/// frames over a 4-beat bar.
pub const DEFAULT_GRID_FRAMES_PER_CYCLE: u64 = 16;

/// Default sample token triggered by grid note events (a pitched synth
/// primitive from the sample-identifier whitelist).
pub const DEFAULT_SAMPLE_TOKEN: &str = "tri";

/// The session binding name under which grid cycles are published.
pub const ORCA_PATTERN_NAME: &str = "orca";

/// Default grid width for a freshly spawned surface (one column per frame at
/// the default 16 frames per cycle).
pub const DEFAULT_GRID_WIDTH: usize = 16;

/// Default grid height for a freshly spawned surface.
pub const DEFAULT_GRID_HEIGHT: usize = 8;

/// Converts one [`OrcaEvent`] fired at `frame_in_cycle` of `frames_per_cycle`
/// into an unclipped unit-cycle [`Event<SampleEvent>`].
///
/// The event occupies the exact rational span
/// `[frame_in_cycle / frames_per_cycle, (frame_in_cycle + 1) / frames_per_cycle)`
/// and triggers `sample_token` repitched by the note's base-36 value in
/// semitones.
///
/// # Errors
///
/// Returns [`PatternError::InvalidDenominator`] when `frames_per_cycle` is
/// zero, and propagates arithmetic errors from rational construction.
pub fn sample_event_from_orca(
    event: &OrcaEvent,
    frame_in_cycle: u64,
    frames_per_cycle: u64,
    sample_token: &str,
) -> Result<Event<SampleEvent>, PatternError> {
    let part = frame_span(frame_in_cycle, frames_per_cycle)?;
    let value = SampleEvent::named(sample_token).repitched(f64::from(event.value));
    Ok(Event {
        whole: None,
        part,
        value,
    })
}

/// Advances `engine` by one full grid cycle (`frames_per_cycle` ticks) and
/// returns the emitted events stamped with their cycle-relative frame spans.
///
/// Events are ordered by frame, and within a frame by the engine's row-major
/// scan order; same-frame events share a span. Calling this repeatedly yields
/// consecutive grid cycles: the grid state carries over, which is what makes
/// non-periodic grids (a moving `E`) evolve across cycles.
///
/// # Errors
///
/// Propagates rational-construction errors from [`frame_span`]. A
/// `frames_per_cycle` of zero ticks nothing and returns an empty batch.
pub fn materialize_cycle(
    engine: &mut OrcaEngine,
    frames_per_cycle: u64,
    sample_token: &str,
) -> Result<Vec<Event<SampleEvent>>, PatternError> {
    let mut events = Vec::new();
    for frame_in_cycle in 0..frames_per_cycle {
        for orca_event in engine.tick() {
            events.push(sample_event_from_orca(
                orca_event,
                frame_in_cycle,
                frames_per_cycle,
                sample_token,
            )?);
        }
    }
    Ok(events)
}

/// Maps the audio engine's transport position onto the current grid frame.
///
/// `current_frame` and `cycle_start_frame` are absolute audio frames from
/// `TransportSnapshot`; `engine_frames_per_cycle` is the audio frames per
/// musical cycle and `grid_frames_per_cycle` the grid's frame count `F`. The
/// result is clamped to `0..grid_frames_per_cycle` and is `None` when either
/// clock is degenerate (zero frames per cycle).
#[must_use]
pub const fn playhead_frame(
    current_frame: u64,
    cycle_start_frame: u64,
    engine_frames_per_cycle: u64,
    grid_frames_per_cycle: u64,
) -> Option<u64> {
    if engine_frames_per_cycle == 0 || grid_frames_per_cycle == 0 {
        return None;
    }
    let offset = current_frame.saturating_sub(cycle_start_frame);
    let frame = offset.saturating_mul(grid_frames_per_cycle) / engine_frames_per_cycle;
    if frame >= grid_frames_per_cycle {
        Some(grid_frames_per_cycle - 1)
    } else {
        Some(frame)
    }
}

/// Drives grid materialization from engine cycle boundaries.
///
/// The publisher owns the grid engine and a boundary tracker. The hosting
/// surface polls it with the latest `TransportSnapshot::current_cycle_start_frame`;
/// on the first poll after [`Self::start`] and on every boundary change it
/// materializes the next grid cycle for publication one cycle ahead of
/// playback (the engine adopts published patterns at the following boundary).
#[derive(Clone, Debug)]
pub struct OrcaPublisher {
    engine: OrcaEngine,
    frames_per_cycle: u64,
    sample_token: String,
    running: bool,
    last_cycle_start: Option<u64>,
}

impl OrcaPublisher {
    /// Wraps a grid engine with a cycle clock of `frames_per_cycle` grid
    /// frames per musical cycle, triggering `sample_token` for note events.
    #[must_use]
    pub fn new(engine: OrcaEngine, frames_per_cycle: u64, sample_token: &str) -> Self {
        Self {
            engine,
            frames_per_cycle,
            sample_token: sample_token.to_owned(),
            running: false,
            last_cycle_start: None,
        }
    }

    /// A stopped publisher over an empty default-sized grid.
    ///
    /// # Panics
    ///
    /// Never panics in practice: the default grid dimensions are nonzero.
    #[must_use]
    pub fn with_default_grid() -> Self {
        let grid = Grid::new(DEFAULT_GRID_WIDTH, DEFAULT_GRID_HEIGHT)
            .expect("default grid dimensions are nonzero");
        Self::new(
            OrcaEngine::new(grid),
            DEFAULT_GRID_FRAMES_PER_CYCLE,
            DEFAULT_SAMPLE_TOKEN,
        )
    }

    /// The wrapped grid engine.
    #[must_use]
    pub const fn engine(&self) -> &OrcaEngine {
        &self.engine
    }

    /// Mutable access to the grid engine, for editing between cycles.
    pub const fn engine_mut(&mut self) -> &mut OrcaEngine {
        &mut self.engine
    }

    /// Grid frames per musical cycle.
    #[must_use]
    pub const fn frames_per_cycle(&self) -> u64 {
        self.frames_per_cycle
    }

    /// Whether the grid clock is running (publishing at cycle boundaries).
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Starts the grid clock. The next [`Self::poll`] publishes immediately,
    /// regardless of the last seen boundary.
    pub const fn start(&mut self) {
        self.running = true;
        self.last_cycle_start = None;
    }

    /// Stops the grid clock; subsequent polls publish nothing.
    pub const fn stop(&mut self) {
        self.running = false;
    }

    /// Reports the current engine cycle boundary and returns the next grid
    /// cycle's events when they must be (re-)published.
    ///
    /// Returns `Ok(None)` while stopped or while the boundary is unchanged
    /// since the previous poll; returns `Ok(Some(events))` on the first poll
    /// after [`Self::start`] and whenever `cycle_start_frame` differs from the
    /// previous poll.
    ///
    /// # Errors
    ///
    /// Propagates rational-construction errors from materialization.
    pub fn poll(
        &mut self,
        cycle_start_frame: u64,
    ) -> Result<Option<Vec<Event<SampleEvent>>>, PatternError> {
        if !self.running || self.last_cycle_start == Some(cycle_start_frame) {
            return Ok(None);
        }
        self.last_cycle_start = Some(cycle_start_frame);
        materialize_cycle(&mut self.engine, self.frames_per_cycle, &self.sample_token).map(Some)
    }
}
