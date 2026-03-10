# Orpheus: A Pattern-Based Music Programming Language

**Author:** Mark  
**Status:** Draft  
**Date:** March 2026

---

## 1. Vision

Orpheus is a Rust-based music programming language designed for composers who think in code. It provides a pattern-based composition model inspired by TidalCycles and Strudel, a custom DSP engine built on Faust's block diagram algebra, and a ratatui terminal interface that functions as both REPL and session view.

The core premise: music is structure, and structure is best expressed in code. Piano rolls and DAW timelines impose a spatial metaphor on an inherently temporal art. Orpheus treats composition as programming — patterns are expressions, transformations are functions, and a song is a program that produces sound.

### 1.1 Design Principles

- **Patterns are first-class.** Every musical concept — rhythm, melody, harmony, control signals — is a pattern that can be queried, composed, and transformed uniformly.
- **Juxtaposition over ceremony.** The most common operation (sequencing) should require the least syntax.
- **Explicit composition.** Parallel layering, transformation, and structure use keywords, not overloaded symbols.
- **Learning through building.** The DSP layer is hand-built, not wrapped. Understanding digital audio is a first-order goal alongside producing music.
- **Dual-mode typing.** Loose inference in the REPL for rapid experimentation; strict inference in `.ode` files for durable artifacts.

### 1.2 Architectural Layers

Orpheus is structured as three distinct layers, each its own crate:

| Layer | Crate | Responsibility |
|-------|-------|---------------|
| **DSP Engine** | `orpheus-dsp` | Audio graph: oscillators, filters, envelopes, effects. Faust-style block diagram combinators for wiring. Runs on the audio thread. |
| **Pattern Engine** | `orpheus-pattern` | Temporal scheduling: the `Pattern<T>` trait, cycle-based and event-stream implementations, pattern combinators and transformations. Produces events that drive the DSP layer. |
| **Language & UI** | `orpheus-lang` | pest grammar, Hindley-Milner type inference, REPL, ratatui TUI, file compilation. The surface that the musician interacts with. |

Each layer can be developed and tested independently. The pattern engine can drive a trivial beep oscillator. The DSP engine can be exercised without the language. The language can be parsed and type-checked without producing audio.

---

## 2. Language Design

### 2.1 Syntax Overview

Orpheus uses juxtaposition for sequencing with a disambiguation rule: a run of identifiers/groups in pattern position forms an implicit sequence. Function application requires parentheses. This means the parser distinguishes between pattern position (where juxtaposition means sequencing) and expression position (where standard evaluation rules apply).

```orpheus
-- Binding
kick = bd ~ bd ~

-- Juxtaposition is sequencing: four events, equally spaced in the cycle
drums = bd sn cp sn

-- Grouping changes subdivision: sn and cp share the second slot
drums = bd (sn cp) bd sn

-- Rest is ~
sparse = bd ~ ~ sn ~ ~ cp ~

-- Parallel composition is explicit
drums = stack(
  bd ~ bd ~,
  ~ sn ~ sn,
  ~ ~ cp ~
)

-- Transformation via pipe operator
drums = bd sn cp sn
  |> every(4, fast(2))
  |> rev

-- Named patterns compose freely
verse_drums = stack(kick_pattern, snare_pattern, hat_pattern)
  |> gain(0.8)

chorus_drums = verse_drums
  |> fast(2)
  |> every(2, rev)
```

### 2.2 Grammar Rules

The core disambiguation rule:

- **Pattern position:** Top-level of a binding's right-hand side, inside `stack(...)` layers, inside grouping parens when in pattern context. Here, juxtaposition means `seq`.
- **Expression position:** Inside function argument lists (after `(`). Here, standard evaluation applies — identifiers resolve to values, no implicit sequencing.

A run of bare identifiers like `bd sn cp` is parsed as `Seq([bd, sn, cp])`. An identifier followed by `(` triggers function-call parsing: `fast(2)` is `Apply(fast, [2])`. The pipe operator `|>` is left-associative and low-precedence, threading the left-hand pattern into the last argument of the right-hand function.

