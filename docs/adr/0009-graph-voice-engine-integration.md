# ADR 0009: Graph Voice Engine Integration

- Status: Accepted
- Date: 2026-07-08

## Context

ADR 0004 shipped the Faust-style graph combinator system but deferred engine
integration: nothing in `engine.rs`/`voice.rs` referenced the `graph/` module,
so a complete stereo voice was expressible in graph nodes yet unplayable from
patterns. Wiring graphs into the engine forces decisions ADR 0004 only
sketched: how graph voices are constructed and owned relative to the audio
thread, how pattern events select and gate them, and how the "construction may
allocate, rendering must not" boundary survives per-event voice lifecycles.

Two constraints shape the design:

- The audio-thread path must stay allocation-free and lock-free. The existing
  per-trigger `ActiveVoice` lifecycle constructs (and drops) voices on the
  audio thread; graph processors own nested scratch buffers, making that
  pattern unacceptable for them.
- The combinator `process()` implementations allocated small `Vec`s of slice
  references on every call, so even a pre-built `Processor` was not
  allocation-free in steady state, contradicting ADR 0004's stated goal.

## Decision

Adopt a pooled, prepared-voice model with a static built-in program table
(`crates/orpheus-dsp/src/graph_voice.rs`).

**Fixed voice interface.** A `GraphVoiceProgram` compiles to a `Processor`
with 4 inputs (gate, freq_hz, gain, pan) and 2 outputs (left, right).
Parameters flow as signal inputs per ADR 0004; the gate is a rectangular
signal derived from the pattern event's span, so graph envelopes (`adsr`/`ar`)
own the note shape.

**Pool-per-program, built off-thread.** `GraphVoiceBank` is constructed in
`EngineCore::new` — before the audio thread exists — building a fixed
polyphony pool (8 voices) per program. Each voice is warmed with
`GraphVoice::prepare()`, which runs one block through the processor so
lazy-but-once scratch growth happens off-thread, then resets state.
Triggering claims an idle pooled voice (a field write, no allocation);
finished voices are reset in place and returned to the pool rather than
dropped, so no deallocation happens on the audio thread either.

**Token selection with sample-bank precedence.** Graph programs are selected
by the same token mechanism as samples and built-in synth fallbacks. The
sample bank and `VoiceKind` fallbacks keep first claim on a token; only tokens
they do not resolve reach the graph bank. The first built-in program is
`gsine` (sine carrier × gate-driven ADSR → trigger gain → equal-power pan),
honouring the trigger's `rate` (pitch, scaled by the engine reference
frequency), `gain`, and `pan` fields.

**Deterministic voice end.** A note's lifetime is `gate_frames` (the event
span) plus the program's fixed release tail in frames. No silence detection
runs on the audio thread.

**Allocation-free combinator processing.** The per-call slice-reference
tables in `Seq`/`Spl`/`Mrg`/`Rec`/`Bind` are now built in fixed-size stack
arrays (up to 32 channels, falling back to heap `Vec`s only for wider
graphs), making warmed `Processor::process` calls genuinely allocation-free.
A thread-local counting-allocator test enforces this.

## Consequences

- Pattern events can play graph-defined voices today, engine-API-only;
  language-level syntax for defining programs remains out of scope.
- Polyphony is bounded and explicit: a program's 9th simultaneous note is
  dropped. Voice stealing can be added later inside `GraphVoiceBank` without
  engine changes.
- The static program table is the extension point for a future
  `EngineCommand::ReplaceGraphVoicePrograms` (mirroring `ReplaceSampleBank`)
  once user-defined graphs exist; the pool build/prepare lifecycle already
  matches that command's swap-at-boundary shape.
- Graph voices bypass the per-trigger insert-effect chain (delay/reverb/
  chorus/compressor and pedals). Effects belong in the graph itself or on
  bus sends; revisit if parity with sample voices is needed.
- Existing voices are untouched: sample and analog rendering is byte-for-byte
  identical to the pre-integration engine.
