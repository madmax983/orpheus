//! Sega Genesis / Mega Drive YM2612 (OPN2) four-operator FM synthesis leaf node.
//!
//! Ported from `madmax983/genesoxide`
//! `crates/genesoxide-core/src/ym2612.rs` (branch `trunk`): the operator kernel
//! (log-sine → attenuation-add → exp), the rate-scaled ADSR envelope generator
//! with its coarse rate grid and key-scaling, SSG-EG, the 8 algorithm
//! topologies, op-1 feedback, and the global LFO → PMS/AMS path. That core is
//! cross-validated against `ymfm` upstream, so the timbre is measured rather
//! than guessed. All host/bus plumbing (the register/MMIO decode, 6-channel
//! multiplexing, timers, CH3 special mode, DAC channel-6 PCM, and the
//! desktop/post-mix layers) is intentionally dropped — the graph instantiates
//! one channel's worth of FM math per voice and drives key-on/off from the
//! voice gate (ADR 0004, mirroring `chiptune.rs`).
//!
//! Two things genesoxide lacks are added here (design doc §2.6/§3.3):
//!   * the **ladder effect** — the discrete Model-1 9-bit DAC crossover
//!     distortion, authored fresh from the Nuked-OPN2 `OPN2_ChOutput` constants;
//!   * a **headroom-safe single-voice normalization** so one voice never clips
//!     Orpheus's output (genesoxide's `FM_SCALE` targets a 6-channel RMS match).
//!
//! ## Table provenance (zero-copyleft)
//!
//! The log-sine and exp ROMs are, per genesoxide's own comments, Nuked-OPN2
//! derived. Rather than copy those (LGPL-provenance) arrays verbatim, this
//! module **regenerates them from the public log-sine / exp formulas** at first
//! use (see [`FmTables`]). The regenerated tables reproduce the hardware ROM
//! exactly (verified in `graph_fm.rs`), keeping Orpheus's tree unambiguously
//! permissive. The formulas are documented at the generator.

// The ported YM2612 kernel has many small pure methods; `const`-ness is
// meaningless for these runtime hot-path helpers, so the nursery
// `missing_const_for_fn` suggestions are silenced module-wide.
#![allow(clippy::missing_const_for_fn)]

use std::sync::LazyLock;

use super::node::Node;

// ---------------------------------------------------------------------------
// Regenerated ROM tables (from public formulas — not copied from Nuked-OPN2)
// ---------------------------------------------------------------------------

/// The YM2612 log-sine and exp ROMs, regenerated from first principles.
///
/// * Log-sine (256-entry quarter wave, log/attenuation domain):
///   `sin[i] = round(-log2(sin((i + 0.5) * PI / 512)) * 256)`.
/// * Exp (256-entry, log→linear):
///   `exp[i] = round(2^((255 - i) / 256) * 1024)`.
///
/// These are the documented Yamaha OPN2 ROM formulas; evaluating them yields the
/// exact hardware ROM contents (pinned in tests), so no copyleft array is copied.
#[derive(Debug)]
pub struct FmTables {
    sin: [u16; 256],
    exp: [u16; 256],
}

impl FmTables {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn generate() -> Self {
        let mut sin = [0u16; 256];
        let mut exp = [0u16; 256];
        for i in 0..256usize {
            // Quarter-wave log-sine ROM.
            let x = (i as f64 + 0.5) * std::f64::consts::PI / 512.0;
            sin[i] = (-(x.sin().log2()) * 256.0).round() as u16;
            // Exp ROM (stored with the mantissa pre-inverted, matching the
            // runtime `exp[atten & 0xFF]` lookup).
            exp[i] = (((255 - i) as f64 / 256.0).exp2() * 1024.0).round() as u16;
        }
        Self { sin, exp }
    }
}

/// Process-wide singleton of the regenerated ROMs. Generated once (forced during
/// node construction, never on the audio path) and shared by reference, so
/// `process()` only ever reads a `&'static` — allocation- and lock-free.
static FM_TABLES: LazyLock<FmTables> = LazyLock::new(FmTables::generate);

// ---------------------------------------------------------------------------
// Constant tables (ported from genesoxide `ym2612.rs`; hardware/MAME reference)
// ---------------------------------------------------------------------------

/// Frequency-multiple table. `MUL = 0` means ×0.5; values are pre-doubled so the
/// multiply is integer and the `>> 1` in `phase_increment` recovers the ratio.
const MULTIPLY_TABLE: [u8; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30];