```
-- This is a sequence of three events:
bd sn cp

-- This is a function call:
fast(2)

-- This is a function call receiving a pattern:
every(4, fast(2))

-- Pipe threads the LHS as last argument:
bd sn cp |> fast(2)
-- desugars to: fast(2, bd sn cp)

bd sn cp |> every(4, fast(2))
-- desugars to: every(4, fast(2), bd sn cp)
```

### 2.3 Pipe Operator Semantics

The pipe operator `|>` threads the left-hand side as the **last** argument to the right-hand function. This is deliberate — transformation functions follow the convention that the pattern being transformed is always the final parameter, enabling partial application on the left:

```orpheus
-- every(n, transform) returns a Pattern -> Pattern function
-- Pipe fills in the final pattern argument
drums |> every(4, fast(2))

-- Chaining reads top-to-bottom as a transformation pipeline
drums = bd sn cp sn
  |> every(4, fast(2))     -- every 4th cycle, double speed
  |> sometimes(rev)         -- randomly reverse
  |> gain(0.7)              -- reduce volume
```

### 2.4 The `stack` Keyword

`stack` is the parallel composition operator. It takes multiple pattern layers and plays them simultaneously. Layers are separated by commas. Each layer occupies the same time span (one cycle by default), and their events are merged.

```orpheus
-- Inline (comma-separated)
drums = stack(bd ~ bd ~, ~ sn ~ sn, hh hh hh hh)

-- Multi-line (commas still required)
drums = stack(
  bd ~ bd ~,
  ~ sn ~ sn,
  hh hh hh hh
)

-- From named patterns
drums = stack(kick, snare, hats)
```

In strict mode (`.ode` files), all layers in a `stack` must unify to the same `Pattern<T>`. In REPL mode, mismatched types are coerced where reasonable.

### 2.5 Control Signals as Patterns

Following TidalCycles' key insight, control parameters are themselves patterns. Filter cutoff, gain, pan — anything that modulates a sound is a `Pattern<Number>` with its own rhythm and structure.

```orpheus
-- Static control
bass |> lpf(800)

-- Patterned control: cutoff sweeps through values each cycle
bass |> lpf(400 800 400 1200)

-- Control patterns can be transformed independently
bass |> lpf(400 800 400 1200 |> slow(2))
```

This means the type of `lpf` is `Pattern<Number> -> Pattern<T> -> Pattern<T>` — it takes a patterned number and a source pattern, returning a transformed pattern. A bare number literal like `800` is implicitly a constant `Pattern<Number>`.

### 2.6 Escape Hatch: Event Streams

The default time model is cycle-based: patterns subdivide a repeating unit of time. But an escape hatch exists for music that doesn't fit cyclic structure — ambient passages, generative pieces, scored sections with explicit timing.

```orpheus
-- Cycle-based (default)
melody = C4 E4 G4 C5

-- Event stream: explicit rational-time placement
ambient = stream(
  at(0.0, bd),
  at(2.5, sn),
  at(6.0, cp)
)

-- They compose because both implement Pattern<T>
mix = stack(melody, ambient)
```

Both cycle patterns and event streams implement the same `Pattern<T>` trait. The query interface is identical: given a time span `(start, end)`, return all events within it. The mixer, transformations, and the rest of the system are agnostic to which time model produced the events.

### 2.7 Meter Annotation

Cycles carry optional meter metadata that bridges abstract cycle-time and musical beat-time. This enables the event-stream escape hatch to reference beats as a unit.

```orpheus
-- Default: 4/4, one cycle = one bar = 4 beats
melody = C4 E4 G4 C5

-- Explicit meter
waltz = meter(3, 4) (C4 E4 G4)   -- one cycle = 3 quarter-note beats

-- Event streams can reference beats because meter is known
bridge = meter(4, 4) stream(
  at(beat(0), bd),
  at(beat(2), sn),
  at(beat(2.5), cp)
)
```

The pattern engine itself doesn't care about meter — it operates on cycle-relative time (0 to 1, repeating). Meter is metadata that the language layer uses to convert beat-relative literals into cycle-relative values. This keeps the core clean while giving the surface language musical vocabulary.

