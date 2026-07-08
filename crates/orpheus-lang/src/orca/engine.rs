//! Frame-tick semantics for the Orca surface.
//!
//! One call to [`OrcaEngine::tick`] rewrites the whole grid once, following
//! the reference Orca evaluation model: row-major single pass with immediate
//! writes, a per-frame lock set, uppercase operators running every frame,
//! lowercase operators running only with a `*` in a cardinal neighbor cell,
//! and bangs living for exactly one frame.
//!
//! v2 implements the full `A`-`Z` pure-operator set (verified against
//! `hundredrabbits/Orca` main-branch `library.js`), including a per-frame
//! `V`/`K` variable store and a deterministic `R`: randomness is hashed from
//! (frame, position) via [`frame_position_hash`] so grid runs replay exactly.
//!
//! v3 adds the IO operator family (`:` `%` `!` `?` `;` `=` `$`): always
//! passive but acting only when banged, locking their data ports eastward on
//! every frame, and emitting typed [`OrcaIoEvent`] payloads instead of
//! driving real transports.

use std::collections::BTreeMap;

use orpheus_pattern::{PatternError, Rational, TimeSpan};

use super::grid::{BANG, COMMENT, EMPTY, Grid, GridError};

/// The base-36 glyph alphabet: index = value.
const KEYS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Cardinal neighbor offsets, in Orca's E/W/S/N order.
const CARDINALS: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// A MIDI note message, the shared payload of the `:` (polyphonic) and `%`
/// (monophonic) operators. Port semantics match reference `library.js`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiNote {
    /// MIDI channel, 0-15 (the operator aborts above 15).
    pub channel: u8,
    /// Octave, clamped to 0-8.
    pub octave: u8,
    /// The note glyph, case preserved: uppercase letters are naturals,
    /// lowercase are sharps (see the reference `transpose.js` table, mirrored
    /// by [`super::publish::midi_note_id`]).
    pub note: char,
    /// Velocity, clamped to 0-16; an empty port defaults to `f` (15).
    pub velocity: u8,
    /// Note length in grid frames, clamped to 0-32; an empty port defaults
    /// to 1.
    pub length: u8,
}

/// Typed payloads for the IO operator family. No transport is attached in
/// v3: events are collected per tick so future consumers (real MIDI out,
/// UDP/OSC sockets, a command interpreter) can drain them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrcaIoEvent {
    /// `:` — polyphonic MIDI note.
    Midi(MidiNote),
    /// `%` — monophonic MIDI note (the transport cuts the previous note).
    MidiMono(MidiNote),
    /// `!` — MIDI control change; `value` is scaled to 0-127 via
    /// `ceil(127 * raw / 35)`.
    MidiCc {
        /// MIDI channel, 0-15 (the operator aborts above 15).
        channel: u8,
        /// Knob (controller) number, 0-35.
        knob: u8,
        /// Controller value scaled to 0-127.
        value: u8,
    },
    /// `?` — MIDI pitch bend; `lsb`/`msb` are scaled to 0-127.
    MidiPb {
        /// MIDI channel, clamped to 0-15 (no abort, unlike `:`/`%`/`!`).
        channel: u8,
        /// Least significant byte, scaled to 0-127.
        lsb: u8,
        /// Most significant byte, scaled to 0-127.
        msb: u8,
    },
    /// `;` — UDP message: the eastward glyphs up to the first empty cell.
    Udp(String),
    /// `=` — OSC message. The wire format (future transport) prepends `/` to
    /// the path glyph and sends each arg glyph as its base-36 value.
    Osc {
        /// The single path glyph east of the operator.
        path: char,
        /// Raw arg glyphs (cells east of the path, up to the first empty).
        args: String,
    },
    /// `$` — self command: the raw command string (e.g. `bpm90`), for a host
    /// command interpreter.
    Command(String),
}

