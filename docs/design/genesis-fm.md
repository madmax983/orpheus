# Design: Sega Genesis / Mega Drive FM Synthesis (YM2612)

- Status: Draft (Phase 1 — design only, no implementation)
- Date: 2026-07-11
- Scope: `orpheus-dsp` graph nodes + `orpheus-lang` voice surface
- Source seed: [`madmax983/genesoxide`](https://github.com/madmax983/genesoxide) — the
  user's Sega Genesis / Mega Drive emulator (the FM/PSG analog of the
  [`madmax983/nes`](https://github.com/madmax983/nes) `apu.rs` seed that produced
  [`chiptune.md`](./chiptune.md)). If genesoxide carries a battle-tested YM2612
  core, the ported node source files should credit it the way the NES chiptune
  nodes credit `apu.rs`.
- Companion: this doc mirrors the structure and depth of
  [`chiptune.md`](./chiptune.md); read that first — the FM work is the same arc
  (extract a tested emulator core into allocation-free graph leaf nodes, then
  expose them as `voice{}` sources) one console over.

## 1. Goal

Give Orpheus the **Sega Genesis / Mega Drive sound** out of the box: Yamaha
YM2612 four-operator FM synthesis with its unmistakable character — glassy DX-style
electric pianos, rubbery slap basses, buzzy brass stabs, and that gritty low-end
"ladder" distortion that makes a Genesis bass line sound like a Genesis and
nothing else. A user should be able to write a voice body such as

```text
fm_genesis("ebass", freq)
```

and hear a recognizable Sega FM timbre immediately, with the authenticity
artefacts — the log-domain sine, the coarse envelope-rate grid, operator
feedback, and the 9-bit DAC ladder crossover — preserved rather than smoothed
away.

The **goal is the sound, not a register-accurate chip**. We do not need a
cycle-accurate YM2612 (no 6-channel timer/register bus, no VGM playback, no
sample DAC channel). We need: the 8 FM algorithms with correct operator
topology, per-operator frequency-multiple / detune / rate-scaled ADSR envelopes,
op-1 feedback, the ladder grit, and (in scope, tunable) the global LFO with
per-channel PMS/AMS. Everything is livecoding-ergonomic: a small signal surface
plus named presets.

This sits beside the NES chiptune nodes (`pulse_nes` / `tri_nes` / `noise_nes`)
and the clean polyBLEP oscillators (`synth/osc.rs`) as another authentic,
character-first source family — not a replacement for any of them.

## 2. YM2612 background

The YM2612 (a.k.a. OPN2) is a 6-channel, 4-operator phase-modulation FM
synthesizer. Each channel is fully described by:

### 2.1 The operator (the atom of FM)

An **operator** is a sine oscillator with its own envelope. Four of them per
voice. Each operator computes, per sample:

```
phase   += phase_increment(note_freq * MUL, DETUNE)
env      = envelope_generator(gate, AR, D1R, D2R, SL, RR, RS)   // attenuation
sample   = sin_table[phase + modulation_input] * exp_table[env + TL]
```

Key authenticity points, all cheap to reproduce faithfully:

- **Phase modulation, not frequency modulation.** A modulator's *output sample*
  is added to the carrier's *phase index* each sample. This is what "FM" on
  these chips actually is; it keeps the spectrum stable and is trivial to
  implement with a phase accumulator + wavetable.
- **Log-domain sine + exp output.** The hardware stores a quarter-sine table in
  the log (attenuation) domain and converts back through a pow2/exp table, so
  amplitude scaling (envelope + TL) is a cheap *addition* in the log domain. A
  faithful port keeps a 256/512-entry sine table and an exp table; the quantization
  is part of the timbre.
- **`MUL` — frequency multiple** (0..15): operator runs at `note_freq * MUL`,
  with the special case **`MUL = 0` → ×0.5**. Integer ratios between operators
  are what make FM spectra harmonic (bells, e-pianos) or clangorous.
- **`DETUNE` (DT, 0..7):** a small ± frequency offset (bit 2 is the sign:
  0 = none, 1..3 = +, 5..7 = −). Slight detune between operators gives the
  shimmer/chorus of an e-piano; large-MUL + detune gives metallic drums.
- **`TL` — total level** (0..127, 0.75 dB/step, 0 = loudest): per-operator
  output attenuation. On *carriers* it sets channel volume; on *modulators* it
  sets **FM index** — i.e. timbral brightness. Sweeping a modulator's TL is the
  classic "FM brightness" gesture.

### 2.2 The envelope generator (EG)

Each operator has a 4-stage DAHDSR-ish envelope working in the **attenuation
(log) domain**, 10-bit:

| Param | Range | Meaning |
|---|---|---|
| `AR`  (attack rate)        | 0..31 | rise from silence to full |
| `D1R` (first decay rate)   | 0..31 | fall from peak to sustain knee |
| `SL`  (sustain level)      | 0..15 | attenuation at the knee |
| `D2R` (second decay rate)  | 0..31 | continued decay while held (0 = hold) |
| `RR`  (release rate)       | 0..15 | fall after key-off |
| `RS`  (rate scaling / KS)  | 0..3  | higher notes → faster envelopes |

The **rates are a coarse exponential grid**, not seconds — a rate of 31 is
near-instant, low rates take seconds, and `RS` speeds every stage up as pitch
rises. That coarse, key-scaled character is audible and should be pinned, not
replaced by a smooth seconds-based ADSR. The EG runs at a fixed internal tick
(the chip clocks it off the sample stream); we derive that tick from
`sample_rate_hz`.

**Optional `SSG-EG`** (0..15, in scope as a switch): a mode where the envelope
loops / mirrors / holds during the sustain phase, producing fast repeating "saw"
or "buzz" envelopes used for Genesis snares, hats, and zaps. Worth including as
an opt-in per operator because a lot of Genesis percussion depends on it; can
land in a Phase-2b follow-up if it bloats the first cut.

### 2.3 The 8 algorithms (operator topology)

The **algorithm** (0..7) selects how the four operators are wired: which
operators modulate which, and which are *carriers* (summed to the channel
output). Operator 1 (`S1`) is the only one with **feedback**. Numbering below is
the standard Sega/MAME operator order `1,2,3,4`; `⟲` marks op-1 self-feedback;
everything that reaches `→out` is a carrier and the carriers are summed.

```
ALG 0 — one serial chain (bells, clav, hard e-piano)
  ⟲
 [1]→[2]→[3]→[4]→out

ALG 1 — 1 and 2 stacked into 3, then 4
  ⟲
 [1]┐
    ├→[3]→[4]→out
 [2]┘

ALG 2 — 2→3→4, with 1 also into 4
     [2]→[3]┐
  ⟲         ├→[4]→out
 [1]────────┘

ALG 3 — 1→2 and 3, both into 4
  ⟲
 [1]→[2]┐
        ├→[4]→out
 [3]────┘

ALG 4 — two parallel 2-op stacks (basses, reeds)
  ⟲
 [1]→[2]────────→out
 [3]→[4]────────→out

ALG 5 — 1 fans out to 2,3,4 (rich, detuned pads/brass)
      ┌→[2]→out
  ⟲  │
 [1]─┼→[3]→out
      └→[4]→out

ALG 6 — 1→2, plus 3 and 4 as bare carriers
  ⟲
 [1]→[2]→out
 [3]────→out
 [4]────→out

ALG 7 — full additive: all four are carriers (organs, additive)
  ⟲
 [1]→out
 [2]→out
 [3]→out
 [4]→out
```

Roughly: low algorithms (0–3) are "deep stacks" → bright, clangorous, evolving
(e-pianos, bells, brass). High algorithms (4–7) are "wide/parallel" → additive,
organ- and bass-like. Getting this routing table exactly right is the single
most load-bearing correctness detail in the whole node.

### 2.4 Feedback

Operator 1 feeds its **own** output back into its phase, scaled by a 3-bit
`FB` amount (0..7). The hardware averages the operator's previous two output
samples for the feedback term (a 1-sample-ish smoothing that tames the loop).
Feedback ranges from a subtle harmonic thickening (FB 1–3) to a saw-like /
noisy edge (FB 6–7) and is essential for basses and noise-ish percussion. This
maps naturally onto the graph's recursive-combinator idea but is cheap to
implement inline as a 2-sample history inside the node.

### 2.5 LFO, PMS, AMS (in scope, modest)

A single **global LFO** (one of 8 rates) drives two per-channel sensitivities:

- **PMS** (phase-mod sensitivity, 0..7) → vibrato (pitch wobble).
- **AMS** (amplitude-mod sensitivity, 0..3) → tremolo, applied to operators
  whose AM-enable bit is set.

These are what give Genesis leads their vibrato and pads their shimmer. In
scope as tunable patch fields; can be a fast follow if the first node ships
without them.

### 2.6 The ladder effect (the grit)

The early discrete-DAC YM2612 (in most Model-1 Genesis units) has a **9-bit
DAC with a crossover discontinuity around zero**: there is a small dead-zone /
offset such that low-level signals are pushed away from zero asymmetrically.
The audible result is **crossover distortion and a characteristic low-end
"grit"/buzz** that is strongest on quiet, bass-heavy material — a defining part
of the Genesis sound, and the reason a clean FM render sounds "too polite."

Faithful modeling (per Nuked-OPN2): before summing a channel, non-zero operator
output is offset by a small fixed amount whose sign depends on the sample's sign
(roughly "positive values +k, negative values −k, zero stays zero"), so the
distortion only partially cancels across channels. We expose this as a
**`ladder: bool`** patch switch (default on) plus, optionally, an intensity, so
users can A/B the "Model 1 grit" against a clean render. The exact offset
constants should be taken from a reference core, not guessed.

> **Note (from source review):** genesoxide's `ym2612.rs` does **not** implement
> the ladder effect — grep confirms no ladder/crossover model in the core (its
> low-end coloration is instead approximated downstream in the desktop post-mix
> EQ chain, `api.rs`). The ladder is therefore **new work Orpheus must add
> itself** on top of the ported core, taking the offset constants from
> Nuked-OPN2. This is arguably the single most valuable authenticity addition we
> make beyond a straight port, since it is the defining Genesis grit.

### 2.7 What we deliberately drop

Mirroring `chiptune.md` §2.2, everything that couples to the chip's host bus is
plumbing the Orpheus graph already replaces:

- **Register/MMIO decode** (`$4000`-style writes, part I/II latches) → replaced
  by patch config + signal inputs.
- **6-channel multiplexing, key-on/off register, timers A/B, busy flags** → the
  graph instantiates one node per voice and drives key-on/off from the voice
  gate; polyphony is the engine's job (voice pool), not the chip's.
- **The DAC / PCM channel-6 sample mode** → drop; Orpheus already has
  `SampleBank` / `sample_player` for PCM, the better analog.
- **SN76489 PSG** → separate, lower-priority follow-up (§8).

## 3. Source evaluation (genesoxide `ym2612.rs`)

The NES arc extracted three self-contained *channel cores* from a full emulator
and dropped the CPU/bus plumbing. The Genesis arc is the same shape but the
extractable unit is bigger: **one YM2612 FM channel** (4 operators + EG +
algorithm router + feedback), not a one-line duty table.

The port source is in hand and reviewed:
[`madmax983/genesoxide`](https://github.com/madmax983/genesoxide),
`crates/genesoxide-core/src/ym2612.rs` (branch `trunk`, ~2243 lines), plus the
sibling ROM tables `ym2612_sin_table.inc` and `ym2612_exp_table.inc`. This is a
**mature, high-quality, cross-validated core** and is the right seed — the direct
`apu.rs → chiptune.rs` analog. Assessment:

### 3.1 What's there (excellent, port it)

- **Operator kernel** (`struct Operator`, `fn generate_output`): 10-bit phase →
  256-entry quarter-wave **log-sine ROM** → attenuation add (envelope + TL + AM
  in the log domain) → **exp ROM** → linear, with the `<< 2` depth fix and
  quarter-wave sign/mirror decode. Faithful and clean.
- **Full operator parameter set** already matches the API in §5.1 field-for-field:
  `total_level`, `sustain_level`, `attack_rate`, `decay_rate`,
  `sustain_rate` (D2R), `release_rate`, `multiply`, `detune`, `key_scale` (RS),
  `am_enable`, `ssg_eg` — and **SSG-EG is fully implemented** (invert state
  machine, `ssg_output_invert`), which is a big head start (§2.2).
- **All 8 algorithms** implemented explicitly (`Channel::output_sample`) with the
  correct carrier topology, plus op-1 **feedback** (`(prev_output + output) >>
  (10 - feedback)`, the ymfm-style 2-sample average).
- **CH3 special mode** (per-operator frequency), **LFO** with AM/PM
  (`advance_lfo`), PMS/AMS, key-scaling, and the EG rate/quarter-tick timing are
  all present and commented with hardware/ymfm references.
- **Cross-validation exists**: `genesoxide-test-harness/src/ymfm_reference.rs`
  already A/Bs this core against **ymfm** — so its accuracy is measured, not
  assumed. We inherit that confidence.

### 3.2 What to drop (host/bus plumbing, per §2.7)

Register write decode (`$4000`-style `write_register`), the 6-channel mux and
per-channel pan/timers, DAC channel-6 PCM mode (`dac_enabled`/`dac_value`), CSM,
and serialization. In Orpheus we instantiate **one channel's worth of math per
voice**, drive key-on/off from the voice gate, and let the engine's voice pool do
polyphony. Also drop the **entire desktop/cpal layer**
(`genesoxide-desktop/src/audio.rs`) and the **post-mix profile chain**
(`api.rs`: `ym_gain`/`psg_gain`/`master_gain`, EQ stages, crossfeed, `soft_limit`)
— Orpheus's mixer/engine own output shaping (ADR 0003).

### 3.3 What to add (not in genesoxide)

- **The ladder effect** — absent from `ym2612.rs` (§2.6). Add it ourselves from
  Nuked-OPN2 constants; it is the headline authenticity win.
- **A clean single-voice output normalization** with real headroom (see §3.4 and
  §4.1) — genesoxide's `FM_SCALE` is tuned for a 6-channel RMS match to ymfm, not
  for a single voice sitting safely in `[-1, 1]`.

### 3.4 Clipping diagnostic (the core is clean)

The user reports chasing clipping in genesoxide. I inspected the operator-mix /
DAC path specifically:

- **The pre-DAC clamp is present and correct.** Each channel's carrier sum is
  right-shifted by 5 and **clamped once to ±256** (`const CLIP: i32 = 256;` in
  `output_sample` / `output_sample_ch3`), explicitly modeling the 9-bit
  multiplexed DAC. The code comments deliberately clamp the *summed carriers
  once* rather than each carrier — matching hardware, and the direct analog of
  the "clamp the operator sum before the DAC" behavior. **Not missing.**
- **No integer overflow / saturation bug.** Sums are `i32`; the worst case is 6
  channels × ±256 = ±1536 — nowhere near `i32` range. Envelope attenuation is
  intentionally left un-clamped ("real hardware allows natural overshoot") but is
  bounded by the `atten >= 4096 → silence` guard, so it cannot blow up the linear
  output.
- **One latent over-unity nit (not the culprit):** `FM_SCALE = 1 / (1536 ×
  0.9937)`. Six in-phase channels at full ±256 therefore map to ±1536 / 1526.3 ≈
  **±1.006** — a deliberate ~0.6 % over-unity trade documented in the code to
  center the 6-channel **RMS** on the ymfm reference. It can nick a hard `[-1,1]`
  clamp by 0.6 % in a pathological all-channels-max case, but is far too small to
  explain audible clipping.

**Verdict: the chip math core is clean — proper per-channel 9-bit clamping, no
overflow.** That implicates the layers we are dropping anyway: the **post-mix
gain chain** (`api.rs` defaults push `ym_gain = 1.1`, sum PSG, apply
`master_gain`, then a `tanh` `soft_limit` limiter with `KNEE = 0.8`) and/or the
**cpal resampler + SPSC ring buffer** in `genesoxide-desktop/src/audio.rs`
(rate-correction under/overruns read as crackle/"clipping"). None of that comes
into Orpheus. For the user's upstream fix, the place to look is the `api.rs`
gain-staging into `soft_limit` and the `audio.rs` ring-buffer fill, **not**
`ym2612.rs`. The only core-side polish worth doing upstream is giving the single
`FM_SCALE` a hair more headroom (divide by ≥1536, not 1526) if absolute-peak
safety matters more than the RMS match.

Because our port renders one voice at a time and re-normalizes for headroom
(§4.1), Orpheus's offline-render headroom/peak tests (§7) double as a standing
proof that the ported math itself never clips — a clean port is itself the
diagnostic.

## 4. Options evaluation

House ethos (from `CLAUDE.md` + ADR 0004 + the chiptune precedent): native
Rust, allocation-free and lock-free on the audio path, test-pinned,
**zero new dependencies unless clearly winning**, and *the sound* over
register-perfect emulation.

### 4.1 Option (a) — port genesoxide's `ym2612.rs` core natively  ✅ recommended

Port the pure FM math from `genesoxide-core/src/ym2612.rs` (§3) into a
self-contained `FmGenesisNode`: 4 operators (phase accumulator + log-sine/exp
ROMs from the `.inc` tables), per-op rate-scaled EG in the attenuation domain,
the 8 algorithm topologies, op-1 feedback (2-sample average), SSG-EG (already
implemented upstream), global LFO → PMS/AMS, and the per-channel ±256 clamp —
**plus a newly-added ladder crossover** (§2.6, §3.3) and a headroom-safe single-
voice normalization (§3.4). Strip the register/bus/mux/DAC/timer plumbing and the
entire cpal + post-mix layer (§3.2). Fixed-size arrays only, so `process()` is
allocation- and lock-free by construction (Node contract).

This is the exact `apu.rs → chiptune.rs` move one console over: the user's own,
**ymfm-cross-validated** code becomes native Orpheus graph nodes.

- **Pros:** zero new runtime deps; the seed is the user's battle-tested,
  reference-checked core, so we inherit its accuracy; fits ADR 0004 and the
  chiptune precedent; pure-Rust `no_std`-friendly math; full control over the
  authenticity switches (ladder on/off, headroom); testable to bit/level
  tolerances. SSG-EG and all 8 algorithms come "for free" from the port.
- **Cons:** we own the correctness of what we port (algorithm table, EG rate
  grid) and of the two additions (ladder constants, normalization); more code
  than a one-line duty table. The log-sine/exp ROM tables carry Nuked-OPN2
  provenance to honor (§10).
- **Verdict:** the clear house-style choice and the direct continuation of the
  NES arc. Port genesoxide's core; add the ladder + headroom; keep ymfm/Nuked as
  offline oracles only (§4.2).

### 4.2 Option (b) — FFI to a reference C/C++ core

- **Nuked-OPN2** (C) — cycle-accurate YM3438/YM2612, the gold standard for the
  ladder effect. **License: LGPL v2.1+.** Copyleft is a poor fit for a
  permissively-licensed, near-zero-dep Rust workspace: it would be the only
  copyleft component, complicates static linking/distribution, and drags in a C
  toolchain + `cc`/`bindgen`-style build deps onto an audio path that is
  currently pure Rust. Rejected on license + dependency grounds despite being
  the most accurate.
- **ymfm** (C++, Aaron Giles) — BSD-licensed, MAME-grade OPN/OPM/OPL cores.
  License is fine (permissive). But it is **C++**, so FFI needs a C shim + a C++
  toolchain in CI, adds a non-Rust dependency to a zero-dep audio path, and
  ships a full register-level multi-chip emulator (VGM-oriented, register-write
  driven) when we want a small note-driven FM voice. The impedance mismatch
  (register writes vs. our signal/patch model) is exactly the plumbing §2.7
  says to discard.
- **Verdict:** reject both for Phase 2. ymfm/Nuked are invaluable as **offline
  reference oracles** — generate a few hundred samples from a known patch and
  check the native kernel against them in a `#[ignore]`d/vendored fixture test —
  but neither should be a runtime dependency.

### 4.3 Option (c) — existing pure-Rust crates (verified)

Searched crates.io / GitHub and verified what these actually are:

- **`rust-synth-emulation`** (h1romas4) — an *experimental WebAssembly build
  wrapping the C++ ymfm library*, not a native-Rust FM synth. Same C++-FFI
  problem as (b), plus a WASM harness. Not usable as a graph node.
- **`tunes`** — a general audio crate that includes *generic* FM synthesis
  (plus granular, Karplus-Strong, wavetable). **Not YM2612-authentic**: no OPN2
  algorithms, EG rate grid, feedback semantics, or ladder effect. Wrong timbre
  and a large dependency for a feature we'd reimplement anyway.
- **`fm_mod_synth` / `rustysynth` / misc "ym2612" repos** — `rustysynth` is a
  SoundFont MIDI synth (unrelated); `fm_mod_synth` is a generic FM toy; most
  GitHub "ym2612" hits are VGM players or emulator sub-modules, not reusable
  synthesis crates.
- **Verdict:** as with `soundlog` in the NES arc, **no maintained pure-Rust
  YM2612-as-synth crate is a better fit than writing the kernel ourselves.** Do
  not add a synthesis dependency.

### 4.4 Recommendation

**Adopt option (a): port `madmax983/genesoxide`'s `ym2612.rs` FM math into a
native Rust `FmGenesisNode` in `orpheus-dsp/src/graph/`.** The core is reviewed,
clean, and ymfm-cross-validated (§3); port the operator/EG/algorithm/feedback/
SSG-EG/LFO kernel and the `.inc` ROM tables (crediting genesoxide and the tables'
Nuked-OPN2 provenance in the header, §10), drop all bus/mux/DAC/cpal/post-mix
plumbing (§3.2), and **add two things genesoxide lacks: the ladder crossover and
a headroom-safe single-voice normalization** (§3.3–§3.4). Keep Nuked-OPN2 and
ymfm as **offline test oracles only**. Add **zero** runtime dependencies. This
keeps the audio path pure-Rust, allocation-free, and lock-free, and matches the
chiptune precedent one console over.

## 5. Proposed graph-node API surface

### 5.1 The node — `FmGenesisNode` / `fm_genesis(...)`

An FM voice has far too many timbre parameters (4 ops × ~8 fields + algorithm +
feedback + LFO) to expose *every* one as an audio-rate signal input the way the
NES leaf nodes do. Pure params-as-signals (ADR 0004) would mean ~35 input
channels — unwireable and pointless, since a patch's `MUL`/algorithm/rate grid
are timbre-defining constants, not per-sample modulation.

**Proposed split** (a deliberate, documented deviation from ADR 0004 that
warrants a short ADR — see §9):

- **Construction-time patch** (a plain `FmPatch` config struct, like
  `sample_player`'s options): algorithm, feedback, per-op `MUL`/`DT`/`TL`/EG
  rates/SSG-EG/AM-enable, LFO rate, per-channel PMS/AMS, and `ladder`.
- **Signal inputs** (audio-rate, the small ergonomic surface):
  1. `gate` — key-on/off; drives all four operator EGs (mirrors how
     `compile_adsr` wires the voice gate).
  2. `freq_hz` — the note fundamental; each operator derives `freq_hz * MUL (± DT)`.
  3. `bright` — a macro that scales modulator `TL` (FM index / brightness),
     so `fm_genesis` responds to an LFO or envelope on timbre.
  4. `fb` — op-1 feedback amount, so feedback is playable/modulatable.
- **Outputs (1):** mono audio, bipolar `f32` ≈ `[-1, 1]` (channels are summed and
  ladder-processed internally; the raw ~14-bit range is scaled to `[-1, 1]`).

```rust
// crates/orpheus-dsp/src/graph/fm.rs  (new module, re-exported from graph/mod.rs)

pub struct FmOp {
    pub mul: u8,          // 0..=15  (0 => x0.5)
    pub detune: u8,       // 0..=7   (bit2 = sign)
    pub total_level: u8,  // 0..=127 (0 = loudest)
    pub ar: u8,           // 0..=31
    pub d1r: u8,          // 0..=31
    pub sl: u8,           // 0..=15
    pub d2r: u8,          // 0..=31
    pub rr: u8,           // 0..=15
    pub rate_scale: u8,   // 0..=3
    pub ssg_eg: u8,       // 0..=15 (0 = off)
    pub am_on: bool,      // LFO AMS applies to this op
}

pub struct FmPatch {
    pub algorithm: u8,    // 0..=7  (topology, §2.3)
    pub feedback: u8,     // 0..=7  (op-1, §2.4)
    pub ops: [FmOp; 4],
    pub lfo_rate: u8,     // 0..=7  (0 = LFO off)
    pub pms: u8,          // 0..=7  vibrato depth
    pub ams: u8,          // 0..=3  tremolo depth
    pub ladder: bool,     // §2.6 Model-1 grit (default true)
}

/// 4 inputs (gate, freq_hz, bright, fb), 1 output (audio).
/// Non-finite / non-positive sample rates fall back to 48 kHz, like the other leaves.
pub fn fm_genesis(sample_rate_hz: f32, patch: FmPatch) -> FmGenesisNode;
```

### 5.2 Preset ideas (named `FmPatch` factories)

Ship a `presets` module of canonical Genesis-flavored patches so livecoders
never hand-author 35 fields for the common cases:

| Preset | Character | Sketch |
|---|---|---|
| `epiano`  | DX-style electric piano | ALG 5, MUL 1:1:1:14, mild feedback, med decays, ladder on |
| `ebass`   | rubbery FM slap bass | ALG 4, two 2-op stacks, high feedback, fast attack/short decay |
| `brass`   | buzzy brass stab | ALG 2/3, bright modulators, PMS vibrato, slow-ish attack |
| `lead`    | cutting square-ish lead | ALG 0/4, high FM index, LFO vibrato |
| `bell`    | metallic bell/chime | ALG 0, inharmonic MUL ratios, long release |
| `drum`    | Genesis kick/snare/hat | ALG 7 + SSG-EG buzz, high feedback, very short EG |

### 5.3 Faust-combinator note

Op-1 feedback is conceptually the graph's `feedback()` recursive combinator, and
a 4-op algorithm is a `par`/`seq`/`merge` block diagram. We nonetheless
implement `FmGenesisNode` as a **single fused leaf** (like the chiptune nodes)
rather than composing eight nodes per voice: it keeps the hot loop tight,
allocation-free, and the algorithm table branch-predictable, and it keeps the
per-sample feedback exact. The combinator algebra remains available for anyone
who wants to wire operators by hand later.

## 6. Language exposure sketch (`voice{}`)

Mirror the established lowering path (`Saw`/`Pulse`/`PulseNes` → `VoiceNodeSpec`
→ `graph` node), exactly as `chiptune.md` §5 lays out:

1. **IR variant** in `crates/orpheus-dsp/src/graph_voice.rs` `VoiceNodeSpec`:
   add `FmGenesis { gate, freq, bright, fb, patch: FmPatch }` alongside `Pulse`
   / `PulseNes` / etc. Because `patch` is timbre config (not a signal ref),
   `inputs()` / `for_each_signal` enumerate only the four signal fields; the
   lowering match adds `VoiceNodeSpec::FmGenesis { patch, .. } =>
   par(fm_genesis(sample_rate_hz, patch.clone()), passthrough(bus))`. `FmPatch`
   must be serializable (it travels through `EngineCommand`).
2. **Language stage** in `crates/orpheus-lang/src/voice.rs` `compile_call`
   (the `pulse_nes` / `tri_nes` / `noise_nes` dispatch block): add
   `"fm_genesis"` to the dispatch and to the "unknown voice stage" help list,
   with a `compile_fm_genesis` builder. Two call shapes:

```text
fm_genesis("ebass")            # named preset; freq from the note, default bright/fb
fm_genesis("ebass", freq)      # preset + explicit pitch signal
fm_genesis("lead", freq, bright)
```

The first string argument selects a preset factory (§5.2) → an `FmPatch` baked
into the spec; remaining args bind the `freq` / `bright` / `fb` **signal**
inputs (ADR 0004), so `fm_genesis("lead", freq, lfo)` sweeps brightness from an
LFO. The `gate` input is wired from the voice's note gate, mirroring
`compile_adsr`'s gate handling. (String-literal args are new to the voice
surface — the NES stages take numbers — so `compile_fm_genesis` needs a small
name→preset lookup with an actionable "unknown preset" error listing the six
names. A raw all-numeric form can come later if hand-patching is wanted.)

**Naming note:** `fm_genesis` is a distinct name, not a reuse of any existing
stage; it coexists with `sine`/`pulse`/`pulse_nes`. The EG lives *inside* the
node (unlike NES nodes, which delegate amplitude to `adsr`), because the FM
envelope *rates and their key-scaling are the timbre* and must be pinned.

## 7. Testing strategy

Follow SPEC-PROOF-RED-GREEN-REFACTOR and mirror the chiptune test layout
(`crates/orpheus-dsp/tests/graph_chiptune.rs` for behavior,
`graph_chiptune_alloc.rs` for the counting-allocator proof). New files:
`graph_fm.rs` and `graph_fm_alloc.rs`.

- **Algorithm routing (the load-bearing pin).** For each of the 8 algorithms,
  configure operators so routing is observable (e.g. silence all non-carriers,
  or give each operator a distinguishable MUL/level) and assert which operators
  reach the output and which only modulate. A per-algorithm reference vector
  (short rendered buffer from a fixed patch) pins the exact topology — the FM
  analog of the chiptune "step sequence matches `DUTY_TABLE`" test.
- **Operator math.** `MUL` frequency ratios (including `MUL 0 => ×0.5`) land on
  the expected partials (FFT-peak or zero-crossing check); `DETUNE` sign/offset
  shifts pitch the right direction.
- **Envelope shapes.** Attack reaches full within the rate's expected window;
  `SL` knee is hit; `D2R = 0` holds; `RR` releases after key-off; `RS` makes a
  higher note's envelope measurably faster. Assert on the attenuation-domain
  breakpoints, not smoothed seconds.
- **Feedback.** `FB = 0` yields a clean sine at op-1's fundamental; increasing
  `FB` monotonically raises high-harmonic energy (spectral-centroid /
  harmonic-count assertion); output stays bounded (no runaway).
- **Ladder on/off.** With `ladder = false` the output is symmetric and clean;
  with `ladder = true` the documented crossover asymmetry/offset appears at low
  levels (assert the near-zero offset and the added low-level harmonic content).
  If a reference oracle (Nuked/ymfm) is vendored as a fixture, cross-check a few
  hundred samples of a known patch to a tolerance.
- **SSG-EG** (if in the first cut): the looping/mirroring envelope repeats at
  the expected period.
- **Headroom / peak (doubles as the genesoxide clipping diagnostic, §3.4).** For
  every preset and a fixed loud note, render offline and assert the absolute peak
  stays within `[-1.0, 1.0]` with a defined margin (e.g. `|peak| <= 0.99`) and
  that no sample is a hard-clipped rail. A single voice's ported math must never
  clip; if a headroom test ever fails, it localizes the fault to our
  normalization, not the operator/DAC math (which §3.4 proved clean). Pin peak
  *and* RMS so a future change to `FM_SCALE`/normalization is caught.
- **Zero-alloc proof.** A counting-`GlobalAlloc` test (verbatim pattern from
  `graph_chiptune_alloc.rs`) asserting `FmGenesisNode::process()` allocates
  **0** bytes across many blocks, for a representative patch.
- **Language round-trip.** In `orpheus-lang`: `fm_genesis("ebass")` compiles to
  the expected `VoiceNodeSpec::FmGenesis` with the right preset patch and signal
  wiring; an unknown preset name yields the actionable error; the spec survives
  `EngineCommand` serialization.

Optional `proofs/` entry only if a genuinely invariant property emerges (e.g.
output boundedness under feedback); the chiptune arc kept its pins as tests, and
FM can too.

## 8. PSG (SN76489) — noted follow-up

The Genesis also has a **Texas Instruments SN76489 PSG**: 3 square-wave tone
channels + 1 noise channel (a 15/16-bit LFSR, periodic or white), each with a
4-bit (16-step, non-linear/logarithmic) volume attenuator and a 10-bit tone
divider. It supplies the Genesis's arps, blips, and hi-hats that sit *over* the
FM bed.

This is **lower priority and a separate PR** but is architecturally the *easy*
one — it is almost exactly the NES pulse/noise story: hard square via a
divider + a duty-less 50% (SN has fixed 50% squares, unlike NES's 4 duties), a
4-bit **logarithmic** volume table (distinct from NES's linear 4-bit), and an
LFSR noise channel with periodic/white modes. It maps cleanly onto new leaf
nodes `psg_tone(sample_rate_hz)` and `psg_noise(sample_rate_hz)` following the
`pulse_nes` / `noise_nes` shape. genesoxide **already has** a PSG core
(`genesoxide-core/src/psg.rs`, including a 16-entry 2 dB-step logarithmic
`VOLUME_TABLE`) to seed the port. Recommend it as **Phase 3**, after the FM node
lands.

## 9. Is an ADR needed?

**Yes — a short one, unlike the chiptune nodes.** The NES nodes needed no ADR
because they were pure params-as-signals leaf nodes that fit ADR 0004 unchanged.
`FmGenesisNode` introduces a **new pattern**: a construction-time `FmPatch`
config struct carrying dozens of timbre parameters that are *not* signal inputs,
plus a preset-name string argument in the voice surface. That is a deliberate,
reasoned deviation from ADR 0004's "parameters are signal inputs" rule (§5.1),
and it will recur for any future rich-patch instrument (analog poly, wavetable,
sampler-synth). An ADR ("config-struct patches for high-parameter-count
instrument nodes") documents *when* a node may take a config struct instead of
signal inputs, and how such patches serialize through `EngineCommand`. It does
**not** cross the mixer/engine boundary (ADR 0003), so it stays a DSP-layer ADR.

## 10. Attribution

The FM operator kernel, EG timing, and algorithm connection table are ported from
**[`madmax983/genesoxide`](https://github.com/madmax983/genesoxide)**
`crates/genesoxide-core/src/ym2612.rs`; each ported source file must carry a
header crediting it, exactly as the chiptune nodes credit `madmax983/nes`
`apu.rs`. The **log-sine and exp ROM tables** (`ym2612_sin_table.inc`,
`ym2612_exp_table.inc`) are, per genesoxide's own code comments, derived from
**Nuked-OPN2** — so that provenance carries into Orpheus and must be noted in the
table headers. The added **ladder constants** likewise come from Nuked-OPN2 and
must be credited.

**License flag to resolve before Phase 2 merges:** Nuked-OPN2 is **LGPL v2.1+**.
Two mitigations, both standard: (1) the sine/exp values are a *hardware ROM* —
factual chip data, widely held not to be a creative work subject to copyright —
and identical tables are generatable from the documented log-sine/exp formulas,
so **regenerating the tables from first principles** (and saying so in the header)
sidesteps the question entirely; (2) genesoxide itself currently ships **no
LICENSE file**, which the user should fix upstream regardless. Recommendation:
**regenerate the ROM tables and the ladder constants from public formulas /
Nuked's documented algorithm** rather than copying the arrays verbatim, keeping
Orpheus's tree unambiguously permissive and zero-copyleft. Confirm the licensing
posture with the user before the implementation PR.