Current parser note:

- `meter(3, 4) pattern` is supported as prefix annotation syntax.
- `meter(3, 4, pattern)` is also accepted for compatibility.

---

## 3. Type System

### 3.1 Type Universe

The type system is small and inferred. No type annotations are required; Hindley-Milner infers everything.

| Type | Description |
|------|------------|
| `Pattern<T>` | Core type. A queryable temporal structure of events of type `T`. |
| `Sample` | Reference to an audio sample (e.g., `bd`, `sn`, `cp`). |
| `Note` | Pitch value. Named (`C4`, `Eb3`) or MIDI number. |
| `Number` | `f64`. Used for control signals, durations, frequencies. |
| `Duration` | Rational time value (internally a ratio). |
| `String` | String literal. Primarily for sample paths and labels. |
| `T -> U` | Function type. Transformations are `Pattern<T> -> Pattern<T>`. |
| `()` | Unit. Used for side-effecting operations (e.g., REPL commands). |

`Pattern<T>` does the heavy lifting. Most values in Orpheus are patterns. A bare `bd` is `Pattern<Sample>`. A bare `C4` is `Pattern<Note>`. A bare `440` in pattern position is `Pattern<Number>`. Transformations like `fast`, `slow`, `rev` are polymorphic: `Number -> Pattern<T> -> Pattern<T>`.

### 3.2 Dual-Mode Inference

The same Hindley-Milner inference engine runs in both modes. The difference is the error policy in the constraint solver:

**REPL mode (loose):**
- Coercions are applied silently: `Number` → `Note` (MIDI pitch), `Number` → `Pattern<Number>` (constant pattern), `Note` → `Pattern<Note>`.
- Unresolved names produce warnings, not errors. The pattern plays with a placeholder sound.
- Partial definitions are allowed. Type variables that can't be resolved default to `Number`.
- Attitude: play the sound, figure out types later.