/// Hardware detune table (MAME `fm.c`), indexed by `[detune & 3][key_code]`.
/// Detune magnitudes 4..7 negate the offset (bit 2 = sign).
#[rustfmt::skip]
const DETUNE_TABLE: [[i32; 32]; 4] = [
    [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [ 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2,
      2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 8, 8],
    [ 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5,
      5, 6, 6, 7, 8, 8, 9,10,11,12,13,14,16,16,16,16],
    [ 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7,
      8, 8, 9,10,11,12,13,14,16,17,19,20,22,22,22,22],
];

/// LFO divider cycles (Nuked-OPN2), indexed by the LFO frequency register (0..7).
const LFO_CYCLES: [u32; 8] = [108, 77, 71, 67, 62, 44, 8, 5];

/// PM (vibrato) shift tables (Nuked-OPN2 `ym3438.c`), `[pms][pm_level]`.
#[rustfmt::skip]
const PG_LFO_SH1: [[u8; 8]; 8] = [
    [7, 7, 7, 7, 7, 7, 7, 7],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [7, 7, 7, 7, 7, 7, 1, 1],
    [7, 7, 7, 7, 1, 1, 1, 1],
    [7, 7, 7, 1, 1, 1, 1, 0],
    [7, 7, 1, 1, 0, 0, 0, 0],
    [7, 7, 1, 1, 0, 0, 0, 0],
    [7, 7, 1, 1, 0, 0, 0, 0],
];

#[rustfmt::skip]
const PG_LFO_SH2: [[u8; 8]; 8] = [
    [7, 7, 7, 7, 7, 7, 7, 7],
    [7, 7, 7, 7, 2, 2, 2, 2],
    [7, 7, 7, 2, 2, 2, 7, 7],
    [7, 7, 2, 2, 7, 7, 2, 2],
    [7, 7, 2, 7, 7, 7, 2, 7],
    [7, 7, 7, 2, 7, 7, 2, 1],
    [7, 7, 7, 2, 7, 7, 2, 1],
    [7, 7, 7, 2, 7, 7, 2, 1],
];

/// AM (tremolo) shift table (Nuked-OPN2), indexed by AMS (0..3).
const EG_AM_SHIFT: [u8; 4] = [7, 3, 1, 0];

/// EG rate shift table (MAME `fm.c`), indexed by `effective_rate >> 2` (0..15).
const EG_RATE_SHIFT: [u8; 16] = [11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 0];

/// MAME-reference EG increment patterns (19 rows of 8).
#[rustfmt::skip]
const EG_INC: [[u8; 8]; 19] = [
    [0,1, 0,1, 0,1, 0,1],
    [0,1, 0,1, 1,1, 0,1],
    [0,1, 1,1, 0,1, 1,1],
    [0,1, 1,1, 1,1, 1,1],
    [1,1, 1,1, 1,1, 1,1],
    [1,1, 1,2, 1,1, 1,2],
    [1,2, 1,2, 1,2, 1,2],
    [1,2, 2,2, 1,2, 2,2],
    [2,2, 2,2, 2,2, 2,2],
    [2,2, 2,4, 2,2, 2,4],
    [2,4, 2,4, 2,4, 2,4],
    [2,4, 4,4, 2,4, 4,4],
    [4,4, 4,4, 4,4, 4,4],
    [4,4, 4,8, 4,4, 4,8],
    [4,8, 4,8, 4,8, 4,8],
    [4,8, 8,8, 4,8, 8,8],
    [8,8, 8,8, 8,8, 8,8],
    [16,16,16,16,16,16,16,16],
    [0,0, 0,0, 0,0, 0,0],
];

/// Native YM2612 output rate (Hz). Used only to derive the envelope/LFO tick
/// cadence so envelope *times* and vibrato *speed* stay authentic regardless of
/// the graph's actual `sample_rate_hz`. Pitch is driven directly from `freq_hz`.
const FM_NATIVE_RATE_HZ: f32 = 53_267.0;

/// The 9-bit multiplexed-DAC clip bound applied once to the summed carriers.
const DAC_CLIP: i32 = 256;

/// Right-shift applied to the carrier sum before the DAC clip (reduces ~11-bit
/// operator output toward the 9-bit DAC range), matching genesoxide/ymfm.
const DAC_RSHIFT: i32 = 5;

/// Single-voice normalization. The clamped carrier sum spans ±`DAC_CLIP`
/// (± a couple LSB after the ladder); dividing by 288 maps that to ≈ ±0.89,
/// leaving comfortable headroom so one voice never approaches the ±1.0 rails.
/// This replaces genesoxide's 6-channel `FM_SCALE`, which is tuned for a summed
/// six-voice RMS match, not single-voice peak safety (design §3.3/§3.4).
const SINGLE_VOICE_SCALE: f32 = 1.0 / 288.0;

/// TL swing (attenuation units) spanned by the `bright` macro at its extremes.
const BRIGHT_TL_RANGE: f32 = 48.0;

/// Falls back to 48 kHz for non-finite or non-positive sample rates, mirroring
/// the other leaf nodes.
fn sanitize_sample_rate(sample_rate_hz: f32) -> f32 {
    if sample_rate_hz.is_finite() && sample_rate_hz > 0.0 {
        sample_rate_hz
    } else {
        48_000.0
    }
}

/// Convert a real frequency in Hz to the chip's `(fnum, block)` pair for the
/// graph's `sample_rate_hz`, mirroring how a Genesis music driver picks a note.
///
/// The operator phase step for `MUL=1`/`DT=0` is `fnum * 2^(block - 1)`, applied
/// to a 2^20-period phase accumulator, so the emitted fundamental is
/// `step * sample_rate_hz / 2^20`. Solving for `step = freq * 2^20 / SR` and
/// normalizing `fnum` into `[1024, 2048)` reproduces the note pitch to sub-cent
/// accuracy while keeping the detune-table and key-scaling lookups (which are
/// indexed by `key_code(fnum, block)`) authentic.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn freq_to_fnum_block(freq_hz: f32, sample_rate_hz: f32) -> (u16, u8) {
    if !(freq_hz.is_finite() && freq_hz > 0.0) {
        return (0, 0);
    }
    let target = f64::from(freq_hz) * f64::from(1u32 << 20) / f64::from(sample_rate_hz);
    let mut fnum = target;
    let mut block: i32 = 1;
    while fnum >= 2048.0 && block < 7 {
        fnum *= 0.5;
        block += 1;
    }
    while fnum < 1024.0 && block > 0 {
        fnum *= 2.0;
        block -= 1;
    }
    let fnum = fnum.round().clamp(0.0, 2047.0) as u16;
    (fnum, block as u8)
}

// ---------------------------------------------------------------------------
// Public patch API (config-struct patch — see ADR 0015)
// ---------------------------------------------------------------------------

/// A single FM operator's timbre configuration.
///
/// All fields are the raw YM2612 register ranges; `total_level` and the envelope
/// rates are the timbre-defining constants that make FM sound like FM, so they
/// travel in the patch rather than as audio-rate signal inputs (ADR 0015).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FmOp {
    /// Frequency multiple, 0..=15 (`0` ⇒ ×0.5).
    pub mul: u8,
    /// Detune, 0..=7 (bit 2 = sign).
    pub detune: u8,
    /// Total level, 0..=127 (`0` = loudest; on modulators this is FM index).
    pub total_level: u8,
    /// Attack rate, 0..=31.
    pub ar: u8,
    /// First decay rate (D1R), 0..=31.
    pub d1r: u8,
    /// Sustain level (SL), 0..=15.
    pub sl: u8,
    /// Second decay / sustain rate (D2R), 0..=31 (`0` = hold).
    pub d2r: u8,
    /// Release rate, 0..=15.
    pub rr: u8,
    /// Rate scaling / key scale, 0..=3 (higher notes ⇒ faster envelopes).
    pub rate_scale: u8,
    /// SSG-EG mode, 0..=15 (`0` = off).
    pub ssg_eg: u8,
    /// Whether the LFO AM (tremolo) applies to this operator.
    pub am_on: bool,
}

