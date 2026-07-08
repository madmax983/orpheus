//! Frame-tick semantics for the Orca surface.
//!
//! One call to [`OrcaEngine::tick`] rewrites the whole grid once, following
//! the reference Orca evaluation model: row-major single pass with immediate
//! writes, a per-frame lock set, uppercase operators running every frame,
//! lowercase operators running only with a `*` in a cardinal neighbor cell,
//! and bangs living for exactly one frame.

use orpheus_pattern::{PatternError, Rational, TimeSpan};

use super::grid::{BANG, EMPTY, Grid, GridError};

/// The base-36 glyph alphabet: index = value.
const KEYS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Cardinal neighbor offsets, in Orca's E/W/S/N order.
const CARDINALS: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// A structured note event emitted by an output operator (`:`) during a tick.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrcaEvent {
    /// Frame index at which the event fired.
    pub frame: u64,
    /// Column of the output operator that emitted the event.
    pub x: usize,
    /// Row of the output operator that emitted the event.
    pub y: usize,
    /// The note glyph read from the operator's note port.
    pub note: char,
    /// Base-36 value of the note glyph (0-35).
    pub value: u8,
}

/// Maps grid frame `frame` of `frames_per_cycle` onto the exact rational span
/// `[frame / frames_per_cycle, (frame + 1) / frames_per_cycle)`.
///
/// This is the seam between the discrete Orca frame clock and Orpheus's
/// rational pattern time: one musical cycle is `[n, n + 1)`, so a grid running
/// `F` frames per cycle places frame `N` at `[N/F, (N+1)/F)`.
///
/// # Errors
///
/// Returns [`PatternError::InvalidDenominator`] when `frames_per_cycle` is
/// zero, and propagates arithmetic errors from rational construction.
pub fn frame_span(frame: u64, frames_per_cycle: u64) -> Result<TimeSpan, PatternError> {
    let denominator = i128::from(frames_per_cycle);
    let start = Rational::checked_from_parts(i128::from(frame), denominator)?;
    let end = Rational::checked_from_parts(i128::from(frame) + 1, denominator)?;
    TimeSpan::new(start, end)
}

/// The base-36 value of a glyph: `.`/`*` are 0, digits are themselves,
/// letters are 10-35 regardless of case.
const fn value_of(glyph: char) -> u64 {
    match glyph {
        '0'..='9' => glyph as u64 - '0' as u64,
        'a'..='z' => glyph as u64 - 'a' as u64 + 10,
        'A'..='Z' => glyph as u64 - 'A' as u64 + 10,
        _ => 0,
    }
}

/// The lowercase glyph for a base-36 value, wrapping mod 36.
fn key_of(value: u64) -> char {
    let index = usize::try_from(value % 36).expect("value % 36 fits in usize");
    char::from(KEYS[index])
}

/// The grid engine: grid state plus the frame counter and per-frame lock set.
#[derive(Clone, Debug)]
pub struct OrcaEngine {
    grid: Grid,
    frame: u64,
    locks: Vec<bool>,
    events: Vec<OrcaEvent>,
}

impl OrcaEngine {
    /// Wraps an existing grid with a fresh frame counter.
    #[must_use]
    pub fn new(grid: Grid) -> Self {
        let locks = vec![false; grid.width() * grid.height()];
        Self {
            grid,
            frame: 0,
            locks,
            events: Vec::new(),
        }
    }

    /// Builds an engine directly from rows of glyphs.
    ///
    /// # Errors
    ///
    /// Propagates [`GridError`] from [`Grid::from_rows`].
    pub fn from_rows(rows: &[&str]) -> Result<Self, GridError> {
        Ok(Self::new(Grid::from_rows(rows)?))
    }

    /// The current grid state.
    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Mutable access to the grid, for editing between ticks.
    pub const fn grid_mut(&mut self) -> &mut Grid {
        &mut self.grid
    }

    /// The index of the next frame [`Self::tick`] will evaluate.
    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    /// Events emitted by the most recent [`Self::tick`].
    #[must_use]
    pub fn events(&self) -> &[OrcaEvent] {
        &self.events
    }

