# Design: Sega Genesis / Mega Drive PSG (SN76489)

- Status: Implemented (Phase 3 PR1 — DSP core; language/voice surface is PR2)
- Date: 2026-07-11
- Scope: `orpheus-dsp` graph nodes (this PR). Language `voice{}` exposure + demo
  are a deliberately separate follow-up PR (PR2).
- Source seed: [`madmax983/genesoxide`](https://github.com/madmax983/genesoxide)
  `crates/genesoxide-core/src/psg.rs` (branch `trunk`) — the user's own Sega
  Genesis / Mega Drive emulator PSG core. This is the PSG analog of the
  `ym2612.rs` seed that produced [`genesis-fm.md`](./genesis-fm.md), which in turn
  mirrors the `madmax983/nes` `apu.rs` → [`chiptune.md`](./chiptune.md) arc.
- Companion: read [`chiptune.md`](./chiptune.md) and [`genesis-fm.md`](./genesis-fm.md)
  first — this is the same arc (extract a tested emulator core into
  allocation-free graph leaf nodes) one chip over. The PSG follow-up was already
  scoped in [`genesis-fm.md`](./genesis-fm.md) §8.

## 1. Goal

Give Orpheus the Genesis's *second* voice: the Texas Instruments **SN76489 PSG**
that sits over the YM2612 FM bed — the arps, blips, bleeps, and hi-hats. Three
50%-duty square-wave tone channels and one LFSR noise channel, each with a 4-bit
(16-step) **logarithmic** attenuator. This is architecturally the *easy* Genesis
chip and the direct continuation of the NES `pulse_nes` / `noise_nes` story:
a hard square via a divider and an LFSR noise generator with periodic/white
modes — but with the SN76489's own two authenticity fingerprints: a **2 dB/step
logarithmic volume grid** (unlike NES's linear 4-bit) and the **Sega 16-bit
LFSR** with taps at bits 0 and 3 (unlike the NES 15-bit tap-0/1 LFSR, and unlike
the discrete TI SN76489's 15-bit tap-0/1 LFSR).

As with the FM node, the goal is *the sound*, not a register-accurate chip: we
drop the latch/data register protocol, the 68000/Z80 bus, and the 4-channel
internal mix, and instead instantiate one channel's worth of PSG math per graph
node, driven by `gate` / `freq_hz` / `level` signal inputs.

## 2. SN76489 background (as implemented by genesoxide)

genesoxide's `psg.rs` is the authority for *this* chip (the Genesis-integrated
PSG); where the canonical discrete TI SN76489 differs, we follow genesoxide and
note the difference.

### 2.1 Tone channels (3× 50% square)

Each tone channel has a **10-bit period** register and a down-counter clocked at
the PSG native rate (master / 240 ≈ 223.72 kHz on NTSC — genesoxide clocks
`Psg::clock_tick` once per `PSG_AUDIO_TICKS = 240` master ticks, i.e.
`master/15/16`). Per native tick (`psg.rs::clock_tick`):

```text
if period == 0 { polarity = true; continue; }        // period-0 quirk
if counter == 0 { counter = period; polarity = !polarity; }
else            { counter -= 1; }
```

The output is a hard 50% square that toggles between `+level` and `−level` (a
**bipolar** square, not the NES's `level`/`0`). Two quirks are pinned bit-exactly:

- **Period-0 quirk.** genesoxide maps a period of `0` to a **constant high
  output** (`polarity = true`, counter frozen) — *not* to period 1 and *not* to
  `0x400`. We reproduce this exactly (pinned in `psg_tone_period_zero_holds_high`).
- **Reload timing.** A toggle occurs on every counter-reaches-zero event, and the
  reload value is the raw period, so toggles are `period + 1` native ticks apart
  and the emitted fundamental is `native / (2 · (period + 1))`. The port inverts
  this to pick the period from `freq_hz`, so the chip's 10-bit pitch
  quantization is reproduced rather than smoothed away (the SN76489 analog of the
  FM node's `fnum`/`block` quantization).

### 2.2 Noise channel (16-bit LFSR)

One noise channel driven by a **16-bit** linear-feedback shift register seeded to
`0x8000`, shifted right, feedback inserted at bit 15. Two modes
(`psg.rs::clock_tick`):

- **White noise:** feedback = `bit0 XOR bit3` (tap mask `0x0009`).
- **Periodic noise:** feedback = `bit0` — a single set bit walks the register,
  giving a period-16 pulse train (a pitched "buzz").

The LFSR advances on **every** noise-counter reload (genesoxide fixed a
half-rate bug here; see its `noise_lfsr_shifts_every_counter_reload` test). The
shift rate is set by a 2-bit rate selector: rates 0/1/2 use fixed reloads
`[0x10, 0x20, 0x40]` (= 16/32/64 native ticks — the SN76489's nominal
÷512/÷1024/÷2048 of the 3.579545 MHz reference, i.e. those divisors taken on the
already-/16-divided native clock), and rate 3 tracks tone channel 3's period. In
the graph node the shift rate is driven directly from `freq_hz` instead of a rate
register (shift once per `period + 1` native ticks, `period = native/freq − 1`).

Both LFSR sequences are pinned against genesoxide's algorithm for the first 20
steps (`advance_psg_lfsr` unit test) — the direct analog of the NES
`advance_lfsr` pin.

> **Note on LFSR width/taps.** The task brief described a "15-bit LFSR"; that is
> the *discrete* TI SN76489 (15-bit, taps 0/1). genesoxide implements the
> **Sega-integrated** PSG variant: a 16-bit register with taps at bits 0 and 3.
> Because genesoxide is the reference for the Genesis's actual chip, we pin its
> 16-bit / tap-0,3 behavior and document the divergence here rather than
> "correcting" it to the discrete part.

### 2.3 4-bit logarithmic attenuation (the fingerprint)

Each channel has a 4-bit attenuation register: 16 steps at **2 dB/step**, `0` =
full volume, `15` = silence. genesoxide's `VOLUME_TABLE` is
`round(10^(−i/10), 3)` with index 15 forced to `0.0`. This is the SN76489's
signature *logarithmic* volume law, distinct from the NES's linear 4-bit grid.

**Zero-copyleft table regeneration.** genesoxide ships **no LICENSE file**, so —
exactly as the FM ROMs were regenerated from the public log-sine/exp formulas
rather than copied — the PSG volume table is **regenerated in-code from the
public 2 dB/step formula** `level(i) = 10^(−i/10)` (with `level(15) = 0` for true
silence), not copied from genesoxide's rounded literals. The formula is
documented at the generator and pinned by `volume_table_matches_2db_formula`,
so Orpheus's tree stays unambiguously permissive.

### 2.4 What we deliberately drop

Mirroring the FM node (`genesis-fm.md` §2.7): the latch/data **register
protocol** (`Psg::write`), the 68000/Z80 bus addresses, the internal **4-channel
mix and `/4` normalization** (`Psg::sample`), the desktop/cpal layer, and the
post-mix EQ/gain profile chain (`api.rs`). Orpheus instantiates **one channel's
worth of PSG math per node** and lets the engine's voice pool and mixer own
polyphony and output shaping (ADR 0003).

## 3. Node API surface (this PR)

Two allocation-free, lock-free leaf nodes (`graph/genesis_psg.rs`), re-exported
from `graph/mod.rs` and the crate root, `psg_`-prefixed to avoid colliding with
the existing NES/adapter `pulse` / `noise` names:

- **`PsgToneNode` / `psg_tone(sample_rate_hz)`** — 3 inputs `(gate, freq_hz,
  level)`, 1 mono output. A hard bipolar 50% square. `gate > 0.5` unmutes;
  `freq_hz` selects the 10-bit period (chip pitch quantization preserved);
  `level` in `[0, 1]` is quantized to the 16-step 2 dB attenuation grid
  (`1.0` → index 0 / loudest, `0.0` → index 15 / silent).
- **`PsgNoiseNode` / `psg_noise(sample_rate_hz)`** — 4 inputs `(gate, mode,
  freq_hz, level)`, 1 mono output. `mode ≥ 0.5` = white, else periodic;
  `freq_hz` sets the LFSR shift rate; `level` as above.
- **`advance_psg_lfsr(shift, white) -> u16`** — the pure, `const`, pinned LFSR
  step (public, mirroring NES `advance_lfsr`).

Non-finite / non-positive sample rates fall back to 48 kHz, matching the other
leaf nodes. The counter/LFSR run at the PSG native rate and are resampled to the
graph's `sample_rate_hz` via a fractional accumulator (the same cadence pattern
as the FM node's EG/LFO).

### 3.1 Headroom

genesoxide's `Psg::sample` sums four channels and divides by 4. For a single
graph voice that convention is replaced by a documented single-voice headroom
scale (`PSG_HEADROOM_SCALE = 0.9`): a full-volume square peaks at `±0.9`, clear
of the `±1.0` rails with no hard-clipped samples (pinned in
`single_voice_never_clips`), leaving the mixer to sum voices.

## 4. Testing strategy

Mirrors the FM layout (`graph_fm.rs` / `graph_fm_alloc.rs`): behavior in
`tests/graph_psg.rs`, allocation discipline in `tests/graph_psg_alloc.rs`, plus
in-module unit tests for the pinned tables/cores. Pins: volume-table formula and
endpoints/monotonicity; both LFSR sequences (white + periodic) for the first 20
steps; periodic-noise period-16; the tone period-0 constant-high quirk; tone
pitch tracking `freq_hz`; the 2 dB logarithmic level spacing; white≠periodic
character; single-voice headroom (peak < 1.0, no rail samples) for tone and
noise; and a counting-allocator proof that `process()` allocates 0 bytes across
many blocks for both nodes.

## 5. Deferred to PR2

Language/voice exposure (`psg_tone` / `psg_noise` `voice{}` stages, preset/arg
surface, `VoiceNodeSpec` variants + `EngineCommand` round-trip) and a demo
program — explicitly out of scope here, matching how the FM core (PR) and FM
voice (separate PR) were split.

## 6. Attribution

The tone counter, noise LFSR, and rate model are ported from
[`madmax983/genesoxide`](https://github.com/madmax983/genesoxide)
`crates/genesoxide-core/src/psg.rs`; the ported source file carries a header
crediting it, as the FM/chiptune nodes credit their seeds. The 4-bit volume
table is **regenerated from the public 2 dB/step formula** (§2.3), not copied,
keeping the tree zero-copyleft.