impl FmOp {
    /// A silent operator at maximum attenuation (the power-on default).
    #[must_use]
    pub const fn silent() -> Self {
        Self {
            mul: 1,
            detune: 0,
            total_level: 127,
            ar: 31,
            d1r: 0,
            sl: 0,
            d2r: 0,
            rr: 7,
            rate_scale: 0,
            ssg_eg: 0,
            am_on: false,
        }
    }
}

/// A complete four-operator FM voice patch.
///
/// This is a construction-time config struct rather than a bank of signal inputs
/// (a documented deviation from ADR 0004; see ADR 0015): an FM voice has ~35
/// timbre parameters that are constants, not per-sample modulation. The small
/// ergonomic surface — `gate`, `freq_hz`, `bright`, `fb` — stays as signal
/// inputs on [`FmGenesisNode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FmPatch {
    /// Operator topology, 0..=7 (design doc §2.3).
    pub algorithm: u8,
    /// Op-1 self-feedback amount, 0..=7 (`0` = none).
    pub feedback: u8,
    /// The four operators, in Sega/MAME order `1, 2, 3, 4`.
    pub ops: [FmOp; 4],
    /// LFO frequency index, 0..=7. The LFO runs whenever `pms > 0 || ams > 0`.
    pub lfo_rate: u8,
    /// Phase-mod sensitivity (vibrato depth), 0..=7.
    pub pms: u8,
    /// Amplitude-mod sensitivity (tremolo depth), 0..=3.
    pub ams: u8,
    /// Enable the Model-1 9-bit DAC crossover "ladder" grit (default on).
    pub ladder: bool,
}

