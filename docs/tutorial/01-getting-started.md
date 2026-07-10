# 1. Getting started

By the end of this chapter you will have Orpheus built, running, and playing a
drum pattern you typed yourself.

## Build it

Orpheus is a Cargo workspace, so you need a Rust toolchain (`rustup` is the
easy path). On **Linux you also need the ALSA development headers** — the audio
backend links against them:

```sh
sudo apt-get install -y libasound2-dev
```

Then, from the repository root:

```sh
cargo run --release
```

The first build takes a couple of minutes; after that it is cached. When it
finishes, Orpheus launches. If you are on a machine with speakers you will hear
silence (nothing is playing yet); if you are on a headless box you will see
`Audio Output Disabled` and a note that it is running with a stub engine — that
is fine, keep going.

## The prompt

Orpheus greets you with a prompt:

```text
>
```

Everything you type here is one of two things:

- A **binding** — a name, an `=`, and a pattern expression. This is the music.
- A **command** — a line starting with `:`. This controls the session.

## Your first sound

Type a binding and press enter:

```text
> d1 = bd sn cp sn
```

Orpheus responds with a confirmation like `✓ bound d1`. You just described a
one-cycle pattern with four events: a **b**ass **d**rum, a **s**nare, a hand
**c**la**p**, and a snare again, evenly spaced across the cycle.

`bd`, `sn`, `cp`, and `hh` are built-in sample tokens (kick, snare, clap,
hi-hat). The whitespace between them means "play these one after another, evenly
dividing the cycle" — so four tokens means four quarter-cycle steps.

Nothing is playing yet, though. Start the clock:

```text
> :play
```

Now you hear the loop. Set a tempo you like:

```text
> :tempo 120
```

`:tempo` takes beats per minute; a cycle is four beats, so at 120 BPM each
four-event cycle takes two seconds.

## Edit live

Redefine `d1` while it plays — bindings hot-swap at the next cycle boundary, so
the change lands in time, not mid-beat:

```text
> d1 = bd bd sn bd
```

Add a second layer with its own name. Every binding you make plays
simultaneously:

```text
> hats = hh hh hh hh
```

You now have a kick/snare pattern and a steady hat line running together.

## Stop and leave

```text
> :stop
> :quit
```

`:stop` halts the transport but keeps your bindings; `:play` resumes them.
`:quit` exits.

## What just happened

- A **cycle** is the fundamental unit of time — one loop. Tempo sets how long a
  cycle lasts.
- A **binding** (`name = pattern`) both defines a pattern and, in a live
  session, schedules it to play.
- **Whitespace is sequencing**: `bd sn cp sn` divides one cycle into four equal
  events. This is the whole basis of the notation, and the
  [next chapter](02-patterns.md) shows everything you can do with it.

> Two flavours of evaluation: in the live REPL, Orpheus uses **loose** typing
> (permissive, quick to experiment). Files you load with `:open` or pass on the
> command line are evaluated in **strict** mode. Every example in this tutorial
> works in the REPL; the capstone is a strict-mode `.ode` file.

Next: **[Patterns](02-patterns.md)**.