    /// Advances the grid by one frame and returns the events it emitted.
    ///
    /// Semantics follow the reference implementation: the operator list is
    /// snapshotted at frame start, then evaluated row-major top-left to
    /// bottom-right with immediate writes to the shared grid. Locked cells
    /// are skipped; uppercase (and non-letter) operators run every frame
    /// while lowercase operators require a `*` in a cardinal neighbor cell.
    pub fn tick(&mut self) -> &[OrcaEvent] {
        self.events.clear();
        self.locks.fill(false);

        let mut snapshot = Vec::new();
        for y in 0..self.grid.height() {
            for x in 0..self.grid.width() {
                let glyph = self.grid.glyph_at(x, y).unwrap_or(EMPTY);
                if glyph != EMPTY {
                    snapshot.push((x, y, glyph));
                }
            }
        }

        for (x, y, glyph) in snapshot {
            if self.locked(x, y) {
                continue;
            }
            let passive = !glyph.is_ascii_lowercase();
            if passive || self.has_bang_neighbor(x, y) {
                self.run_operator(x, y, glyph);
            }
        }

        self.frame += 1;
        &self.events
    }

    fn run_operator(&mut self, x: usize, y: usize, glyph: char) {
        match glyph {
            BANG => {
                self.grid.set(x, y, EMPTY);
            }
            'N' | 'n' => self.op_move(x, y, glyph, 0, -1),
            'S' | 's' => self.op_move(x, y, glyph, 0, 1),
            'E' | 'e' => self.op_move(x, y, glyph, 1, 0),
            'W' | 'w' => self.op_move(x, y, glyph, -1, 0),
            'A' | 'a' => self.op_add(x, y),
            'C' | 'c' => self.op_clock(x, y),
            'D' | 'd' => self.op_delay(x, y),
            ':' => self.op_out(x, y),
            // Digits are inert data; unimplemented letters are no-ops in v0.
            _ => {}
        }
    }

    /// Movement: step into an empty destination (locking it), or explode into
    /// a `*` in place when the destination is occupied or out of bounds.
    fn op_move(&mut self, x: usize, y: usize, glyph: char, dx: i64, dy: i64) {
        let destination = self.offset(x, y, dx, dy);
        match destination {
            Some((nx, ny)) if self.grid.glyph_at(nx, ny) == Some(EMPTY) => {
                self.grid.set(x, y, EMPTY);
                self.grid.set(nx, ny, glyph);
                self.lock(nx, ny);
            }
            _ => {
                // Collision or out of bounds: the mover explodes.
                self.grid.set(x, y, BANG);
                self.lock(x, y);
            }
        }
    }

    /// Add: writes `keyOf((left + right) mod 36)` below, case-sensitive.
    fn op_add(&mut self, x: usize, y: usize) {
        let left = self.read_and_lock(x, y, -1, 0);
        let right = self.read_and_lock(x, y, 1, 0);
        let sum = value_of(left) + value_of(right);
        self.write_value_below(x, y, Some(key_of(sum)));
    }

    /// Clock: writes `keyOf(floor(frame / rate) mod m)` below every frame.
    /// An empty mod writes nothing (matching main-branch orca-js).
    fn op_clock(&mut self, x: usize, y: usize) {
        let rate = value_of(self.read_and_lock(x, y, -1, 0)).max(1);
        let modulo = value_of(self.read_and_lock(x, y, 1, 0));
        let output = (modulo > 0).then(|| key_of((self.frame / rate) % modulo));
        self.write_value_below(x, y, output);
    }

    /// Delay: bangs below when `frame mod (rate * m) == 0`, or always when
    /// `m == 1`; writes `.` on non-bang frames.
    fn op_delay(&mut self, x: usize, y: usize) {
        let rate = value_of(self.read_and_lock(x, y, -1, 0)).max(1);
        let modulo = value_of(self.read_and_lock(x, y, 1, 0)).max(1);
        let fires = self.frame.is_multiple_of(rate * modulo) || modulo == 1;
        if let Some((ox, oy)) = self.offset(x, y, 0, 1) {
            self.grid.set(ox, oy, if fires { BANG } else { EMPTY });
            self.lock(ox, oy);
        }
    }