**File mode (strict — `.ode` files):**
- No implicit coercions. `stack(bd sn, C4 E4)` is a type error (`Pattern<Sample>` vs `Pattern<Note>` don't unify).
- Unresolved names are errors.
- All type variables must resolve.
- Attitude: if it compiles, the structure is sound.

Implementation: a single `unify(a, b, mode)` function in the constraint solver. In loose mode, when unification fails, it attempts a coercion table before reporting an error. In strict mode, it reports immediately. The AST, parser, and inference algorithm are identical across modes.

### 3.3 Workflow

The intended workflow mirrors the design:

1. **Sketch** in the REPL. Types are inferred loosely. Experiment freely.
2. **Commit** by pasting into a `.ode` file. The compiler enforces strict inference.
3. **Import** committed `.ode` files back into the REPL as trusted building blocks.

Strict-mode artifacts become a library of solid, typed definitions. The REPL pulls them in and riffs on them without friction. The hyperfocus session produces the `.ode` files; later sessions can explore them loosely.

---

## 4. Pattern Engine

### 4.1 Core Trait

```rust
/// A temporal pattern of events.
/// The fundamental query: "what happens between time `start` and `end`?"
pub trait Pattern<T>: Send + Sync {
    fn query(&self, span: TimeSpan) -> Vec<Event<T>>;
}

/// A time span defined by rational start and end points.
pub struct TimeSpan {
    pub start: Rational,
    pub end: Rational,
}

/// An event: a value that occupies a time span.
pub struct Event<T> {
    /// The "whole" — the complete time span this event conceptually occupies.
    pub whole: Option<TimeSpan>,
    /// The "part" — the portion of the whole that falls within the query span.
    pub part: TimeSpan,
    /// The value.
    pub value: T,
}
```

The `whole` vs `part` distinction (borrowed from TidalCycles) is critical. When a cycle boundary bisects an event, the `whole` records its original extent while `part` records the fragment within the queried span. This enables correct handling of long notes that span multiple queries without double-triggering.

### 4.2 Cycle-Based Patterns

The default pattern type. Time is a repeating 0→1 cycle. Juxtaposition subdivides it equally.

```rust
pub struct CyclePattern<T> {
    pub events: Vec<PatternNode<T>>,
}

pub enum PatternNode<T> {
    Atom(T),
    Rest,
    Group(Vec<PatternNode<T>>),  // subdivision
    // ...
}
```

`query(0.0, 1.0)` on `bd sn cp` returns three events at `[0, 1/3)`, `[1/3, 2/3)`, `[2/3, 1)`. `query(1.0, 2.0)` returns the same events shifted by one cycle. The pattern is infinite and repeating.

### 4.3 Event Stream Patterns

The escape hatch. Events have explicit onset times and durations expressed as rationals. No repeating cycle — the pattern is finite (or generative).

```rust
pub struct EventStream<T> {
    pub events: Vec<(Rational, Duration, T)>,  // (onset, duration, value)
}
```

`query(start, end)` returns events whose onset falls within `[start, end)`. Implements the same `Pattern<T>` trait, so it composes with cycle-based patterns via `stack`, transformations, etc.

### 4.4 Transformations

Transformations are functions from `Pattern<T>` to `Pattern<T>`, implemented by modifying the query:

| Transform | Semantics | Implementation |
|-----------|-----------|----------------|
| `fast(n)` | Speed up by factor `n` — `n` cycles in the time of one. | Scale the query span by `n` before delegating. |
| `slow(n)` | Slow down by factor `n`. | Scale the query span by `1/n`. |
| `rev` | Reverse the pattern within each cycle. | Mirror the query span around the cycle midpoint. |
| `every(n, f)` | Apply `f` only every `n`th cycle. | Check cycle number; delegate to `f` or identity. |
| `sometimes(f)` | Apply `f` with 50% probability per cycle. | Deterministic PRNG seeded by cycle number. |
| `shift(n)` | Rotate pattern forward by `n` within the cycle. | Offset the query span. |
| `degrade` | Randomly drop events. | Filter events using cycle-seeded PRNG. |
| `jux(f)` | Apply `f` to one stereo channel. | Pan original left, `f(pattern)` right. |

Transformations compose via the pipe operator. The implementation is purely functional — each transformation wraps the inner pattern and modifies the query, building a chain of query-rewriting closures.

### 4.5 Deterministic Randomness

"Random" operations (`sometimes`, `degrade`, `shuffle`) must be deterministic — the same cycle always produces the same result. This is essential for live coding: modifying one part of a pattern shouldn't change the random choices in another part.

Implementation: each random operation uses a PRNG seeded by a hash of the cycle number and the operation's position in the AST. This gives the illusion of randomness while maintaining referential transparency.

---

## 5. DSP Engine

### 5.1 Philosophy

The DSP layer is hand-built for learning, not wrapped from an existing library. The goal is first-principles understanding of digital audio: how oscillators alias, why filter stability matters, what makes an envelope click-free. `fundsp`'s source serves as reference and study material, not a dependency.

### 5.2 Block Diagram Algebra

Signal processing nodes compose using Faust's five operators:

| Operator | Syntax | Semantics |
|----------|--------|-----------|
| Sequential | `a : b` | Output of `a` feeds input of `b`. |
| Parallel | `a , b` | `a` and `b` run side by side, no connection. |
| Split | `a <: b` | Output of `a` is copied to all inputs of `b`. |
| Merge | `a :> b` | Multiple outputs of `a` are summed into inputs of `b`. |
| Recursive | `a ~ b` | Feedback: output of `a` feeds `b`, output of `b` feeds back to `a` (one sample delay). |

These operators are internal to the DSP layer — they're how synths and effects are defined in Rust, not exposed directly in the Orpheus language. The language interacts with the DSP layer through named synths and effects that are pre-wired graphs.

```rust
// Example: a simple subtractive synth defined with combinators
// saw oscillator -> low-pass filter -> ADSR envelope
let synth = saw() | lpf() | adsr(0.01, 0.2, 0.7, 0.3);
//          ^^^^    ^^^^^    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//          osc  :  filter :       envelope
```

### 5.3 Core Primitives

**Oscillators:** sine, saw (band-limited via PolyBLEP), square (via PolyBLEP), triangle, white noise, pink noise.

**Filters:** one-pole low/high pass, biquad (peaking, notch, shelf), Moog-style ladder filter (4-pole resonant LPF).

**Envelopes:** ADSR with configurable curves, AR, custom breakpoint envelopes.

**Effects:** delay (with feedback), reverb (Freeverb algorithm as starting point), chorus, distortion/saturation, compressor.

**Utilities:** gain, pan, mix, split, DC blocker, soft clip.

### 5.4 Audio Thread Architecture

The audio callback thread has hard constraints: no allocation, no locking, no blocking. The architecture is:

```
┌─────────────────┐     lock-free      ┌──────────────────┐
│   REPL / UI     │   ring buffer /    │   Audio Thread    │
│   Thread        │ ──triple buffer──> │   (cpal callback) │
│                 │                    │                   │
│  Pattern eval   │   commands:        │  DSP graph eval   │
│  Type checking  │   - swap pattern   │  Buffer filling   │
│  User input     │   - modify param   │  Output to device │
└─────────────────┘   - add/remove     └──────────────────┘
                       node
```

The REPL thread evaluates patterns and produces a schedule of upcoming events. These are sent to the audio thread via a lock-free ring buffer (consider `rtrb` or `ringbuf` crates, or hand-roll for learning). The audio thread consumes events and drives the DSP graph to fill output buffers.

When the user modifies a pattern in the REPL, the new pattern is sent as a replacement command. The audio thread crossfades to the new pattern at the next cycle boundary to avoid clicks. This is the core "hot-reload" mechanism for live coding.

---

## 6. User Interface

### 6.1 ratatui TUI Layout

```
┌─ Orpheus ─────────────────────────────────────────────────┐
│ ┌─ Patterns ──────────────────┐ ┌─ Scope ───────────────┐ │
│ │ kick  = bd ~ bd ~           │ │ ▁▂▃▅▇▅▃▂▁▂▃▅▇▅▃▂▁   │ │
│ │ snare = ~ sn ~ sn           │ │                       │ │
│ │ hats  = hh hh hh hh         │ │ ┌─ Spectrum ────────┐ │ │
│ │ drums = stack(kick, snare,  │ │ │ ▇▅▃▂▁▁▁▁▁▁▁▁▁▁▁  │ │ │
│ │   hats) |> gain(0.8)        │ │ └───────────────────┘ │ │
│ │                              │ └───────────────────────┘ │
│ │ bass = saw(freq)            │ ┌─ Transport ───────────┐ │
│ │   |> lpf(cutoff, res)       │ │ ▶ Playing  BPM: 120   │ │
│ │   |> env(adsr(...))         │ │ Cycle: 47  1/4        │ │
│ │                              │ └───────────────────────┘ │
│ └──────────────────────────────┘                           │
│ ┌─ REPL ──────────────────────────────────────────────────┐│
│ │ > drums |> every(4, fast(2))                            ││
│ │ [Pattern<Sample>] ok                                    ││
│ │ > bass |> lpf(400 800 400 1200)                         ││
│ │ [Pattern<Note>] ok                                      ││
│ │ >                                                       ││
│ └─────────────────────────────────────────────────────────┘│
└───────────────────────────────────────────────────────────┘
```

**Left pane:** Pattern editor. Displays current bindings. Functions as a minimal code editor.

**Top-right:** Live visualization. Oscilloscope (waveform) and spectrum analyzer, rendered as braille/block characters.

**Bottom-right:** Transport controls and status. BPM, current cycle, time signature, play/pause state.

**Bottom pane:** REPL. Evaluate expressions, see type feedback, modify the running composition in real time.

### 6.2 Tracker-Style Visualization (Future)

While the language is cycle-based, the TUI can optionally render active patterns as a tracker-style grid — rows are time steps, columns are layers. This provides a visual cross-check: write in cycles, see it as a grid. The grid is read-only and derived from querying the active patterns.

---

## 7. File Format

### 7.1 `.ode` Files

The durable artifact format. Strict type inference. Importable.

```orpheus
-- song.ode

-- Imports
use "drums.ode" (kick, snare, hats)
use "synths.ode" (bass, pad)

-- Definitions
verse = stack(
  kick |> gain(0.9),
  snare,
  hats |> gain(0.6),
  bass |> lpf(600)
)

chorus = stack(
  kick |> fast(2),
  snare |> every(2, fast(2)),
  hats,
  bass |> lpf(1200),
  pad |> slow(2)
)

-- Song structure
song = seq_sections(
  section(verse, 16),
  section(chorus, 16),
  section(verse, 16),
  section(chorus, 32)
)
```

### 7.2 Project Structure

```
my_song/
  song.ode          -- main composition
  drums.ode         -- drum patterns
  synths.ode        -- synth definitions
  samples/          -- audio sample overrides for the built-in drum tokens
    kick.wav
    snare.wav
    clap.wav
    hihat.wav
    ...
```

### 7.3 Live Sample Packs And Recording

The current live workflow keeps filesystem access off the audio thread:

```text
:samples ./samples
:reload-samples
:render song out.wav 16
:render song out.flac 16
```

- `:samples <directory>` scans WAV overrides and stages them for the next cycle boundary.
- `:reload-samples` rescans the previously configured directory and hot-swaps the bank at the next cycle boundary.
- Supported filename aliases map onto the built-in drum tokens:
  - `bd` or `kick`
  - `sn` or `snare`
  - `cp` or `clap`
  - `hh`, `hat`, or `hihat`
- `:render` now dispatches on the file extension and supports both WAV and FLAC output.

---

## 8. Implementation Plan

### Phase 1: Foundation (Weeks 1-4)

**Goal:** Parse Orpheus, evaluate patterns, hear sound.

- [ ] pest grammar: bindings, juxtaposition, grouping, rest, function application, pipe operator, `stack`
- [ ] AST definition
- [ ] Pattern engine: `Pattern<T>` trait, `CyclePattern`, `query()` implementation
- [ ] Trivial DSP: sine oscillator, sample playback (load wav, trigger on event)
- [ ] cpal audio output: callback thread with ring buffer from pattern evaluator
- [ ] Minimal REPL: read a line, parse, evaluate, hear sound

**Milestone:** Type `bd sn cp sn` in the REPL and hear a four-on-the-floor kick pattern.

### Phase 2: Language (Weeks 5-8)

**Goal:** The language feels usable for real patterns.

- [ ] Type inference: Hindley-Milner with dual-mode (loose/strict)
- [ ] Core transformations: `fast`, `slow`, `rev`, `every`, `sometimes`, `shift`, `degrade`
- [ ] Control signal patterns: `gain`, `pan`, `lpf`, `hpf` accepting `Pattern<Number>`
- [ ] Note literals: `C4`, `Eb3`, `F#5` parsed as `Pattern<Note>`
- [ ] `.ode` file loading with `use` imports
- [ ] Error reporting with source locations

**Milestone:** Write a multi-track drum pattern with transformations in a `.ode` file and hear it.

### Phase 3: DSP (Weeks 9-14)

**Goal:** Build the synthesis engine from scratch.

- [ ] Oscillators: saw (PolyBLEP), square, triangle, sine, noise
- [ ] Faust-style block diagram combinators (sequential, parallel, split, merge, recursive)
- [ ] Filters: one-pole, biquad, ladder
- [ ] Envelopes: ADSR
- [ ] Effects: delay, reverb, chorus
- [ ] Synth definitions wirable from the language layer

**Milestone:** Define a subtractive synth in Orpheus and play melodic patterns through it.

### Phase 4: TUI (Weeks 15-18)

**Goal:** ratatui interface that functions as a session view.

- [ ] Layout: pattern editor, REPL, transport, visualization panes
- [ ] Live oscilloscope and spectrum display (braille characters)
- [ ] Pattern hot-reload: edit in the REPL, hear changes at next cycle boundary
- [ ] Transport controls: play, pause, BPM, tap tempo
- [ ] REPL history, autocomplete, type feedback

**Milestone:** Full TUI session where you can write, modify, and perform a song live.

### Phase 5: Polish & Escape Hatch (Weeks 19-22)

**Goal:** Complete the language and the non-cyclic time model.

- [ ] Event stream patterns: `stream(...)`, `at(...)` syntax
- [ ] Meter annotation: `meter(3, 4)`, beat-relative time literals
- [ ] `seq_sections` for song-level structure
- [ ] Sample management: directory scanning, hot-reload of samples
- [ ] Recording: render output to wav/flac
- [ ] Documentation and examples

**Milestone:** Compose and export a complete multi-section song.

---

## 9. Technical Decisions

### 9.1 Parser: pest

pest provides a PEG grammar via a `.pest` file. Declarative, easy to iterate on, already proven in GLOSSA. The grammar maps cleanly to the juxtaposition-with-disambiguation design.

### 9.2 Audio I/O: cpal

cpal is the de facto Rust audio I/O crate. Cross-platform (ALSA, PulseAudio, WASAPI, CoreAudio). Provides a callback-based API that drives the audio thread.

### 9.3 Rational Time: custom or `num-rational`

Pattern timing must use exact rationals, not floats. `1/3 + 1/3 + 1/3 == 1` must hold exactly, or events drift. `num-rational` provides `Ratio<i64>` which works, but a lightweight custom `Rational` type may be preferred for control over representation and performance.

### 9.4 Lock-Free Communication

The REPL-to-audio-thread channel must be lock-free. Options:

- `rtrb`: Lock-free SPSC ring buffer. Simple, proven.
- Triple buffering: Writer updates a back buffer, atomically swaps a pointer. Reader always sees the latest complete state.
- Custom: for learning purposes, implementing a lock-free queue is valuable.

### 9.5 Sample Format

WAV files loaded via `hound` or `symphonia`. Stored as `Vec<f32>` in an `Arc` shared between the REPL thread (which loads them) and the audio thread (which reads them). The `Arc` avoids copying; the audio thread never deallocates (deallocation happens on the REPL thread when the last reference drops).

---

## 10. Open Questions

1. **MIDI output?** Should Orpheus support driving external hardware/software via MIDI, or is audio-only sufficient for v1?

2. **Live coding performance model.** Is the REPL sufficient, or should there be a "performance mode" with keybindings for muting/soloing layers, switching sections, etc.?

3. **Effect routing.** How are effects applied — per-pattern, per-layer, or via a global effects bus? This affects both the DSP architecture and the language syntax.

4. **Persistence of REPL state.** Should the REPL auto-save its history/state so you can resume a session? Or is the `.ode` file the only persistence mechanism?

5. **Collaboration / sharing.** Beyond git (since songs are code), is there value in a package manager for sharing samples, synth definitions, or pattern libraries?

6. **Integration with existing tools.** OSC support for talking to SuperCollider, Ableton Link for tempo sync with other apps/musicians?

7. **Visual feedback for patterns.** Beyond the oscilloscope, should the TUI render a pianoroll-style or tracker-style view of the currently playing patterns? Read-only, derived from the live query.

---

## 11. Influences and Prior Art

| Project | What Orpheus Takes | What Orpheus Does Differently |
|---------|-------------------|-------------------------------|
| **TidalCycles** | Cycle-based time model, pattern transformations, mini-notation concepts | First-class grammar instead of string DSL; Rust instead of Haskell; dual-mode types |
| **Strudel** | Browser-based accessibility, JavaScript port of Tidal concepts | Terminal-native; compiled artifacts; hand-built DSP |
| **Faust** | Block diagram algebra for signal processing | DSP is a layer, not the language; pattern composition on top |
| **Sonic Pi** | Live coding in a beginner-friendly environment | Expression-based patterns instead of imperative sleep/play; not Ruby |
| **ChucK** | Explicit time advancement, strongly-timed audio | Declarative patterns instead of imperative time management |
| **SuperCollider** | Powerful synthesis server, SynthDef model | Unified language instead of client/server split |
| **fundsp** | Faust algebra in Rust, operator overloading | Study material; Orpheus builds its own DSP for learning |
| **Renoise/trackers** | Grid-based visualization | Visual inspiration for TUI; not the primary composition model |
