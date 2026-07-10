# Orpheus examples

Example `.ode` scripts for the Orpheus live-coding environment. Run any of them
by passing the path to the `orpheus` binary, or by opening them from inside the
REPL with `:open <path>`.

> `.ode` files support `//` line comments — on their own line or trailing an
> expression. The example scripts are now self-documenting: per-line feature
> annotations live inline in the source. This README keeps the high-level
> overview and the play/render instructions; open the `.ode` files themselves
> for the line-by-line commentary.

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

`reference_song.ode` is now self-documenting: every binding carries an inline
`//` comment explaining what it does, and each section is introduced by a
comment banner. Open the file directly for the line-by-line tour.

At a glance, the song exercises: mini-notation rests (`~`), alternation
(`<…>`), fast repeat (`*n`), polymeter (`{…}%n`), Euclidean masks
(`mask(euclid(…))`), layering (`stack`, `jux`/`rev`), conditional transforms
(`every`, `whenmod`, `within`, `off`, `chunk`, `shuffle`), the randomness
family (`degrade`, `randcat`, `markov`, `pchoose`), sampled control signals
(`segment` + `range`), rational `fast`/`slow`, per-note voice parameters
(`p1`–`p2`, plus `param_ramp` automation), voice pragmas (`poly`, `steal`,
`release`), `gain`/`pan`, and Aeolian scale degrees (`degrees(aeolian, …)`).
