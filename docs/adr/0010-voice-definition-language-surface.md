# ADR 0010: Language Surface for User-Defined Graph Voice Programs

- Status: Accepted
- Date: 2026-07-08
- Extends: ADR 0009 (graph voice engine integration), ADR 0004 (graph
  combinators)

## Context

ADR 0009 wired graph-compiled voices into the render engine behind a static
built-in program table (`gsine`) and named two follow-ups: a user-facing way
to define programs from the language, and an
`EngineCommand::ReplaceGraphVoicePrograms` swap mirroring `ReplaceSampleBank`.
Until now a user could not define an instrument: the sonic surface of graph
voices was exactly one hardcoded program.

Two surface shapes were considered:

- a `synth(...)`-style builtin taking positional/named arguments — zero
  grammar changes, but named arguments do not survive general evaluation
  (assignment expressions are pedal-DSL-only), and a flat argument list cannot
  express signal routing (an envelope sweeping a filter cutoff);
- a block form following the pedal DSL precedent (`graph { ... }`), which
  already established how Orpheus describes DSP in-language: let-bound
  signals, stage calls, `|>` pipes, and `+`/`*` arithmetic.

## Decision

**`name = voice { ... }` block, sharing the pedal DSL's body grammar.** The
`voice` block reuses the existing `graph_body` grammar rule (bindings then one
result expression) and the pedal DSL's tight-`*`-is-multiplication rewrite, so
the parser change is one rule plus one AST variant. The binding name is the
program's pattern token.

**Voice-specific compilation, not pedal compilation.** A voice body sees two
ambient inputs — `gate` (1 during the event span) and `freq` (note Hz) — and
composes the graph vocabulary that `orpheus-dsp` exports: `sine`/`saw`/`tri`/
`pulse`/`noise` oscillators, gate-driven `adsr`/`ar` envelopes (segment times
as literals, bound as constants per ADR 0004's `bind`), the ladder `lowpass`,
`drive` saturation, and `+`/`*` mixing. The result is the mono voice signal;
the per-trigger gain and equal-power pan stages are appended automatically to
meet ADR 0009's fixed `[gate, freq, gain, pan] -> [left, right]` interface.
The program's release tail is the longest envelope release in the body (small
default when none), preserving ADR 0009's deterministic voice end.

**Declarative spec as the crate boundary.** Compilation produces a
`GraphVoiceSpec` (`crates/orpheus-dsp/src/graph_voice.rs`): a flat, validated
DAG of data-only `VoiceNodeSpec`s. Specs are `Clone`/`PartialEq`, so they can
ride inside `EngineCommand` and give banks structural equality. Lowering onto
combinators is generic: each node becomes a `wire`-then-`par` stage that
prepends its output to a growing signal bus, reusing `seq`/`par`/`wire`/
`passthrough` rather than adding a new evaluator. Validation at `new()` means
compilation cannot fail later.

**Swap discipline mirrors the sample bank.** The session rebuilds a complete
`GraphVoiceBank` (built-ins plus one program per `voice` binding, user
programs shadowing same-token built-ins) on the language thread —
construction compiles and pre-warms every pooled voice — and enqueues
`EngineCommand::ReplaceGraphVoicePrograms`. The audio thread stores it as
pending and moves it into place at the next cycle boundary, exactly like
`ReplaceSampleBank`; redefining a voice mid-session is therefore safe and
takes effect on the next cycle. `Clone` on a bank rebuilds from the retained
specs (documented as off-thread-only); `PartialEq` compares specs and sample
rate.

**Token resolution keeps sample-bank precedence.** A voice binding's name
resolves inside pattern sequences as a sample-like token
(`Pattern<Sample>`-coercing in the type system); at trigger time the sample
bank and built-in fallbacks keep first claim, and only unresolved tokens reach
the graph bank (ADR 0009 unchanged).

## Consequences

- Users define instruments in session files, e.g.
  `pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }`
  and play them with `melody = pluck ~ pluck ~`.
- The audio-thread path stays allocation-free: the counting-allocator test
  covers a spec-compiled user voice alongside the built-in.
- Notes sounding on the outgoing pool end at the swap boundary when a bank is
  replaced; the old bank is dropped on the audio thread, the same trade-off
  the sample-bank swap already accepts.
- The voice vocabulary is a curated subset of the graph module: `split`/
  `merge`/`feedback` topologies, audio-rate envelope-segment modulation, and
  per-program polyphony configuration are deliberate follow-ups.
- A bare voice binding referenced outside a pattern is the voice value itself
  (aliasing), not a pattern; tokens only take effect inside sequences.
