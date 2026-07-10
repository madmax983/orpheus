# 🎻 Orpheus

**A livecoding environment where music is a program.** Orpheus is a cycle-based
live-coding audio workspace written in Rust: you type pattern expressions into a
terminal REPL and hear them immediately. It pairs full TidalCycles-style pattern
parity with a Faust-style DSP graph, a user-defined `voice { }` instrument
language, an Orca grid surface, and real MIDI / OSC / UDP output.

The core premise: **music is structure, and structure is best expressed in
code.** Patterns are expressions, transformations are functions, and a song is a
program that produces sound.

## ⏱️ A 10-second taste

Launch the REPL (`cargo run --release`) and type:

```text
> :tempo 120
> drums = bd sn cp sn
> hats  = hh*8 |> gain(0.6)
> pluck = voice { saw(freq) * ar(gate, 0.001, 0.2) }
> bass  = pluck ~ pluck ~
> :play
```

`drums` is a four-event drum pattern; `hh*8 |> gain(0.6)` speeds a hat token to
eight hits and pipes it through a gain control; `pluck` is a one-line synth voice
you just defined; and `bass` schedules that voice as a pattern token next to `~`
rests. `:play` starts the clock. Every line here is real, evaluated syntax —
these snippets are checked in
[`crates/orpheus-lang/tests/readme_examples.rs`](crates/orpheus-lang/tests/readme_examples.rs)
so the README can't drift from the language.

## ✨ Highlights

*   **Real pattern grammar.** Not quoted mini-notation strings — Orpheus has a
    pest grammar, so notation is bare source: sequences (`bd sn cp`), groups
    `( )`, stacks `,`, alternation `<a b c>`, `*`/`/` speed, `!` replicate, `?`
    degrade, polymeter `{a b, c d e}%n`, and inline euclid `bd(3,8)`.
*   **~100 pattern transforms.** `every`, `when`, `within`, `jux`, `off`,
    `chunk`, `iter`, `rev`, `fast`/`slow`, `euclid`, `degrade`, `sometimes`,
    `chord`, `arp`, `strum`, `lsystem`, `wolfram`, scales and microtonal
    `tuning`/`load_scl`, and more — all composable left-to-right with `|>`.
*   **Faust-style DSP graph.** A `graph { }` block and a combinator layer
    (`seq`, `par`, `split`, `merge`, `feedback`) build signal DAGs from
    oscillators, SVF/biquad filters, delays, and sample players (ADR 0004).
*   **User-defined instruments.** The `voice { }` DSL compiles synth definitions
    to pooled graph voices, hot-swapped at cycle boundaries, with polyphony,
    voice-stealing, and per-note `p1`–`p4` parameter automation (ADR 0010).
*   **Orca grid surface.** A 100r.co-style 2D glyph grid livecoding layer over
    the same rational-time model, with the full `A`–`Z` operator set and IO
    operators.
*   **Real IO out.** MIDI, OSC, and UDP output (plus MIDI clock) run on a
    dedicated transport thread, never on the audio thread. MIDI/audio file
    export via `:export` and `:render`.
*   **TUI or plain REPL.** A ratatui terminal UI with per-track level meters and
    transcript scrollback, or a stdio REPL when piped. Loose typing in the REPL
    for fast experimentation, strict typing in `.ode` files for durable songs.

## 🚀 Install & run

Orpheus is built entirely in Rust with Cargo.

### Prerequisites

On Linux you need the ALSA development headers (used by both the audio backend
and MIDI):

```bash
sudo apt-get install -y libasound2-dev
```

### Run

```bash
cargo run --release                 # launch the TUI / REPL
cargo run --release -- song.ode     # evaluate a startup .ode file, then drop into the REPL
```

If both stdin and stdout are terminals, Orpheus opens the ratatui **TUI**;
otherwise it runs the plain **stdio REPL** (so piping input drives the line
REPL). If no audio device is available it prints an "Audio Output Disabled"
notice and runs headless on a stub engine, so it still evaluates patterns
without sound hardware.

Common REPL commands: `:tempo <bpm>`, `:play` / `:stop`, `:mixer`,
`:track` / `:bus` / `:send`, `:render`, `:export`, `:roll`, `:midi`, `:help`,
`:quit`.

## 📚 Learn more

*   **Getting started + tutorial:** [`docs/tutorial/`](docs/tutorial/) — a
    hands-on walkthrough from your first pattern to voices and the grid.
*   **Language & builtin reference:**
    [`docs/reference/language-reference.md`](docs/reference/language-reference.md)
    — every builtin, notation form, the `voice { }` / `graph { }` DSL, REPL
    commands, and Orca operators.
*   **Feature index:** [`docs/plans/parity-roadmap.md`](docs/plans/parity-roadmap.md)
    tracks TidalCycles + Faust parity and points at each impl site.
*   **Design & decisions:** the overall
    [design document](docs/design/orpheus_design.md), the
    [Orca surface design](docs/design/orca-surface.md), and the
    [architecture decision records](docs/adr/) (notably ADR 0004 on the DSP
    graph and ADR 0010 on the voice language).
*   **Runnable examples:** [`docs/examples/`](docs/examples/).

## 📦 Workspace layout

Orpheus is a Cargo workspace: a root binary plus three library crates.

*   **`orpheus` (root binary):** session entrypoint — sets up the cpal audio
    stream, parses CLI arguments, and launches the TUI or stdio REPL.
*   **`orpheus-pattern`:** the temporal core — exact rational time
    (`TimeSpan`, `Rational`), cycle/stream patterns, and query semantics (with
    Verus proofs).
*   **`orpheus-dsp`:** audio scheduling and rendering — the real-time engine,
    routing snapshots, sample bank, synth primitives, and the Faust-style graph
    combinators.
*   **`orpheus-lang`:** parser, evaluator, type inference, builtins, mixer, Orca
    surface, REPL, and TUI.

## 🧠 Philosophy

*   **Patterns are first-class:** every musical concept is a pattern that can be
    queried, composed, and transformed.
*   **Juxtaposition over ceremony:** the most common operation — sequencing —
    requires the least syntax.
*   **Explicit composition:** layering, transformation, and structure use
    explicit keywords and the `|>` pipe.
*   **Dual-mode typing:** loose inference in the REPL for rapid experimentation,
    strict inference in `.ode` files for durable artifacts.

## 🛠️ Development

```bash
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
cargo doc --workspace --open        # API docs
```

Behavior changes follow SPEC-PROOF-RED-GREEN-REFACTOR: add or update a test
before adding runtime behavior. Architecture decisions live in `docs/adr/`, and
the audio-thread path stays allocation-free and lock-free.
