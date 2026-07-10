# The Orpheus Tutorial

Welcome. Orpheus is a live-coding language for making music with *patterns* —
you type a line, you hear a sound, you edit the line, the sound changes at the
next cycle boundary. It draws on TidalCycles and Strudel for its pattern
language and on Faust for its signal graphs, and it runs in a terminal.

This tutorial takes you from your first sound to a full arrangement. Work
through it in order the first time; afterwards, treat it as a set of recipes.

## Learning path

1. **[Getting started](01-getting-started.md)** — build it, launch it, make your
   first sound, and understand what a binding is.
2. **[Patterns](02-patterns.md)** — the notation: sequences, rests, groups,
   alternation, speed, replication, degrade, stacks, polymeter, and euclid.
3. **[Transforms](03-transforms.md)** — the `|>` pipe and the functions that
   reshape a pattern: `fast`, `rev`, `every`, `jux`, `off`, `segment`, and the
   effect controls.
4. **[Instruments](04-instruments.md)** — the `voice { }` DSL: build a synth from
   oscillators, envelopes, and filters, then drive it with a note pattern.
5. **[Mixing and export](05-mixing-and-export.md)** — tracks, buses, sends, and
   getting audio, stems, or MIDI out of the system.
6. **[The Orca grid](06-orca-grid.md)** — a brief tour of the 2D grid surface for
   driving MIDI/OSC/UDP.
7. **[Putting it together](07-capstone.md)** — assemble everything into one
   playable `.ode` piece.

Looking up a specific builtin? See the
[language reference](../reference/language-reference.md) for the full registry.

## Launching the REPL

Orpheus is a single binary. From the workspace root:

```sh
cargo run --release
```

If both your terminal's input and output are a TTY, Orpheus opens its **ratatui
TUI**. If input or output is redirected (for example, when you pipe a script
in), it drops to a plain **stdio REPL**. Both evaluate the same language.

You can also load a startup script and then drop into the session:

```sh
cargo run --release -- song.ode
```

### Headless machines

Orpheus needs an audio device to actually make sound. On a machine without one
(CI, a container, an SSH session), it prints **`Audio Output Disabled`** and runs
with a silent stub engine. Everything else — evaluating bindings, `:render` to a
file, `:export` — still works, so you can develop patterns anywhere and only
need speakers to hear them live.

### The commands you use constantly

Inside the session, lines that start with `:` are commands; everything else is a
binding. The four you will use in every session:

| Command | What it does |
| --- | --- |
| `:play` | Start the transport clock (begin playing) |
| `:stop` | Stop the transport clock |
| `:tempo <bpm>` | Set the global tempo, e.g. `:tempo 140` |
| `:quit` | Leave the session |

Run `:help` at any time for the full command table. The rest of the commands
(`:mixer`, `:render`, `:midi`, `:export`, ...) are introduced in the chapters
that need them.

Ready? Start with **[Getting started](01-getting-started.md)**.