    /// Output (simplified `:`): locks its note port every frame; when banged,
    /// emits an [`OrcaEvent`] carrying the note glyph and its base-36 value.
    fn op_out(&mut self, x: usize, y: usize) {
        let note = self.read_and_lock(x, y, 1, 0);
        if self.has_bang_neighbor(x, y) {
            self.events.push(OrcaEvent {
                frame: self.frame,
                x,
                y,
                note,
                value: u8::try_from(value_of(note) % 36).expect("base-36 value fits in u8"),
            });
        }
    }

    /// Applies a signed offset to a position, returning `None` when the
    /// result falls off the grid.
    fn offset(&self, x: usize, y: usize, dx: i64, dy: i64) -> Option<(usize, usize)> {
        let nx = i64::try_from(x).ok()?.checked_add(dx)?;
        let ny = i64::try_from(y).ok()?.checked_add(dy)?;
        let nx = usize::try_from(nx).ok()?;
        let ny = usize::try_from(ny).ok()?;
        self.grid.in_bounds(nx, ny).then_some((nx, ny))
    }

    /// Reads an operand port relative to the operator and locks its cell so
    /// the glyph acts as data instead of executing later this frame.
    /// Out-of-bounds ports read as empty.
    fn read_and_lock(&mut self, x: usize, y: usize, dx: i64, dy: i64) -> char {
        self.offset(x, y, dx, dy).map_or(EMPTY, |(px, py)| {
            self.lock(px, py);
            self.grid.glyph_at(px, py).unwrap_or(EMPTY)
        })
    }

    /// Writes a value-port output into the cell below the operator, applying
    /// Orca's case rule: the output is uppercased iff the glyph east of the
    /// operator is an uppercase letter. The output cell is locked even when
    /// nothing is written.
    fn write_value_below(&mut self, x: usize, y: usize, output: Option<char>) {
        let Some((ox, oy)) = self.offset(x, y, 0, 1) else {
            return;
        };
        self.lock(ox, oy);
        if let Some(glyph) = output {
            let uppercase = self
                .offset(x, y, 1, 0)
                .and_then(|(px, py)| self.grid.glyph_at(px, py))
                .is_some_and(|right| right.is_ascii_uppercase());
            let glyph = if uppercase {
                glyph.to_ascii_uppercase()
            } else {
                glyph
            };
            self.grid.set(ox, oy, glyph);
        }
    }

    /// Whether any cardinal neighbor currently holds a `*`.
    fn has_bang_neighbor(&self, x: usize, y: usize) -> bool {
        CARDINALS.iter().any(|&(dx, dy)| {
            self.offset(x, y, dx, dy)
                .and_then(|(nx, ny)| self.grid.glyph_at(nx, ny))
                == Some(BANG)
        })
    }

    fn lock(&mut self, x: usize, y: usize) {
        let index = y * self.grid.width() + x;
        self.locks[index] = true;
    }

    fn locked(&self, x: usize, y: usize) -> bool {
        let index = y * self.grid.width() + x;
        self.locks[index]
    }
}

#[cfg(test)]
mod tests {
    use super::{key_of, value_of};

    #[test]
    fn value_of_matches_reference_table() {
        assert_eq!(value_of('.'), 0);
        assert_eq!(value_of('*'), 0);
        assert_eq!(value_of('0'), 0);
        assert_eq!(value_of('9'), 9);
        assert_eq!(value_of('a'), 10);
        assert_eq!(value_of('A'), 10, "case never affects numeric value");
        assert_eq!(value_of('z'), 35);
        assert_eq!(value_of('Z'), 35);
    }

    #[test]
    fn key_of_wraps_mod_36() {
        assert_eq!(key_of(0), '0');
        assert_eq!(key_of(10), 'a');
        assert_eq!(key_of(35), 'z');
        assert_eq!(key_of(36), '0');
        assert_eq!(key_of(38), '2');
    }
}