/// A structured event emitted by an IO operator during a tick.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrcaEvent {
    /// Frame index at which the event fired.
    pub frame: u64,
    /// Column of the operator that emitted the event.
    pub x: usize,
    /// Row of the operator that emitted the event.
    pub y: usize,
    /// The typed IO payload.
    pub io: OrcaIoEvent,
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
///
/// Visible outside this (private) module so the OSC transport encodes arg
/// glyphs with the same table; not re-exported from the `orca` module.
pub const fn value_of(glyph: char) -> u64 {
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

/// Converts a base-36 value to `i64` for signed offset arithmetic.
fn to_i64(value: u64) -> i64 {
    i64::try_from(value).expect("base-36 values fit in i64")
}

/// Converts an already-clamped port value to `u8` for IO event payloads.
fn to_u8(value: u64) -> u8 {
    u8::try_from(value).expect("clamped port values fit in u8")
}

/// Applies a port's default glyph: the reference substitutes the default
/// when the cell reads `.` or `*` (`operator.js` `listen`).
const fn defaulted(glyph: char, default: char) -> char {
    if glyph == EMPTY || glyph == BANG {
        default
    } else {
        glyph
    }
}

/// Scales a base-36 port value onto 0-127 the way the reference scales CC
/// values and pitch-bend bytes: `ceil(127 * raw / 35)`.
fn scale_to_127(raw: u64) -> u8 {
    to_u8((127 * raw.min(35)).div_ceil(35))
}

/// Deterministic pseudo-random `u64` for the `R` operator, hashed from the
/// frame number and the operator's grid position.
///
/// This mirrors the degrade family's `event_coin` (see
/// `crates/orpheus-lang/src/value.rs`): XOR the inputs with distinct
/// rotations so they decorrelate, then apply the `SplitMix64` finalizer. The
/// same (frame, position) always yields the same value, so grids containing
/// `R` remain deterministic, replayable functions of (initial grid, frame) —
/// the property the publish bridge and any future `Pattern<T>` impl rely on.
fn frame_position_hash(frame: u64, x: usize, y: usize) -> u64 {
    let column = u64::try_from(x).expect("grid coordinates fit in u64");
    let row = u64::try_from(y).expect("grid coordinates fit in u64");
    let mut state = frame ^ column.rotate_left(11) ^ row.rotate_left(43);
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;
    state
}

/// The grid engine: grid state plus the frame counter and per-frame lock set.
#[derive(Clone, Debug)]
pub struct OrcaEngine {
    grid: Grid,
    frame: u64,
    locks: Vec<bool>,
    events: Vec<OrcaEvent>,
    /// Per-frame `V`/`K` variable store, cleared at the start of every tick
    /// (matching orca-js `release()`): writers must precede readers in scan
    /// order.
    variables: BTreeMap<char, char>,
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
            variables: BTreeMap::new(),
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
        self.variables.clear();

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
            'B' | 'b' => self.op_subtract(x, y),
            'C' | 'c' => self.op_clock(x, y),
            'D' | 'd' => self.op_delay(x, y),
            'F' | 'f' => self.op_if(x, y),
            'G' | 'g' => self.op_generate(x, y),
            'H' | 'h' => self.op_halt(x, y),
            'I' | 'i' => self.op_increment(x, y),
            'J' | 'j' => self.op_jumper(x, y, glyph),
            'K' | 'k' => self.op_konkat(x, y),
            'L' | 'l' => self.op_lesser(x, y),
            'M' | 'm' => self.op_multiply(x, y),
            'O' | 'o' => self.op_read(x, y),
            'P' | 'p' => self.op_push(x, y),
            'Q' | 'q' => self.op_query(x, y),
            'R' | 'r' => self.op_random(x, y),
            'T' | 't' => self.op_track(x, y),
            'U' | 'u' => self.op_uclid(x, y),
            'V' | 'v' => self.op_variable(x, y),
            'X' | 'x' => self.op_write(x, y),
            'Y' | 'y' => self.op_jymper(x, y, glyph),
            'Z' | 'z' => self.op_lerp(x, y),
            COMMENT => self.op_comment(x, y),
            ':' => self.op_midi(x, y, false),
            '%' => self.op_midi(x, y, true),
            '!' => self.op_cc(x, y),
            '?' => self.op_pb(x, y),
            ';' => self.op_udp(x, y),
            '=' => self.op_osc(x, y),
            '$' => self.op_self(x, y),
            // Digits are inert data.
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
        let left = value_of(self.read_and_lock(x, y, -1, 0));
        let right = value_of(self.read_and_lock(x, y, 1, 0));
        self.write_port(x, y, 0, 1, Some(key_of(left + right)), true);
    }

    /// Subtract: writes `keyOf(|right - left|)` below, case-sensitive.
    fn op_subtract(&mut self, x: usize, y: usize) {
        let left = value_of(self.read_and_lock(x, y, -1, 0));
        let right = value_of(self.read_and_lock(x, y, 1, 0));
        self.write_port(x, y, 0, 1, Some(key_of(left.abs_diff(right))), true);
    }

    /// Lesser: writes `keyOf(min(left, right))` below, case-sensitive.
    fn op_lesser(&mut self, x: usize, y: usize) {
        let left = value_of(self.read_and_lock(x, y, -1, 0));
        let right = value_of(self.read_and_lock(x, y, 1, 0));
        self.write_port(x, y, 0, 1, Some(key_of(left.min(right))), true);
    }

    /// Multiply: writes `keyOf((left * right) mod 36)` below, case-sensitive.
    fn op_multiply(&mut self, x: usize, y: usize) {
        let left = value_of(self.read_and_lock(x, y, -1, 0));
        let right = value_of(self.read_and_lock(x, y, 1, 0));
        self.write_port(x, y, 0, 1, Some(key_of(left * right)), true);
    }

    /// Clock: writes `keyOf(floor(frame / rate) mod m)` below every frame.
    /// An empty mod writes nothing (matching main-branch orca-js).
    fn op_clock(&mut self, x: usize, y: usize) {
        let rate = value_of(self.read_and_lock(x, y, -1, 0)).max(1);
        let modulo = value_of(self.read_and_lock(x, y, 1, 0));
        let output = (modulo > 0).then(|| key_of((self.frame / rate) % modulo));
        self.write_port(x, y, 0, 1, output, true);
    }

    /// Delay: bangs below when `frame mod (rate * m) == 0`, or always when
    /// `m == 1`; writes `.` on non-bang frames.
    fn op_delay(&mut self, x: usize, y: usize) {
        let rate = value_of(self.read_and_lock(x, y, -1, 0)).max(1);
        let modulo = value_of(self.read_and_lock(x, y, 1, 0)).max(1);
        let fires = self.frame.is_multiple_of(rate * modulo) || modulo == 1;
        self.bang_below(x, y, fires);
    }

    /// If: bangs below when the operand glyphs are equal (raw glyph
    /// comparison, case-sensitive; two empty cells compare equal).
    fn op_if(&mut self, x: usize, y: usize) {
        let left = self.read_port(x, y, -1, 0);
        let right = self.read_port(x, y, 1, 0);
        self.bang_below(x, y, left == right);
    }

    /// Uclid: bangs below on the Euclidean rhythm
    /// `(step * (frame + max - 1)) mod max + step >= max`. No port defaults
    /// (matching main-branch orca-js): an empty step never bangs.
    fn op_uclid(&mut self, x: usize, y: usize) {
        let step = value_of(self.read_and_lock(x, y, -1, 0));
        let max = value_of(self.read_and_lock(x, y, 1, 0)).max(1);
        let bucket = (step * (self.frame + max - 1)) % max + step;
        self.bang_below(x, y, bucket >= max);
    }

    /// Random: writes a uniform value from the inclusive range spanned by
    /// the operands, case-sensitive. Deterministic: the draw is
    /// [`frame_position_hash`] of the frame number and operator position, so
    /// identical grids replay identically.
    fn op_random(&mut self, x: usize, y: usize) {
        let a = value_of(self.read_and_lock(x, y, -1, 0));
        let b = value_of(self.read_and_lock(x, y, 1, 0));
        // orca-js main swaps a descending range, making [min, max] inclusive
        // either way; equal operands return the operand.
        let (low, high) = if a <= b { (a, b) } else { (b, a) };
        let value = low + frame_position_hash(self.frame, x, y) % (high - low + 1);
        self.write_port(x, y, 0, 1, Some(key_of(value)), true);
    }

    /// Increment: reads its own output cell as state and rewrites it stepped
    /// by `step` (left, empty means +0), wrapping mod `m` (right); an empty
    /// mod outputs `0`. Case-sensitive.
    fn op_increment(&mut self, x: usize, y: usize) {
        let step = value_of(self.read_and_lock(x, y, -1, 0));
        let modulo = value_of(self.read_and_lock(x, y, 1, 0));
        let value = value_of(self.port_glyph(x, y, 0, 1).unwrap_or(EMPTY));
        let output = if modulo == 0 {
            '0'
        } else {
            key_of((value + step) % modulo)
        };
        self.write_port(x, y, 0, 1, Some(output), true);
    }

    /// Lerp: steps its own output cell toward `target` (right) by `rate`
    /// (left, empty means 0) per frame, clamping onto the target when close.
    /// Case-sensitive.
    fn op_lerp(&mut self, x: usize, y: usize) {
        let rate = self.read_value(x, y, -1, 0);
        let target = self.read_value(x, y, 1, 0);
        let value = to_i64(value_of(self.port_glyph(x, y, 0, 1).unwrap_or(EMPTY)));
        let delta = if value <= target - rate {
            rate
        } else if value >= target + rate {
            -rate
        } else {
            target - value
        };
        let next = u64::try_from((value + delta).rem_euclid(36)).expect("rem_euclid(36) >= 0");
        self.write_port(x, y, 0, 1, Some(key_of(next)), true);
    }

    /// Halt: locks the cell below so its operator never executes. Writes
    /// nothing (in orca-js the halted glyph's value round-trips through a
    /// write that always refuses it).
    fn op_halt(&mut self, x: usize, y: usize) {
        self.lock_offset(x, y, 0, 1);
    }

    /// Variable: with a `write` operand (west), stores `variables[write] =
    /// read` and outputs nothing; otherwise with a `read` operand (east),
    /// outputs the stored glyph (or `.`) below, verbatim. The store is
    /// cleared every frame, so writers must precede readers in scan order.
    fn op_variable(&mut self, x: usize, y: usize) {
        let write = self.read_and_lock(x, y, -1, 0);
        let read = self.read_and_lock(x, y, 1, 0);
        if write != EMPTY {
            self.variables.insert(write, read);
        } else if read != EMPTY {
            let value = self.variables.get(&read).copied().unwrap_or(EMPTY);
            self.write_port(x, y, 0, 1, Some(value), false);
        }
    }

    /// Konkat: treats each of the `len` glyphs east as a variable name,
    /// locking it and writing the variable's value (or `.`) beneath it.
    fn op_konkat(&mut self, x: usize, y: usize) {
        let len = self.read_value(x, y, -1, 0).max(1);
        for offset in 0..len {
            let key = self.read_and_lock(x, y, offset + 1, 0);
            if key == EMPTY {
                continue;
            }
            let value = self.variables.get(&key).copied().unwrap_or(EMPTY);
            self.write_port(x, y, offset + 1, 1, Some(value), false);
        }
    }

    /// Generate: copies the `len` glyphs east of the operator to the block
    /// starting at relative `{x, y + 1}`, verbatim, locking both the
    /// operands and the written cells.
    fn op_generate(&mut self, x: usize, y: usize) {
        let dx = self.read_value(x, y, -3, 0);
        let dy = self.read_value(x, y, -2, 0) + 1;
        let len = self.read_value(x, y, -1, 0).max(1);
        for offset in 0..len {
            let glyph = self.read_port(x, y, offset + 1, 0);
            self.write_port(x, y, dx + offset, dy, glyph, false);
        }
    }

    /// Read (`O`): reads the glyph at relative `{x + 1, y}` and outputs it
    /// below, verbatim.
    fn op_read(&mut self, x: usize, y: usize) {
        let dx = self.read_value(x, y, -2, 0);
        let dy = self.read_value(x, y, -1, 0);
        let glyph = self.read_port(x, y, dx + 1, dy);
        self.write_port(x, y, 0, 1, glyph, false);
    }

    /// Push: locks the `len`-wide row below and writes the value operand
    /// into slot `key mod len` of it, verbatim.
    fn op_push(&mut self, x: usize, y: usize) {
        let key = self.read_value(x, y, -2, 0);
        let len = self.read_value(x, y, -1, 0).max(1);
        let value = self.read_port(x, y, 1, 0);
        for offset in 0..len {
            self.lock_offset(x, y, offset, 1);
        }
        self.write_port(x, y, key % len, 1, value, false);
    }

    /// Query: reads `len` glyphs starting at relative `{x + 1, y}` and
    /// writes them, verbatim, so the last lands directly below the operator.
    fn op_query(&mut self, x: usize, y: usize) {
        let dx = self.read_value(x, y, -3, 0);
        let dy = self.read_value(x, y, -2, 0);
        let len = self.read_value(x, y, -1, 0).max(1);
        for offset in 0..len {
            let glyph = self.read_port(x, y, dx + offset + 1, dy);
            self.write_port(x, y, offset - len + 1, 1, glyph, false);
        }
    }

    /// Track: locks the `len` cells east on its own row and outputs the
    /// glyph at index `key mod len`, verbatim, below.
    fn op_track(&mut self, x: usize, y: usize) {
        let key = self.read_value(x, y, -2, 0);
        let len = self.read_value(x, y, -1, 0).max(1);
        for offset in 0..len {
            self.lock_offset(x, y, offset + 1, 0);
        }
        let glyph = self.port_glyph(x, y, (key % len) + 1, 0);
        self.write_port(x, y, 0, 1, glyph, false);
    }

    /// Write (`X`): writes the value operand, verbatim, at relative
    /// `{x, y + 1}`.
    fn op_write(&mut self, x: usize, y: usize) {
        let dx = self.read_value(x, y, -2, 0);
        let dy = self.read_value(x, y, -1, 0) + 1;
        let value = self.read_port(x, y, 1, 0);
        self.write_port(x, y, dx, dy, value, false);
    }

    /// Jumper (`J`): the head of a column of jumpers copies the glyph above
    /// it, verbatim, to below the last consecutive jumper; a jumper directly
    /// below a `J` is a dormant chain body (orca-js compares against the
    /// literal uppercase glyph).
    fn op_jumper(&mut self, x: usize, y: usize, glyph: char) {
        let north = self.port_glyph(x, y, 0, -1);
        if north == Some('J') {
            return;
        }
        self.lock_offset(x, y, 0, -1);
        let mut dy = 1;
        while self.port_glyph(x, y, 0, dy) == Some(glyph) {
            dy += 1;
        }
        self.write_port(x, y, 0, dy, north, false);
    }

    /// Jymper (`Y`): the horizontal analog of [`Self::op_jumper`] — copies
    /// the glyph west of the chain head to east of the last consecutive
    /// jymper.
    fn op_jymper(&mut self, x: usize, y: usize, glyph: char) {
        let west = self.port_glyph(x, y, -1, 0);
        if west == Some('Y') {
            return;
        }
        self.lock_offset(x, y, -1, 0);
        let mut dx = 1;
        while self.port_glyph(x, y, dx, 0) == Some(glyph) {
            dx += 1;
        }
        self.write_port(x, y, dx, 0, west, false);
    }

    /// Comment (`#`): locks every cell east on its own row up to and
    /// including the matching `#` (or to the row end when unmatched), plus
    /// itself, turning the whole span into inert data (reference
    /// `library.js` `OperatorComment`). Locked glyphs never execute, so a
    /// commented `*` also never self-erases — though, matching the
    /// reference, it still reads as a bang neighbor for unlocked operators
    /// on adjacent rows.
    fn op_comment(&mut self, x: usize, y: usize) {
        for cx in (x + 1)..self.grid.width() {
            self.lock(cx, y);
            if self.grid.glyph_at(cx, y) == Some(COMMENT) {
                break;
            }
        }
        self.lock(x, y);
    }

    /// MIDI note output (`:` polyphonic, `%` monophonic). Port layout east
    /// of the operator: channel `{1,0}` (aborts above 15), octave `{2,0}`
    /// (clamp 0-8), note `{3,0}` (must not be empty or a digit), velocity
    /// `{4,0}` (default `f`, clamp 0-16), length `{5,0}` (default `1`, clamp
    /// 0-32). All five ports are locked on every frame the operator runs —
    /// the reference locks ports unconditionally after `operation()` — but
    /// an event is only emitted with a `*` in a cardinal neighbor.
    fn op_midi(&mut self, x: usize, y: usize, mono: bool) {
        let channel_glyph = self.read_and_lock(x, y, 1, 0);
        let octave_glyph = self.read_and_lock(x, y, 2, 0);
        let note = self.read_and_lock(x, y, 3, 0);
        let velocity_glyph = self.read_and_lock(x, y, 4, 0);
        let length_glyph = self.read_and_lock(x, y, 5, 0);
        if !self.has_bang_neighbor(x, y) {
            return;
        }
        if channel_glyph == EMPTY || octave_glyph == EMPTY || note == EMPTY {
            return;
        }
        if note.is_ascii_digit() {
            return;
        }
        let channel = value_of(channel_glyph);
        if channel > 15 {
            return;
        }
        let payload = MidiNote {
            channel: to_u8(channel),
            octave: to_u8(value_of(octave_glyph).min(8)),
            note,
            velocity: to_u8(value_of(defaulted(velocity_glyph, 'f')).min(16)),
            length: to_u8(value_of(defaulted(length_glyph, '1')).min(32)),
        };
        let io = if mono {
            OrcaIoEvent::MidiMono(payload)
        } else {
            OrcaIoEvent::Midi(payload)
        };
        self.emit(x, y, io);
    }

    /// MIDI control change (`!`). Ports east: channel `{1,0}` (aborts above
    /// 15), knob `{2,0}`, value `{3,0}` scaled to 0-127. Channel and knob
    /// must be non-empty; an empty value reads as 0.
    fn op_cc(&mut self, x: usize, y: usize) {
        let channel_glyph = self.read_and_lock(x, y, 1, 0);
        let knob_glyph = self.read_and_lock(x, y, 2, 0);
        let value_glyph = self.read_and_lock(x, y, 3, 0);
        if !self.has_bang_neighbor(x, y) {
            return;
        }
        if channel_glyph == EMPTY || knob_glyph == EMPTY {
            return;
        }
        let channel = value_of(channel_glyph);
        if channel > 15 {
            return;
        }
        self.emit(
            x,
            y,
            OrcaIoEvent::MidiCc {
                channel: to_u8(channel),
                knob: to_u8(value_of(knob_glyph)),
                value: scale_to_127(value_of(value_glyph)),
            },
        );
    }

    /// MIDI pitch bend (`?`). Ports east: channel `{1,0}` clamped to 0-15
    /// (no abort), lsb `{2,0}` and msb `{3,0}` each scaled to 0-127. Channel
    /// and lsb must be non-empty; an empty msb reads as 0.
    fn op_pb(&mut self, x: usize, y: usize) {
        let channel_glyph = self.read_and_lock(x, y, 1, 0);
        let lsb_glyph = self.read_and_lock(x, y, 2, 0);
        let msb_glyph = self.read_and_lock(x, y, 3, 0);
        if !self.has_bang_neighbor(x, y) {
            return;
        }
        if channel_glyph == EMPTY || lsb_glyph == EMPTY {
            return;
        }
        self.emit(
            x,
            y,
            OrcaIoEvent::MidiPb {
                channel: to_u8(value_of(channel_glyph).min(15)),
                lsb: scale_to_127(value_of(lsb_glyph)),
                msb: scale_to_127(value_of(msb_glyph)),
            },
        );
    }

    /// UDP output (`;`): concatenates (and locks) the eastward glyphs up to
    /// the first empty cell and emits them as a message when banged. The
    /// reference has no empty-message guard, so an empty message still emits.
    fn op_udp(&mut self, x: usize, y: usize) {
        let message = self.read_message(x, y, 1);
        if !self.has_bang_neighbor(x, y) {
            return;
        }
        self.emit(x, y, OrcaIoEvent::Udp(message));
    }

    /// OSC output (`=`): the path is the single glyph east (`{1,0}`), the
    /// args are the glyphs from `{2,0}` east up to the first empty cell. No
    /// event without a path.
    fn op_osc(&mut self, x: usize, y: usize) {
        let args = self.read_message(x, y, 2);
        let path = self.read_and_lock(x, y, 1, 0);
        if !self.has_bang_neighbor(x, y) {
            return;
        }
        if path == EMPTY {
            return;
        }
        self.emit(x, y, OrcaIoEvent::Osc { path, args });
    }

    /// Self command (`$`): reads (and locks) the eastward glyphs up to the
    /// first empty cell and emits them as a command string when banged.
    /// Unlike `;`, an empty command emits nothing. Command interpretation
    /// (e.g. orca-js `bpm`/`frame`) is a host concern, not an engine one.
    fn op_self(&mut self, x: usize, y: usize) {
        let command = self.read_message(x, y, 1);
        if !self.has_bang_neighbor(x, y) {
            return;
        }
        if command.is_empty() {
            return;
        }
        self.emit(x, y, OrcaIoEvent::Command(command));
    }

    /// Reads and locks the eastward message cells starting at offset
    /// `start`, stopping at the first empty cell, the grid edge, or 36
    /// glyphs (the reference IO message loop).
    fn read_message(&mut self, x: usize, y: usize, start: i64) -> String {
        let mut message = String::new();
        for dx in start..=36 {
            let Some((px, py)) = self.offset(x, y, dx, 0) else {
                break;
            };
            self.lock(px, py);
            let glyph = self.grid.glyph_at(px, py).unwrap_or(EMPTY);
            if glyph == EMPTY {
                break;
            }
            message.push(glyph);
        }
        message
    }

    /// Records an IO event fired by the operator at `(x, y)` this frame.
    fn emit(&mut self, x: usize, y: usize, io: OrcaIoEvent) {
        self.events.push(OrcaEvent {
            frame: self.frame,
            x,
            y,
            io,
        });
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

    /// The glyph at a signed offset from the operator, without locking;
    /// `None` out of bounds.
    fn port_glyph(&self, x: usize, y: usize, dx: i64, dy: i64) -> Option<char> {
        self.offset(x, y, dx, dy)
            .and_then(|(px, py)| self.grid.glyph_at(px, py))
    }

    /// Reads an operand port and locks its cell so the glyph acts as data
    /// instead of executing later this frame; `None` out of bounds.
    fn read_port(&mut self, x: usize, y: usize, dx: i64, dy: i64) -> Option<char> {
        let (px, py) = self.offset(x, y, dx, dy)?;
        self.lock(px, py);
        self.grid.glyph_at(px, py)
    }

    /// [`Self::read_port`] with out-of-bounds ports reading as empty.
    fn read_and_lock(&mut self, x: usize, y: usize, dx: i64, dy: i64) -> char {
        self.read_port(x, y, dx, dy).unwrap_or(EMPTY)
    }

    /// Reads an operand port as a base-36 value, for offset arithmetic.
    fn read_value(&mut self, x: usize, y: usize, dx: i64, dy: i64) -> i64 {
        to_i64(value_of(self.read_and_lock(x, y, dx, dy)))
    }

    /// Locks the cell at a signed offset from the operator, if in bounds.
    fn lock_offset(&mut self, x: usize, y: usize, dx: i64, dy: i64) {
        if let Some((px, py)) = self.offset(x, y, dx, dy) {
            self.lock(px, py);
        }
    }

    /// Writes a value-port output at a signed offset from the operator. The
    /// output cell is locked even when nothing is written (out-of-bounds
    /// inputs produce no write, matching orca-js). A `sensitive` output
    /// applies Orca's case rule: uppercased iff the glyph east of the
    /// operator is an uppercase letter; non-sensitive outputs are copied
    /// verbatim.
    fn write_port(
        &mut self,
        x: usize,
        y: usize,
        dx: i64,
        dy: i64,
        output: Option<char>,
        sensitive: bool,
    ) {
        let Some((ox, oy)) = self.offset(x, y, dx, dy) else {
            return;
        };
        self.lock(ox, oy);
        let Some(glyph) = output else {
            return;
        };
        let uppercase = sensitive
            && self
                .port_glyph(x, y, 1, 0)
                .is_some_and(|right| right.is_ascii_uppercase());
        let glyph = if uppercase {
            glyph.to_ascii_uppercase()
        } else {
            glyph
        };
        self.grid.set(ox, oy, glyph);
    }

    /// Writes a bang-port output below the operator: `*` when firing, `.`
    /// otherwise, locking the cell (matching orca-js `bang()`).
    fn bang_below(&mut self, x: usize, y: usize, fires: bool) {
        if let Some((ox, oy)) = self.offset(x, y, 0, 1) {
            self.grid.set(ox, oy, if fires { BANG } else { EMPTY });
            self.lock(ox, oy);
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