impl FmPatch {
    /// Returns the bitmask of carrier operators for a given algorithm; bit `i`
    /// set means operator `i` (0-based) is summed to the output. Modulators are
    /// the complement, and are what the `bright` macro scales.
    #[must_use]
    const fn carrier_mask(algorithm: u8) -> u8 {
        match algorithm {
            0..=3 => 0b1000, // op4 only
            4 => 0b1010,     // op2, op4
            5 | 6 => 0b1110, // op2, op3, op4
            _ => 0b1111,     // algo 7: all four
        }
    }
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

/// DX-style electric piano: op1 fans out (ALG 5) with a bright detuned partial
/// and mild feedback for the classic glassy tine shimmer.
#[must_use]
pub fn epiano() -> FmPatch {
    FmPatch {
        algorithm: 5,
        feedback: 4,
        ops: [
            FmOp {
                mul: 1,
                detune: 3,
                total_level: 38,
                ar: 31,
                d1r: 12,
                sl: 3,
                d2r: 6,
                rr: 6,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 12,
                ar: 31,
                d1r: 8,
                sl: 2,
                d2r: 4,
                rr: 6,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 4,
                total_level: 20,
                ar: 31,
                d1r: 10,
                sl: 2,
                d2r: 5,
                rr: 6,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 14,
                detune: 0,
                total_level: 30,
                ar: 31,
                d1r: 18,
                sl: 4,
                d2r: 7,
                rr: 7,
                rate_scale: 2,
                ssg_eg: 0,
                am_on: false,
            },
        ],
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: true,
    }
}

/// Rubbery FM slap bass: two parallel 2-op stacks (ALG 4) with high feedback and
/// a fast, short envelope.
#[must_use]
pub fn ebass() -> FmPatch {
    FmPatch {
        algorithm: 4,
        feedback: 6,
        ops: [
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 34,
                ar: 31,
                d1r: 14,
                sl: 4,
                d2r: 8,
                rr: 8,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 6,
                ar: 31,
                d1r: 16,
                sl: 3,
                d2r: 6,
                rr: 8,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 3,
                detune: 4,
                total_level: 40,
                ar: 31,
                d1r: 16,
                sl: 4,
                d2r: 8,
                rr: 8,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 8,
                ar: 31,
                d1r: 16,
                sl: 3,
                d2r: 6,
                rr: 8,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
        ],
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: true,
    }
}

/// Buzzy brass stab: op2→op3→op4 with op1 also into op4 (ALG 2), bright
/// modulators, LFO vibrato, and a slightly soft attack.
#[must_use]
pub fn brass() -> FmPatch {
    FmPatch {
        algorithm: 2,
        feedback: 3,
        ops: [
            FmOp {
                mul: 1,
                detune: 3,
                total_level: 32,
                ar: 24,
                d1r: 8,
                sl: 2,
                d2r: 0,
                rr: 7,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: true,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 30,
                ar: 24,
                d1r: 8,
                sl: 2,
                d2r: 0,
                rr: 7,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 4,
                total_level: 28,
                ar: 24,
                d1r: 8,
                sl: 2,
                d2r: 0,
                rr: 7,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 8,
                ar: 26,
                d1r: 6,
                sl: 1,
                d2r: 0,
                rr: 7,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: true,
            },
        ],
        lfo_rate: 3,
        pms: 4,
        ams: 1,
        ladder: true,
    }
}

/// Cutting square-ish lead: serial chain (ALG 0) at high FM index with LFO
/// vibrato.
#[must_use]
pub fn lead() -> FmPatch {
    FmPatch {
        algorithm: 0,
        feedback: 5,
        ops: [
            FmOp {
                mul: 2,
                detune: 3,
                total_level: 30,
                ar: 31,
                d1r: 6,
                sl: 1,
                d2r: 0,
                rr: 8,
                rate_scale: 0,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 28,
                ar: 31,
                d1r: 6,
                sl: 1,
                d2r: 0,
                rr: 8,
                rate_scale: 0,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 4,
                total_level: 26,
                ar: 31,
                d1r: 6,
                sl: 1,
                d2r: 0,
                rr: 8,
                rate_scale: 0,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 4,
                ar: 31,
                d1r: 4,
                sl: 0,
                d2r: 0,
                rr: 8,
                rate_scale: 0,
                ssg_eg: 0,
                am_on: false,
            },
        ],
        lfo_rate: 4,
        pms: 3,
        ams: 0,
        ladder: true,
    }
}

/// Metallic bell / chime: serial chain (ALG 0) with an inharmonic modulator MUL
/// ratio and a long release.
#[must_use]
pub fn bell() -> FmPatch {
    FmPatch {
        algorithm: 0,
        feedback: 2,
        ops: [
            FmOp {
                mul: 7,
                detune: 1,
                total_level: 34,
                ar: 31,
                d1r: 12,
                sl: 3,
                d2r: 4,
                rr: 3,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 2,
                detune: 0,
                total_level: 24,
                ar: 31,
                d1r: 12,
                sl: 3,
                d2r: 4,
                rr: 3,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 14,
                detune: 5,
                total_level: 30,
                ar: 31,
                d1r: 12,
                sl: 3,
                d2r: 4,
                rr: 3,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 1,
                detune: 0,
                total_level: 6,
                ar: 31,
                d1r: 10,
                sl: 2,
                d2r: 3,
                rr: 2,
                rate_scale: 1,
                ssg_eg: 0,
                am_on: false,
            },
        ],
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: true,
    }
}

/// Genesis percussion: full-additive (ALG 7) with SSG-EG buzz, high feedback,
/// and a very short envelope.
#[must_use]
pub fn drum() -> FmPatch {
    FmPatch {
        algorithm: 7,
        feedback: 7,
        ops: [
            FmOp {
                mul: 0,
                detune: 0,
                total_level: 10,
                ar: 31,
                d1r: 20,
                sl: 6,
                d2r: 20,
                rr: 12,
                rate_scale: 2,
                ssg_eg: 0b1100,
                am_on: false,
            },
            FmOp {
                mul: 8,
                detune: 6,
                total_level: 18,
                ar: 31,
                d1r: 22,
                sl: 8,
                d2r: 22,
                rr: 12,
                rate_scale: 2,
                ssg_eg: 0b1110,
                am_on: false,
            },
            FmOp {
                mul: 12,
                detune: 2,
                total_level: 20,
                ar: 31,
                d1r: 22,
                sl: 8,
                d2r: 22,
                rr: 12,
                rate_scale: 2,
                ssg_eg: 0,
                am_on: false,
            },
            FmOp {
                mul: 15,
                detune: 7,
                total_level: 16,
                ar: 31,
                d1r: 24,
                sl: 10,
                d2r: 24,
                rr: 12,
                rate_scale: 2,
                ssg_eg: 0,
                am_on: false,
            },
        ],
        lfo_rate: 0,
        pms: 0,
        ams: 0,
        ladder: true,
    }
}

/// Look up a named preset. Returns `None` for an unknown name; the six known
/// names are `epiano`, `ebass`, `brass`, `lead`, `bell`, `drum`.
#[must_use]
pub fn preset_by_name(name: &str) -> Option<FmPatch> {
    match name {
        "epiano" => Some(epiano()),
        "ebass" => Some(ebass()),
        "brass" => Some(brass()),
        "lead" => Some(lead()),
        "bell" => Some(bell()),
        "drum" => Some(drum()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Internal envelope state + operator (ported from genesoxide `ym2612.rs`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnvState {
    Attack,
    Decay,
    Sustain,
    Release,
}

/// One FM operator: phase generator + rate-scaled envelope generator. Ported
/// near-verbatim from genesoxide `Operator`, dropping only the register plumbing.
#[derive(Clone, Debug)]
struct Operator {
    phase: u32,
    envelope: u16,
    env_state: EnvState,
    /// The `bright`-adjusted effective total level used this sample.
    total_level: u8,
    /// The patch's authored total level (the `bright` baseline).
    base_total_level: u8,
    sustain_level: u8,
    attack_rate: u8,
    decay_rate: u8,
    sustain_rate: u8,
    release_rate: u8,
    multiply: u8,
    detune: u8,
    key_scale: u8,
    key_on: bool,
    am_enable: bool,
    ssg_eg: u8,
    ssg_output_invert: bool,
    output: i32,
    prev_output: i32,
}

impl Operator {
    fn from_config(op: FmOp) -> Self {
        Self {
            phase: 0,
            envelope: 1023,
            env_state: EnvState::Release,
            total_level: op.total_level,
            base_total_level: op.total_level,
            sustain_level: op.sl,
            attack_rate: op.ar,
            decay_rate: op.d1r,
            sustain_rate: op.d2r,
            release_rate: op.rr,
            multiply: op.mul,
            detune: op.detune,
            key_scale: op.rate_scale,
            key_on: false,
            am_enable: op.am_on,
            ssg_eg: op.ssg_eg,
            ssg_output_invert: false,
            output: 0,
            prev_output: 0,
        }
    }

    fn reset(&mut self) {
        self.phase = 0;
        self.envelope = 1023;
        self.env_state = EnvState::Release;
        self.total_level = self.base_total_level;
        self.key_on = false;
        self.ssg_output_invert = false;
        self.output = 0;
        self.prev_output = 0;
    }

    fn ssg_enabled(&self) -> bool {
        self.ssg_eg & 0x08 != 0
    }
    fn ssg_attack(&self) -> bool {
        self.ssg_eg & 0x04 != 0
    }

    fn key_on(&mut self, fnum: u16, block: u8) {
        if self.key_on {
            return;
        }
        self.key_on = true;
        self.phase = 0;
        let rate = self.effective_rate(self.attack_rate, fnum, block);
        if rate >= 62 {
            self.envelope = 0;
            self.env_state = EnvState::Decay;
        } else {
            self.env_state = EnvState::Attack;
        }
        self.ssg_output_invert = self.ssg_enabled() && self.ssg_attack();
    }

    fn key_off(&mut self) {
        if !self.key_on {
            return;
        }
        self.key_on = false;
        if self.ssg_enabled() && self.ssg_output_invert {
            self.envelope = 0x200u16.wrapping_sub(self.envelope) & 0x3FF;
            self.ssg_output_invert = false;
        }
        self.env_state = EnvState::Release;
    }

    fn ssg_clock(&mut self, fnum: u16, block: u8) {
        if self.envelope < 0x200 {
            return;
        }
        let mode = self.ssg_eg & 0x07;
        let attack = mode & 0x04 != 0;
        let alternate = mode & 0x02 != 0;
        let hold = mode & 0x01 != 0;

        if hold {
            self.ssg_output_invert = attack ^ alternate;
            if self.env_state != EnvState::Attack {
                self.envelope = if self.ssg_output_invert { 0x200 } else { 0x3FF };
            }
        } else {
            if alternate {
                self.ssg_output_invert = !self.ssg_output_invert;
            }
            if matches!(self.env_state, EnvState::Decay | EnvState::Sustain) {
                self.env_state = EnvState::Attack;
                let rate = self.effective_rate(self.attack_rate, fnum, block);
                if rate >= 62 {
                    self.envelope = 0;
                }
            }
            if !alternate {
                self.phase = 0;
            }
        }

        if self.env_state == EnvState::Release {
            self.envelope = 0x3FF;
        }
    }

    fn key_code(fnum: u16, block: u8) -> u8 {
        const OPN_FKTABLE: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 3, 3, 3, 3, 3, 3];
        let note = OPN_FKTABLE[((fnum >> 7) & 0x0F) as usize];
        (block << 2) | note
    }

    fn effective_rate(&self, base_rate: u8, fnum: u16, block: u8) -> u8 {
        if base_rate == 0 {
            return 0;
        }
        let kc = Self::key_code(fnum, block);
        let ks_shift = 3u8.saturating_sub(self.key_scale);
        let scaled = (base_rate * 2) + (kc >> ks_shift);
        scaled.min(63)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn eg_calc_increment(rate: u8, eg_counter: u32) -> u8 {
        if rate == 0 {
            return 0;
        }
        let rate = rate.min(63) as usize;
        let rate_high = rate >> 2;
        let rate_low = rate & 3;
        let shift = u32::from(EG_RATE_SHIFT[rate_high.min(15)]);
        if shift > 0 && (eg_counter & ((1u32 << shift) - 1)) != 0 {
            return 0;
        }
        let row = match rate_high {
            0..=11 => rate_low,
            12 => 4 + rate_low,
            13 => 8 + rate_low,
            14 => 12 + rate_low,
            _ => 16,
        };
        let step_idx = ((eg_counter >> shift) & 7) as usize;
        EG_INC[row][step_idx]
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn update_envelope(&mut self, fnum: u16, block: u8, eg_counter: u32) {
        if self.ssg_enabled() {
            self.ssg_clock(fnum, block);
        }
        match self.env_state {
            EnvState::Attack => {
                let rate = self.effective_rate(self.attack_rate, fnum, block);
                if rate >= 62 {
                    self.envelope = 0;
                    self.env_state = EnvState::Decay;
                    return;
                }
                let inc = Self::eg_calc_increment(rate, eg_counter);
                if inc > 0 && self.envelope > 0 {
                    let delta = (!(i32::from(self.envelope)) * i32::from(inc)) >> 4;
                    self.envelope = (i32::from(self.envelope) + delta).max(0) as u16;
                }
                if self.envelope == 0 {
                    self.env_state = EnvState::Decay;
                }
            }
            EnvState::Decay => {
                let rate = self.effective_rate(self.decay_rate, fnum, block);
                let inc = u16::from(Self::eg_calc_increment(rate, eg_counter));
                let inc = if self.ssg_enabled() && self.envelope < 0x200 {
                    inc * 4
                } else {
                    inc
                };
                self.envelope = (self.envelope + inc).min(1023);
                let sl_mapped = if self.sustain_level == 15 {
                    31u16
                } else {
                    u16::from(self.sustain_level)
                };
                let target = sl_mapped * 32;
                if self.envelope >= target {
                    self.env_state = EnvState::Sustain;
                }
            }
            EnvState::Sustain => {
                let rate = self.effective_rate(self.sustain_rate, fnum, block);
                let inc = u16::from(Self::eg_calc_increment(rate, eg_counter));
                let inc = if self.ssg_enabled() && self.envelope < 0x200 {
                    inc * 4
                } else {
                    inc
                };
                self.envelope = (self.envelope + inc).min(1023);
            }
            EnvState::Release => {
                let rate = self.effective_rate(self.release_rate * 2 + 1, fnum, block);
                let inc = u16::from(Self::eg_calc_increment(rate, eg_counter));
                self.envelope = (self.envelope + inc).min(1023);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn phase_increment(&self, fnum: u16, block: u8) -> u32 {
        let fnum_doubled = u32::from(fnum) << 1;
        let base_step = (fnum_doubled << u32::from(block)) >> 2;
        let dt_mag = (self.detune & 3) as usize;
        let kc = Self::key_code(fnum, block) as usize;
        let dt_offset = DETUNE_TABLE[dt_mag][kc.min(31)] as u32;
        let detuned = if self.detune & 4 != 0 {
            base_step.wrapping_sub(dt_offset)
        } else {
            base_step.wrapping_add(dt_offset)
        } & 0x1_FFFF;
        let mult = u32::from(MULTIPLY_TABLE[self.multiply as usize]);
        (detuned * mult) >> 1
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    fn compute(
        &mut self,
        tables: &FmTables,
        phase_inc: u32,
        modulation: i32,
        am_atten: u32,
    ) -> i32 {
        self.phase = self.phase.wrapping_add(phase_inc);
        let phase_10 = ((self.phase >> 10) as i32).wrapping_add(modulation) as u32;
        let sign = (phase_10 >> 9) & 1;
        let half = (phase_10 >> 8) & 1;
        let phase_8 = (phase_10 & 0xFF) as usize;
        let index = if half != 0 { 255 - phase_8 } else { phase_8 };
        let log_sin = tables.sin[index];

        let effective_envelope = if self.ssg_enabled() && self.ssg_output_invert {
            0x200u16.wrapping_sub(self.envelope) & 0x3FF
        } else {
            self.envelope
        };

        let am = if self.am_enable { am_atten } else { 0 };
        let atten = u32::from(log_sin)
            + u32::from(effective_envelope) * 4
            + u32::from(self.total_level) * 32
            + am;

        if atten >= 4096 {
            self.prev_output = self.output;
            self.output = 0;
            return 0;
        }

        let mantissa = (atten & 0xFF) as usize;
        let exponent = atten >> 8;
        let linear = (i32::from(tables.exp[mantissa]) << 2) >> exponent;
        let output = if sign != 0 { -linear } else { linear };
        self.prev_output = self.output;
        self.output = output;
        output
    }
}

// ---------------------------------------------------------------------------
// Internal channel (one voice's algorithm router + LFO PM)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Channel {
    operators: [Operator; 4],
    fnum: u16,
    block: u8,
    algorithm: u8,
    feedback: u8,
    pms: u8,
    ams: u8,
}

impl Channel {
    fn lfo_pm_offset(&self, pm_raw: u8) -> i32 {
        if self.pms == 0 {
            return 0;
        }
        let pms = self.pms as usize;
        let mut pm_l = (pm_raw & 0x0F) as usize;
        if pm_l & 0x08 != 0 {
            pm_l ^= 0x0F;
        }
        let sign = pm_raw & 0x10 != 0;
        let fnum_h = i32::from(self.fnum >> 4);
        let sh1 = i32::from(PG_LFO_SH1[pms][pm_l]);
        let sh2 = i32::from(PG_LFO_SH2[pms][pm_l]);
        let mut fm = (fnum_h >> sh1) + (fnum_h >> sh2);
        if pms > 5 {
            fm <<= pms - 5;
        }
        fm >>= 2;
        fm >>= 1;
        if sign { -fm } else { fm }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn output_sample(&mut self, tables: &FmTables, am_level: u8, pm_raw: u8) -> i32 {
        let am_atten = if self.ams > 0 {
            let shift = EG_AM_SHIFT[self.ams as usize];
            (u32::from(am_level) >> shift) * 4
        } else {
            0
        };
        let pm_offset = self.lfo_pm_offset(pm_raw);
        let fnum_pm = (i32::from(self.fnum) + pm_offset).clamp(0, 0x7FF) as u16;

        let ops = &mut self.operators;
        let inc0 = ops[0].phase_increment(fnum_pm, self.block);
        let inc1 = ops[1].phase_increment(fnum_pm, self.block);
        let inc2 = ops[2].phase_increment(fnum_pm, self.block);
        let inc3 = ops[3].phase_increment(fnum_pm, self.block);

        let fb_mod = if self.feedback > 0 {
            (ops[0].prev_output + ops[0].output) >> (10 - i32::from(self.feedback))
        } else {
            0
        };

        match self.algorithm {
            0 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, o1 >> 1, am_atten);
                let o3 = ops[2].compute(tables, inc2, o2 >> 1, am_atten);
                let o4 = ops[3].compute(tables, inc3, o3 >> 1, am_atten);
                (o4 >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            1 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, 0, am_atten);
                let o3 = ops[2].compute(tables, inc2, (o1 >> 1) + (o2 >> 1), am_atten);
                let o4 = ops[3].compute(tables, inc3, o3 >> 1, am_atten);
                (o4 >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            2 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, 0, am_atten);
                let o3 = ops[2].compute(tables, inc2, o2 >> 1, am_atten);
                let o4 = ops[3].compute(tables, inc3, (o1 >> 1) + (o3 >> 1), am_atten);
                (o4 >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            3 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, o1 >> 1, am_atten);
                let o3 = ops[2].compute(tables, inc2, 0, am_atten);
                let o4 = ops[3].compute(tables, inc3, (o2 >> 1) + (o3 >> 1), am_atten);
                (o4 >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            4 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, o1 >> 1, am_atten);
                let o3 = ops[2].compute(tables, inc2, 0, am_atten);
                let o4 = ops[3].compute(tables, inc3, o3 >> 1, am_atten);
                ((o2 + o4) >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            5 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, o1 >> 1, am_atten);
                let o3 = ops[2].compute(tables, inc2, o1 >> 1, am_atten);
                let o4 = ops[3].compute(tables, inc3, o1 >> 1, am_atten);
                ((o2 + o3 + o4) >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            6 => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, o1 >> 1, am_atten);
                let o3 = ops[2].compute(tables, inc2, 0, am_atten);
                let o4 = ops[3].compute(tables, inc3, 0, am_atten);
                ((o2 + o3 + o4) >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
            _ => {
                let o1 = ops[0].compute(tables, inc0, fb_mod, am_atten);
                let o2 = ops[1].compute(tables, inc1, 0, am_atten);
                let o3 = ops[2].compute(tables, inc2, 0, am_atten);
                let o4 = ops[3].compute(tables, inc3, 0, am_atten);
                ((o1 + o2 + o3 + o4) >> DAC_RSHIFT).clamp(-DAC_CLIP, DAC_CLIP)
            }
        }
    }
}

/// The Nuked-OPN2 Model-1 discrete-DAC crossover distortion ("ladder").
///
/// Nuked-OPN2 (`ym3438.c` `OPN2_ChOutput`) models the 9-bit multiplexed DAC as:
/// `sign = out >> 8; if (out >= 0) { out++; sign++; }` — the pin then swings
/// between the active sample `out` and the idle level `sign`, which is `-1`
/// below zero and `+1` at/above zero. That asymmetric idle step around zero is
/// the crossover grit (and the Genesis's tiny positive DC bias). For a single
/// mono voice we superimpose the idle bias on the active sample, reproducing the
/// `+1` LSB positive offset and the `-1`/`+1` dead-zone across zero. Absent from
/// genesoxide; added here per the design doc §2.6/§3.3.
///
/// A true-zero carrier sum stays zero (design doc: "zero stays zero"), so an
/// idle/ungated voice contributes no DC — only *sounding* samples get the grit.
fn apply_ladder(sample_9bit: i32) -> i32 {
    if sample_9bit == 0 {
        return 0;
    }
    let sign = sample_9bit >> 8; // -1 (negative) or 0 (non-negative)
    if sample_9bit > 0 {
        (sample_9bit + 1) + (sign + 1)
    } else {
        sample_9bit + sign
    }
}

// ---------------------------------------------------------------------------
// FmGenesisNode
// ---------------------------------------------------------------------------

/// A Sega Genesis / Mega Drive YM2612 four-operator FM voice.
///
/// **4 inputs** (`gate`, `freq_hz`, `bright`, `fb`), **1 output** (mono audio,
/// bipolar `f32` ≈ `[-1, 1]`):
///
/// * `gate` — key-on/off (`> 0.5` = on); a rising edge keys all four operator
///   envelopes on, a falling edge releases them.
/// * `freq_hz` — the note fundamental; each operator runs at `freq_hz * MUL (±DT)`.
/// * `bright` — FM-index macro, `≈ [0, 2]` (`1.0` = as authored); scales the
///   modulator operators' total level (brightness). Non-finite ⇒ `1.0`.
/// * `fb` — op-1 feedback, `[0, 1]` mapped to `0..=7`. Non-finite ⇒ the patch's
///   authored feedback.
///
/// The timbre (algorithm, per-op MUL/DT/TL/EG rates/SSG-EG, LFO/PMS/AMS,
/// `ladder`) is fixed at construction via [`FmPatch`] (ADR 0015).
#[derive(Clone, Debug)]
pub struct FmGenesisNode {
    channel: Channel,
    tables: &'static FmTables,
    sample_rate_hz: f32,
    ladder: bool,
    patch_feedback: u8,
    /// Bitmask of carrier operators (exempt from the `bright` macro).
    carrier_mask: u8,
    // LFO state.
    lfo_enabled: bool,
    lfo_frequency: u8,
    lfo_counter: u32,
    lfo_phase: u8,
    lfo_am_level: u8,
    lfo_pm_raw: u8,
    lfo_accum: f32,
    // Envelope-generator tick state.
    eg_counter: u32,
    eg_accum: f32,
    // Gate edge detection.
    prev_gate: bool,
}

impl FmGenesisNode {
    /// LFO ticks per output sample (the LFO clocks once per native FM sample).
    fn lfo_tick_per_sample(&self) -> f32 {
        FM_NATIVE_RATE_HZ / self.sample_rate_hz
    }

    /// EG ticks per output sample (the EG clocks at the native rate / 3).
    fn eg_tick_per_sample(&self) -> f32 {
        (FM_NATIVE_RATE_HZ / 3.0) / self.sample_rate_hz
    }

    /// Advance the LFO by one native tick, updating the cached AM/PM outputs.
    fn advance_lfo(&mut self) {
        if !self.lfo_enabled {
            self.lfo_am_level = 0;
            self.lfo_pm_raw = 0;
            return;
        }
        self.lfo_counter += 1;
        if self.lfo_counter >= LFO_CYCLES[self.lfo_frequency as usize] {
            self.lfo_counter = 0;
            self.lfo_phase = self.lfo_phase.wrapping_add(1) & 0x7F;
        }
        let phase = self.lfo_phase;
        self.lfo_am_level = if phase < 64 {
            phase * 2
        } else {
            (127 - phase) * 2
        };
        self.lfo_pm_raw = phase >> 2;
    }

    /// Step the 12-bit EG counter (skips 0 on overflow, per hardware).
    fn step_eg_counter(&mut self) {
        self.eg_counter += 1;
        self.eg_counter = (self.eg_counter & 0xFFF) + (self.eg_counter >> 12);
    }
}

impl Node for FmGenesisNode {
    fn inputs(&self) -> u32 {
        4
    }
    fn outputs(&self) -> u32 {
        1
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]], frames: usize) {
        let gate = inputs[0];
        let freq = inputs[1];
        let bright = inputs[2];
        let fb = inputs[3];
        let out = &mut outputs[0];

        for i in 0..frames {
            // ── Pitch: track freq_hz into the chip's (fnum, block) domain ──
            let f = freq[i];
            if f.is_finite() && f > 0.0 {
                let (fnum, block) = freq_to_fnum_block(f, self.sample_rate_hz);
                self.channel.fnum = fnum;
                self.channel.block = block;
            }

            // ── Gate edges → key-on / key-off all four operators ──
            let gate_on = gate[i].is_finite() && gate[i] > 0.5;
            if gate_on && !self.prev_gate {
                let (fnum, block) = (self.channel.fnum, self.channel.block);
                for op in &mut self.channel.operators {
                    op.key_on(fnum, block);
                }
            } else if !gate_on && self.prev_gate {
                for op in &mut self.channel.operators {
                    op.key_off();
                }
            }
            self.prev_gate = gate_on;

            // ── bright macro: shift the modulator operators' total level ──
            let bright_val = if bright[i].is_finite() {
                bright[i].clamp(0.0, 2.0)
            } else {
                1.0
            };
            let tl_delta = ((1.0 - bright_val) * BRIGHT_TL_RANGE).round() as i32;
            for (idx, op) in self.channel.operators.iter_mut().enumerate() {
                if self.carrier_mask & (1 << idx) != 0 {
                    op.total_level = op.base_total_level;
                } else {
                    op.total_level =
                        (i32::from(op.base_total_level) + tl_delta).clamp(0, 127) as u8;
                }
            }

            // ── fb: playable op-1 feedback ──
            self.channel.feedback = if fb[i].is_finite() {
                (fb[i].clamp(0.0, 1.0) * 7.0).round() as u8
            } else {
                self.patch_feedback
            };

            // ── LFO cadence (native rate) ──
            self.lfo_accum += self.lfo_tick_per_sample();
            let lfo_ticks = self.lfo_accum.floor();
            self.lfo_accum -= lfo_ticks;
            for _ in 0..(lfo_ticks as u32) {
                self.advance_lfo();
            }

            // ── EG cadence (native rate / 3): count ticks for this sample ──
            self.eg_accum += self.eg_tick_per_sample();
            let eg_ticks_f = self.eg_accum.floor();
            self.eg_accum -= eg_ticks_f;
            let eg_ticks = eg_ticks_f as u32;

            // ── Synthesize this sample with the current envelope ──
            let raw = self
                .channel
                .output_sample(self.tables, self.lfo_am_level, self.lfo_pm_raw);
            let shaped = if self.ladder { apply_ladder(raw) } else { raw };
            out[i] = shaped as f32 * SINGLE_VOICE_SCALE;

            // ── Advance envelopes for the ticks that elapsed ──
            for _ in 0..eg_ticks {
                self.step_eg_counter();
                let (fnum, block) = (self.channel.fnum, self.channel.block);
                let eg_counter = self.eg_counter;
                for op in &mut self.channel.operators {
                    op.update_envelope(fnum, block, eg_counter);
                }
            }
        }
    }

    fn reset(&mut self) {
        for op in &mut self.channel.operators {
            op.reset();
        }
        self.channel.feedback = self.patch_feedback;
        self.lfo_counter = 0;
        self.lfo_phase = 0;
        self.lfo_am_level = 0;
        self.lfo_pm_raw = 0;
        self.lfo_accum = 0.0;
        self.eg_counter = 0;
        self.eg_accum = 0.0;
        self.prev_gate = false;
    }
}

/// Creates a Sega Genesis YM2612 FM voice node from a patch.
/// 4 inputs (`gate`, `freq_hz`, `bright`, `fb`), 1 output (mono audio).
///
/// Non-finite or non-positive sample rates fall back to 48 kHz.
#[must_use]
pub fn fm_genesis(sample_rate_hz: f32, patch: FmPatch) -> FmGenesisNode {
    // Force the shared ROM generation now (construction may allocate/compute);
    // `process()` then only reads the resulting `&'static`.
    let tables: &'static FmTables = LazyLock::force(&FM_TABLES);

    let carrier_mask = FmPatch::carrier_mask(patch.algorithm);
    let operators = std::array::from_fn(|idx| Operator::from_config(patch.ops[idx]));

    FmGenesisNode {
        channel: Channel {
            operators,
            fnum: 0,
            block: 0,
            algorithm: patch.algorithm & 0x07,
            feedback: patch.feedback & 0x07,
            pms: patch.pms & 0x07,
            ams: patch.ams & 0x03,
        },
        tables,
        sample_rate_hz: sanitize_sample_rate(sample_rate_hz),
        ladder: patch.ladder,
        patch_feedback: patch.feedback & 0x07,
        carrier_mask,
        lfo_enabled: patch.pms > 0 || patch.ams > 0,
        lfo_frequency: patch.lfo_rate & 0x07,
        lfo_counter: 0,
        lfo_phase: 0,
        lfo_am_level: 0,
        lfo_pm_raw: 0,
        lfo_accum: 0.0,
        eg_counter: 0,
        eg_accum: 0.0,
        prev_gate: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regenerated log-sine ROM must reproduce the hardware ROM exactly.
    /// Reference values are the genesoxide `ym2612_sin_table.inc` (Nuked-OPN2
    /// ROM); regenerating them from the public formula is what keeps Orpheus's
    /// tree copyright-clean.
    #[test]
    fn regenerated_sin_table_matches_hardware_rom() {
        let t = FmTables::generate();
        // Head of the ROM.
        assert_eq!(
            &t.sin[0..8],
            &[2137, 1731, 1543, 1419, 1326, 1252, 1190, 1137]
        );
        // A couple of interior anchors.
        assert_eq!(t.sin[16], 846);
        assert_eq!(t.sin[64], 352);
        // The quarter wave decreases monotonically toward ~0 at the peak.
        for i in 1..256 {
            assert!(t.sin[i] <= t.sin[i - 1], "sin ROM must be monotonic at {i}");
        }
        assert!(
            t.sin[255] <= 1,
            "sin ROM should approach 0 at the sine peak"
        );
    }

    /// The regenerated exp ROM must reproduce the hardware ROM exactly.
    #[test]
    fn regenerated_exp_table_matches_hardware_rom() {
        let t = FmTables::generate();
        assert_eq!(
            &t.exp[0..16],
            &[
                2042, 2037, 2031, 2026, 2020, 2015, 2010, 2004, 1999, 1993, 1988, 1983, 1977, 1972,
                1966, 1961
            ]
        );
        // Monotonic decreasing across the mantissa.
        for i in 1..256 {
            assert!(t.exp[i] <= t.exp[i - 1], "exp ROM must be monotonic at {i}");
        }
    }

    #[test]
    fn freq_to_fnum_block_reproduces_a440() {
        // base_step = fnum * 2^(block-1); emitted freq = step * SR / 2^20.
        let sr = 48_000.0;
        let (fnum, block) = freq_to_fnum_block(440.0, sr);
        let step = f64::from(fnum) * 2.0f64.powi(i32::from(block) - 1);
        let emitted = step * f64::from(sr) / f64::from(1u32 << 20);
        assert!(
            (emitted - 440.0).abs() < 1.0,
            "expected ~440 Hz, got {emitted} (fnum={fnum}, block={block})"
        );
    }

    #[test]
    fn carrier_masks_match_algorithm_topologies() {
        assert_eq!(FmPatch::carrier_mask(0), 0b1000);
        assert_eq!(FmPatch::carrier_mask(3), 0b1000);
        assert_eq!(FmPatch::carrier_mask(4), 0b1010);
        assert_eq!(FmPatch::carrier_mask(5), 0b1110);
        assert_eq!(FmPatch::carrier_mask(6), 0b1110);
        assert_eq!(FmPatch::carrier_mask(7), 0b1111);
    }

    #[test]
    fn ladder_off_is_identity_on_is_asymmetric() {
        // True zero stays zero (silence is silent); nonzero samples are biased
        // away from zero — positives up, negatives down (crossover dead-zone).
        assert_eq!(apply_ladder(0), 0);
        assert_eq!(apply_ladder(100), 102);
        assert_eq!(apply_ladder(-100), -101);
        // Non-zero samples are pushed away from zero (crossover dead-zone).
        assert!(apply_ladder(1) > 1);
        assert!(apply_ladder(-1) < -1);
    }
}
