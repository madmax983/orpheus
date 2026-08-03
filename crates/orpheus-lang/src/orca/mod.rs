//! Orca-inspired grid livecoding surface (R&D spike).
//!
//! This module hosts a self-contained, TUI-free grid engine implementing
//! Orca semantics: row-major single-pass frame evaluation with immediate
//! writes, a per-frame lock set, uppercase-every-frame vs. lowercase-on-bang
//! execution, one-frame `*` bang lifetime, base-36 values, movement
//! operators that explode into bangs on collision or out-of-bounds, (as of
//! v2) the full `A`-`Z` pure-operator set with a deterministic, replayable
//! `R` (randomness hashed from frame and position), (as of v3) the IO
//! operator family (`:` `%` `!` `?` `;` `=` `$`) emitting typed
//! [`OrcaIoEvent`] payloads with no transport attached yet, (as of v4)
//! the `#` comment operator plus an audio bridge that maps note velocity to
//! gain and note length to event duration, and (as of v7) a host-side `$`
//! command interpreter ([`parse_command`]) transcribing the reference
//! `commander.js` grammar.
//!
//! The key seam to the rest of Orpheus is [`frame_span`], which maps grid
//! frame `N` of `F` frames-per-cycle onto the exact rational
//! [`orpheus_pattern::TimeSpan`] `[N/F, (N+1)/F)`. The publish bridge
//! materializes grid cycles into unit-cycle
//! `Vec<Event<SampleEvent>>` batches, one per engine cycle boundary because
//! running grids are not cycle-periodic. As of v5 (ADR 0009) those batches
//! feed a first-class engine generator source
//! (`orpheus_dsp::TrackSource::Generator`, slot [`ORCA_GENERATOR_ID`]), so
//! consecutive cycles play back-to-back and note lengths sustain across
//! cycle boundaries; the ADR 0008 per-cycle re-publish path remains as a
//! fallback. See `docs/design/orca-surface.md`.
//!
//! The grid engine ([`Grid`], [`OrcaEngine`]) stays TUI-free; the TUI pane
//! hosting this surface lives in the `tui` module.

mod commands;
mod engine;
mod grid;
mod publish;
pub mod transport;

pub use commands::{CommandOutcome, MAX_GRID_FRAME, OrcaCommand, adjusted_frame, parse_command};
pub use engine::{MidiNote, OrcaEngine, OrcaEvent, OrcaIoEvent, frame_span};
pub use grid::{BANG, COMMENT, EMPTY, Grid, GridError, is_valid_glyph};
pub use publish::{
    CycleIoEvent, DEFAULT_GRID_FRAMES_PER_CYCLE, DEFAULT_GRID_HEIGHT, DEFAULT_GRID_WIDTH,
    DEFAULT_SAMPLE_TOKEN, ORCA_GENERATOR_ID, ORCA_PATTERN_NAME, OrcaCycle, OrcaPublisher,
    materialize_cycle, materialize_cycle_io, materialize_generator_cycles, midi_note_id,
    playhead_frame, sample_event_from_orca,
};
