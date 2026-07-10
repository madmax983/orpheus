# Orpheus examples

Example `.ode` scripts for the Orpheus live-coding environment. Run any of them
by passing the path to the `orpheus` binary, or by opening them from inside the
REPL with `:open <path>`.

> `.ode` files currently have no comment syntax, so any per-line feature
> annotations for these examples live here in this README rather than in the
> source. Treat this file as living documentation kept in step with the scripts.

- `reference_song.ode` — the full-language reference song (documented below)
- `analog_showcase.ode` — analog-style synth voice showcase
- `phase5_escape_hatch.ode` — escape-hatch feature demo

---

## Reference song

`reference_song.ode` is a musical A-minor (Aeolian) acid-techno reference
exercise. It is deliberately built to touch the full language surface in one
coherent piece rather than as isolated snippets:

- **Three custom `voice { … }` instruments** synthesized from graph combinators:
  - `acid` — the acid bass: `saw` oscillator through a resonant low-pass
    (`svf_lp`), `drive` saturation, and an ADSR-gated amp, with 8-voice polyphony,
    oldest-note stealing, and a 20 ms parameter ramp (`param_ramp`) for smooth
    per-note filter automation.
  - `pad` — a saw+tri pad with a feedback delay tail and a slow ADSR swell.
  - `pluck` — a tri+pulse pluck through a low-pass with a fast AR envelope.
- **A sample drum/percussion bed** built from the stock sample tokens
  (`bd`, `hh`, `cp`, `sn`) as `kick`, `kick_fill`, `hats`, `hats_busy`,
  `open_hat`, `clap`, `snare`, and `perc`.
- **Seven sections** arranged on a **36-cycle** timeline via `seq_sections`:
  `intro` (4) → `build` (4) → `drop` (8) → `peak` (8) → `melodic` (4) →
  `breakdown` (4) → `outro` (4). The `song` binding is the entry point.

At the default tempo of **120 BPM** each cycle is 2 seconds, so the rendered
reference is **72 seconds** long, **48 kHz stereo**.

### Play it live (REPL)

Launch the binary with the file as its startup argument — it loads the script
and starts playing:

```sh
cargo run --release -- docs/examples/reference_song.ode
```

Or start the REPL with no arguments and load the file from the prompt:

```sh
cargo run --release
```

```text
:open docs/examples/reference_song.ode
```

Both paths evaluate every top-level binding and make `song` the active pattern.
(If no audio device is available, Orpheus prints an "Audio Output Disabled"
warning and continues headless — offline rendering below still works.)

### Render it offline to a WAV

Load the song, then use the `:export master` command to render the full mix —
the user voices, the drum bed, and any bus returns — down to a single stereo
WAV file:

```text
:open docs/examples/reference_song.ode
:export master reference_song_master.wav 36
```

`:export master <path> [cycles] [--no-buses]` renders `cycles` cycles of the
current mix (bus reverb/delay tails are folded in by default; pass `--no-buses`
for a dry, track-only master). Rendering all 36 cycles yields the full
72-second, 48 kHz stereo master.

To render each track to its own file instead of one master sum, use
`:export stems [cycles] [--buses]`, which writes per-track WAVs to an
`exports/stems-<timestamp>/` directory:

```text
:export stems 36
```

### What each section demonstrates

The feature map below is cross-checked against the bindings in
`reference_song.ode`. It is a representative tour, not an exhaustive list.

| Area | Where | What it shows |
| --- | --- | --- |
| Rests `~` | `kick` (`bd ~ bd ~`), `snare`, `clap`, `open_hat`, `breakdown` | mini-notation rests |
| Alternation `<…>` | `kick_fill` (`bd ~ bd <bd cp>`) | pick a different step each cycle |
| Fast repeat `*n` | `hats` (`hh*8`), `hats_busy` (`hh*16`) | subdivide a step |
| Polymeter `{…}%n` | `perc` (`{hh hh hh, cp ~}%8`) | independent layers over a shared step count |
| Euclid via mask | `hats` (`mask(euclid(5,8))`), `hats_busy` (`mask(euclid(11,16))`) | Euclidean rhythms applied as a boolean mask |
| `stack` | every `section` (`intro`, `build`, `drop`, …), `pad_chord` | layer patterns simultaneously |
| `every` | `hats_busy` (`every(4, fast(2))`) | apply a transform every N cycles |
| `whenmod` | `bass_evolve` (`whenmod(8, 4, rev)`) | conditional transform by cycle modulus |
| `within` | `pad_lift` (`within(0, 0.5, rev, …)`) | transform only part of the cycle |
| `off` | `lead` (`off(0.125, pitch(12))`) | time-shifted, transposed copy |
| `chunk` | `lead_choose` (`chunk(4, fast(2))`) | rotate a transform across chunks |
| `shuffle` | `lead_evolve` (`shuffle(4)`) | reorder steps |
| `jux` / `rev` | `perc` (`jux(rev)`) | stereo-split with a reversed right channel |
| `degrade` | `hats_busy` | randomly drop events |
| `randcat` | `lead_evolve` (`randcat(lead_a, lead_b)`) | random cycle-wise concatenation |
| `markov` | `lead_wander` (`markov(…)`) | Markov-chain pitch walk |
| `pchoose` | `lead_choose` (`pchoose(…)`) | weighted random choice per event |
| `segment` + `range` | `filter_env` (`segment(16, rand) \|> range(320, 3600)`) | sampled control signal mapped to a range |
| Rational `fast` / `slow` | `bass_deep`/`bass_solo` (`slow(2)`), `bass_drive` (`fast(2)`) | rational time-scaling |
| Per-note params `p1`–`p2` + intra-note automation | `bass_drive` (`p1(300 700 1500 …)`), `bass_evolve` (`p1(filter_env)`), `lead` (`p2`), `acid` `param_ramp` | patterned per-note voice parameters, automated within and across notes |
| Voice pragmas | `acid`/`pluck` (`poly`, `steal = oldest`), `pad` (`release`) | polyphony, voice-stealing, and release pragmas |
| `gain` / `pan` | throughout (`gain(0.35)`, `pan(0.15)`) | per-pattern level and stereo placement |
| Scale degrees | `bass_deg`, `arp_deg` (`degrees(aeolian, …)`) + `pitch` | Aeolian degree sequences driving melody/bass |
