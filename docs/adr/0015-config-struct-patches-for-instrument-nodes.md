# ADR 0015: Config-Struct Patches for High-Parameter-Count Instrument Nodes

- Status: Accepted
- Date: 2026-07-11

## Context

ADR 0004 established the Faust "parameters as signal inputs" model for DSP graph
nodes: a filter's cutoff is an input channel, not a struct field, so every
parameter is modulatable and the combinator algebra handles all routing. This
works cleanly for the existing leaf nodes, which each expose a handful of
inputs (`saw` has 1, `ladder_filter` has 3, `pulse_nes` has 3).

The Sega Genesis YM2612 FM voice (`FmGenesisNode`, `crates/orpheus-dsp/src/graph/genesis_fm.rs`,
implementing the design in `docs/design/genesis-fm.md`) breaks that assumption.
A four-operator FM voice is described by roughly 35 parameters: per operator
(×4) a frequency multiple, detune, total level, four envelope rates, sustain
level, key-scaling, SSG-EG mode, and AM-enable; plus a channel algorithm,
feedback, LFO rate, PMS, AMS, and the ladder switch. Exposing all of these as
audio-rate signal inputs would mean a ~35-channel node that is unwireable in
practice and pointless in principle: a patch's algorithm, MUL ratios, and
envelope rate grid are timbre-*defining constants* selected once, not per-sample
modulation. This same shape will recur for any future rich-patch instrument
(analog poly, wavetable, sampler-synth).

## Decision

Allow a DSP graph instrument node to take a **construction-time config struct**
("patch") for its timbre-defining constants, while keeping a small, deliberately
chosen set of genuinely performable parameters as signal inputs.

For `FmGenesisNode`:

- **Construction-time patch** — `FmPatch` (a plain `Clone + Copy + Debug + PartialEq`
  struct, like `sample_player`'s options): `algorithm`, `feedback`, the four
  `FmOp`s (MUL / DT / TL / EG rates / SSG-EG / AM-enable), `lfo_rate`, `pms`,
  `ams`, and `ladder`. Named preset constructors (`epiano`, `ebass`, `brass`,
  `lead`, `bell`, `drum`) return ready-made patches so livecoders never
  hand-author 35 fields.
- **Signal inputs** — the four-channel ergonomic surface: `gate` (key-on/off,
  drives all operator envelopes), `freq_hz` (note fundamental), `bright`
  (FM-index / modulator-level macro), and `fb` (playable op-1 feedback). These
  are the parameters a performer actually modulates.

### Scope and boundaries

- This is a **DSP-layer** decision. It does not cross the mixer/engine boundary
  (ADR 0003); the patch is owned by the node.
- When such a patch travels through `EngineCommand` (the language-exposure PR,
  not this one), it must be `Clone` and carry no borrowed state — `FmPatch` is
  `Copy`, satisfying that ahead of time.
- Nodes that fit ADR 0004's all-signals model (the NES chiptune leaves, the
  oscillators, the filters) stay as they are. A config-struct patch is reserved
  for nodes whose parameter count and constant-ness make per-parameter signal
  inputs impractical.

## Consequences

- **Positive:** FM voices are livecoding-ergonomic (a preset name plus four
  performable signals); the hot loop stays tight and branch-predictable with the
  patch baked in; the pattern generalizes to future high-parameter instruments.
- **Negative:** a documented, intentional deviation from ADR 0004's uniform
  "everything is a signal" rule — two node categories now exist (pure-signal
  leaves vs. config-struct instruments). The line between them ("is this
  parameter a per-sample control or a timbre constant?") is a judgment call made
  per node.
- **Neutral:** timbre parameters are not audio-rate modulatable on these nodes
  by design; anything that genuinely needs to be swept is promoted to a signal
  input (as `bright` and `fb` were).
