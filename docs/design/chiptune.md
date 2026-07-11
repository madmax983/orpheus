# Design: Authentic NES-Style Chiptune Synthesis

- Status: Draft (Phase 1 — design only, no implementation)
- Date: 2026-07-10
- Scope: `orpheus-dsp` graph nodes + `orpheus-lang` voice surface
- Source seed: [`madmax983/nes`](https://github.com/madmax983/nes) — `crates/nes-core/src/apu.rs` (branch `trunk`). The channel synthesis cores in this design are adapted from that battle-tested NES APU emulator; ported node source files should carry a header crediting `madmax983/nes`.

## 1. Goal

Give Orpheus authentic NES-style chiptune synthesis out of the box. The lo-fi
character **is** the feature: hard-edged duty-cycle squares, a 16-level
staircase triangle, and the exact 15-bit LFSR noise of the 2A03 — not smoothed,
band-limited approximations. A user should be able to write a voice body such as
`pulse_nes(freq, 2)` or `noise_nes(0, freq)` and hear a recognizable NES timbre
immediately, with the authenticity artefacts (4-bit volume quantization, hard
duty edges, LFSR periodicity, optional hardware pitch grid) preserved rather
than filtered away.

This deliberately contrasts with the existing anti-aliased primitives:
`orpheus-dsp/src/synth/osc.rs` implements `PulseOsc`/`TriOsc`/`SawOsc` with
polyBLEP band-limiting (`poly_blep`, `MAX_NORMALIZED_STEP = 0.49`). Those are
"clean analog" sources; the chiptune nodes are their intentionally raw
counterparts and must live alongside, not replace, them.

## 2. Source evaluation (apu.rs)

`apu.rs` is a full, cycle-accurate NES 2A03 APU: 1231 lines implementing four
channels plus a frame sequencer, DMC DMA plumbing, a nonlinear mixer, and a
hardware output filter chain.

### 2.1 Channels present

| Channel | Struct | Core synthesis |
|---|---|---|
| Pulse 1 & 2 | `PulseChannel` | 4 duty patterns (`DUTY_TABLE`: 12.5% / 25% / 50% / 25%-negated), 11-bit timer, 4-bit envelope (decay or constant volume), per-channel **sweep** unit (pulse-1 vs pulse-2 negate differs by 1), length counter |
| Triangle | `TriangleChannel` | 32-step `TRIANGLE_TABLE` staircase (15→0→15, **only 16 distinct 4-bit levels**), 11-bit timer, linear counter + length counter, **no volume control** (fixed amplitude); muted when `timer_reload < 2` (ultrasonic) |
| Noise | `NoiseChannel` | 15-bit LFSR `shift_register` (init `1`), `mode` bit selects tap (**bit 6 short / bit 1 long**), XOR feedback into bit 14, 16-entry `NOISE_PERIOD_TABLE`, 4-bit envelope, length counter |
| DMC | `DmcChannel` | 1-bit delta PCM, 7-bit `output_level`, fetches sample bytes from CPU memory via `DmcDmaRequest` (DMA + IRQ) |

Key constant tables (all directly portable): `DUTY_TABLE[4][8]`,
`TRIANGLE_TABLE[32]`, `NOISE_PERIOD_TABLE[16]`, `LENGTH_TABLE[32]`,
`DMC_RATE_TABLE[16]`.

### 2.2 Emulator-coupling analysis

The file cleanly separates into **channel cores** (extractable) and **emulator
plumbing** (drop or reimplement).

**Extractable channel cores — zero CPU/memory dependency.** Each channel's
sound is produced by pure state-machine methods:

- `PulseChannel`: `clock_timer()` steps `duty_step` (0..7); `output()` returns
  `DUTY_TABLE[duty][step] ? volume : 0`.
- `TriangleChannel`: `clock_timer()` steps `sequence_step` (0..31); `output()`
  returns `TRIANGLE_TABLE[step]`.
- `NoiseChannel`: `clock_timer()` advances the LFSR
  (`feedback = (sr & 1) ^ ((sr >> tap) & 1); sr = (sr >> 1) | (feedback << 14)`);
  `output()` returns `(sr & 1 == 0) ? volume : 0`.

These read only their own fields — no bus, no memory, no CPU handle. This is
exactly the "duty/period/volume → sample" kernel we want.

**Plumbing to drop or reimplement:**

- **MMIO register decode** (`write_register`, `$4000–$4017`) — replace with
  direct signal-input params (freq, duty, volume). Drop the address map.
- **Frame sequencer** (`clock_frame_sequencer`, `FRAME_STEP_*`, mode-4/mode-5,
  frame IRQ) drives envelope/sweep/length at 240/120 Hz off the CPU clock. For a
  synth these units are optional: the graph already owns amplitude shaping via
  `AdsrNode`/`ArNode` and note gating. Recommendation: **omit** the length
  counter and frame-IRQ entirely, and treat the 4-bit envelope/sweep as
  optional authenticity extras driven by an internal sample-rate-derived tick,
  not a CPU clock.
- **CPU clocking / resampling** (`step_cpu_cycle`, `CPU_CLOCK_HZ = 1_789_773`,
  `sample_accumulator` down-conversion to `AUDIO_SAMPLE_RATE`, pulse/noise
  clocked at CPU/2, triangle at CPU/1) — replace with the graph's own
  per-sample advance at the node's `sample_rate_hz`. The one authenticity
  artefact worth **preserving optionally** is the 11-bit period grid: real NES
  pitch is `timer = CPU/(16*freq) - 1` (pulse/noise) or `CPU/(32*freq) - 1`
  (triangle), quantized to an integer, which detunes notes characteristically.
- **DMC** (`DmcChannel`, `DmcDmaRequest`, memory fetch, IRQ) — **drop for PR1**.
  It needs a CPU address space and delivers PCM, not a tone generator. A future
  "sample player over the existing `SampleBank`" is the better Orpheus analog.
- **Nonlinear mixer** (`get_mixer_tables`, `pulse_table`/`tnd_table`,
  `MAX_SAMPLE_AMPLITUDE`) and **output filter chain** (`apply_output_filters`:
  HP 90 Hz → HP 440 Hz → LP 14 kHz, plus `soft_limit_sample`) are the authentic
  *console coloration*. These are **global/master-bus** behavior, not per-voice.
  Out of scope for PR1 nodes; candidate for a later optional "NES master" effect
  (see §6 on ADR need).

**Verdict:** the three tone-generating cores (pulse duty, triangle staircase,
noise LFSR) are cleanly extractable as self-contained state machines. Everything
that couples to the CPU, memory, frame counter, or MMIO map is plumbing that the
Orpheus graph already replaces with signal inputs, its own sample clock, and its
own envelopes/gates.

## 3. Build vs buy

### 3.1 The `soundlog` crate finding

**`soundlog` is not a chiptune synthesizer.** Verified via docs.rs: it is a
library for **building and parsing VGM (Video Game Music) files** — the
register-write log format for retro sound chips (YM2612, SN76489, AY-8910, and
~35 others). It offers a builder API, a streaming parser (`VgmStream`), and a
callback/state-tracking layer (`VgmCallbackStream`). It manages *register-write
logs and their timing*; it does **not** generate audio samples from those
registers. It would only be useful if Orpheus wanted to import/export `.vgm`
files, which is a different feature entirely. It is irrelevant to synthesis.

(No maintained pure-Rust NES-APU-as-synth crate is a clearly better fit than the
user's own code; general options like `fundsp` don't ship authentic 2A03
channels, and pulling a full emulator crate drags in exactly the CPU/memory
plumbing §2.2 says to discard.)

### 3.2 Recommendation

**Adapt the APU channel cores as native `orpheus-dsp` graph nodes.** Orpheus
keeps a near-zero-dependency, allocation-free, lock-free audio path, and the
user's APU is battle-tested and theirs — so port the three tone cores directly,
preserving the NES authenticity switches (duty tables, 4-bit volume
quantization, 32-step triangle staircase, 15-bit LFSR tap modes, and an optional
hardware pitch/period grid), and drop the emulator plumbing. Do **not** add
`soundlog` or any synthesis dependency.

## 4. Proposed node set for PR 1

Each node follows the graph `Node` trait (`crates/orpheus-dsp/src/graph/node.rs`):
`inputs()`/`outputs()` fixed at construction, block `process()` with no
allocation or locking, `reset()` restoring construction state. Parameters are
**signal inputs** per ADR 0004 (a duty or volume is an input channel, not a
config field), so they can be modulated at audio rate. Sample rate is passed to
the constructor function (mirroring `sine(sample_rate_hz)` and
`pulse(sample_rate_hz)`). New leaf nodes belong in a new
`crates/orpheus-dsp/src/graph/chiptune.rs` module, re-exported from
`graph/mod.rs` alongside the existing primitives.

Output convention: like the existing oscillators, emit bipolar `f32` in roughly
`[-1, 1]`. NES channels are unipolar (0..15); map `level → (level/15)` then to
bipolar so silence is 0 and the duty/staircase edges are preserved.

### 4.1 `PulseNesNode` — `pulse_nes(sample_rate_hz)`

- **Inputs (3):** `freq_hz`, `duty` (0..3 index; continuous values quantized to
  the nearest of the 4 patterns), `volume` (0..1, quantized to 4 bits / 16 levels).
- **Outputs (1):** audio.
- **Internal state:** phase accumulator (or an integer NES timer when the
  hardware-grid switch is on), `duty_step: u8` (0..7).
- **NES-authentic details to preserve:**
  - `DUTY_TABLE` = `[[0,1,0,0,0,0,0,0],[0,1,1,0,0,0,0,0],[0,1,1,1,1,0,0,0],[1,0,0,1,1,1,1,1]]`
    (12.5% / 25% / 50% / 25%-negated); output is `duty_bit ? volume : 0`, a hard
    two-level square with **no band-limiting** (this is the point).
  - 4-bit volume quantization: `vol = round(volume * 15) / 15`.
  - Optional hardware-pitch switch: snap `freq` to the 11-bit timer grid
    `timer = round(CPU/(16*freq)) - 1`, clamped to `0..0x7FF`, then play back at
    the quantized frequency for authentic detuning.
  - Optional sweep is deferred (envelope/sweep are frame-sequencer extras, §2.2);
    PR1 exposes duty/freq/volume only.
- **Tests that pin behavior:** duty high-time ratios (2/16, 4/16, 8/16, 12/16 of
  a period); output takes only two values (`0` and `±volume`); a full period's
  step sequence matches `DUTY_TABLE[duty]`; 4-bit volume quantization lands on
  the 16-level grid.

### 4.2 `TriNesNode` — `tri_nes(sample_rate_hz)`

- **Inputs (1):** `freq_hz`.
- **Outputs (1):** audio.
- **Internal state:** `sequence_step: u8` (0..31).
- **NES-authentic details to preserve:**
  - `TRIANGLE_TABLE` = `[15,14,…,1,0,0,1,…,14,15]` — a 32-entry, **16-level**
    staircase (each level held for two steps at the turnaround). The visible
    quantization staircase is the signature triangle timbre and must not be
    smoothed.
  - **No volume control** (fixed amplitude), matching hardware.
  - Ultrasonic mute: when the (grid-quantized) period would be `< 2`, output 0
    (matches `timer_reload < 2` guard).
- **Tests that pin behavior:** the emitted step sequence over one period equals
  `TRIANGLE_TABLE` exactly; output takes exactly 16 distinct levels; symmetry of
  the up/down ramp; ultrasonic-mute produces silence.

### 4.3 `NoiseNesNode` — `noise_nes(sample_rate_hz)`

- **Inputs (3):** `mode` (0 = long/15-bit, non-zero = short/93-step),
  `freq_hz` (or period index when the NES-table switch is on), `volume` (4-bit).
- **Outputs (1):** audio.
- **Internal state:** `shift_register: u16` (init `1`), timer/phase.
- **NES-authentic details to preserve:**
  - LFSR step: `tap = if short {6} else {1}; feedback = (sr & 1) ^ ((sr >> tap) & 1); sr = (sr >> 1) | (feedback << 14)` — a 15-bit register, tap bit 1 (long,
    period 32767) vs bit 6 (short, period 93). This bit-exact sequence is the
    reference target.
  - Output `(sr & 1 == 0) ? volume : 0`, 4-bit volume quantization.
  - Optional `NOISE_PERIOD_TABLE` = `[4,8,16,32,64,96,128,160,202,254,380,508,762,1016,2034,4068]`
    for the 16 authentic NES noise "pitches"; otherwise map `freq_hz` continuously.
- **Tests that pin behavior:** starting from `sr = 1`, the produced LFSR bit
  stream matches a checked-in reference vector for both long and short modes;
  long mode has period 32767, short mode period 93; output is `{0, ±volume}` only.

## 5. Language exposure sketch (PR 2)

The lowering path is already established for the existing oscillators and should
be mirrored exactly:

1. **IR variants** in `crates/orpheus-dsp/src/graph_voice.rs` `VoiceNodeSpec`
   (serializable, travels through `EngineCommand`). Add `PulseNes { freq, duty,
   volume }`, `TriNes { freq }`, `NoiseNes { mode, freq, volume }` alongside the
   existing `Pulse { freq, width }` / `Tri { freq }` / `Noise { seed }`. Update
   the `inputs`/`for_each_signal` arms (around lines 723–828) and the lowering
   match (around lines 1323–1326) using the established
   `par(pulse_nes(sr), passthrough(bus))` shape.
2. **Language stages** in `crates/orpheus-lang/src/voice.rs` `compile_call`
   (around lines 489–511): add `"pulse_nes"`, `"tri_nes"`, `"noise_nes"` to the
   dispatch and to the "unknown voice stage" help list, with `compile_*`
   builders analogous to `compile_pulse`/`compile_oscillator`/`compile_noise`.

Proposed surface (usable as voice bodies, pipe-friendly):

```text
pulse_nes(freq, duty)      # duty in 0..3 (default 2 = 50%); NES square
tri_nes(freq)              # 16-level staircase triangle
noise_nes(mode, freq)      # mode 0 = long/tonal-hiss, 1 = short/metallic
```

**Naming note:** the language already binds `pulse` to the band-limited
`PulseOsc` (`voice.rs::compile_pulse`, default width
`DEFAULT_PULSE_WIDTH`). The NES variant must therefore be a **distinct name**
(`pulse_nes`), not a reuse of `pulse`, to avoid clobbering the clean oscillator.
`tri_nes` / `noise_nes` likewise sit beside the existing `tri` / `noise`. All
three should accept bound signals for their params so `pulse_nes(lfo, 2)`
sweeps pitch and a modulated duty works, consistent with ADR 0004.

## 6. Is an ADR needed?

**Not for the PR1 nodes.** They live entirely inside the `orpheus-dsp` graph
module and add `VoiceNodeSpec` variants — the same boundary and pattern ADR 0004
and the existing `Saw`/`Pulse`/`Noise` nodes already established. They do not
introduce new engine commands, cross the engine/mixer boundary, or change the
routing snapshot contract. A short design doc (this file) plus the standard
SPEC-PROOF-RED-GREEN-REFACTOR test-first flow is sufficient.

**An ADR *would* be warranted** if a later phase adds the **global NES
coloration** (§2.2: the nonlinear `pulse_table`/`tnd_table` mixer and the
HP90→HP440→LP14k output filter + soft clip) as a master-bus effect, since that
crosses into the mixer/engine boundary governed by ADR 0003 and would change how
the master path colors all audio.

## 7. Attribution

The pulse/triangle/noise synthesis cores in this design are adapted from
**[madmax983/nes](https://github.com/madmax983/nes)**, `crates/nes-core/src/apu.rs`.
Every ported node source file should include a header comment crediting that
repository as the origin of the channel algorithms and constant tables
(`DUTY_TABLE`, `TRIANGLE_TABLE`, `NOISE_PERIOD_TABLE`).
