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
//! [`OrcaIoEvent`] payloads with no transport attached yet, and (as of v4)
//! the `#` comment operator plus an audio bridge that maps note velocity to
//! gain and note length to event duration.
//!
//! The key seam to the rest of Orpheus is [`frame_span`], which maps grid
//! frame `N` of `F` frames-per-cycle onto the exact rational
//! [`orpheus_pattern::TimeSpan`] `[N/F, (N+1)/F)`. The v1 publish bridge
//! ([`publish`]) materializes grid cycles into unit-cycle
//! `Vec<Event<SampleEvent>>` batches for the existing session publication
//! path, re-publishing at every engine cycle boundary because running grids
//! are not cycle-periodic. See `docs/design/orca-surface.md` and ADR 0008.
//!
//! The grid engine ([`Grid`], [`OrcaEngine`]) stays TUI-free; the TUI pane
//! hosting this surface lives in the `tui` module.

mod engine;
mod grid;
mod publish;

pub use engine::{MidiNote, OrcaEngine, OrcaEvent, OrcaIoEvent, frame_span};
pub use grid::{BANG, COMMENT, EMPTY, Grid, GridError, is_valid_glyph};
pub use publish::{
    DEFAULT_GRID_FRAMES_PER_CYCLE, DEFAULT_GRID_HEIGHT, DEFAULT_GRID_WIDTH, DEFAULT_SAMPLE_TOKEN,
    ORCA_PATTERN_NAME, OrcaPublisher, materialize_cycle, midi_note_id, playhead_frame,
    sample_event_from_orca,
};
