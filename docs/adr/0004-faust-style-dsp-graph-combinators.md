# ADR 0004: Faust-Style DSP Graph Combinators

- Status: Accepted
- Date: 2026-03-25

## Context

Orpheus has a set of hand-built scalar DSP primitives (oscillators, filters,
saturation, gain) and a hardcoded voice pipeline (`AnalogVoice`: osc -> filter
-> sat -> gain). The design document (section 5.2) calls for Faust's
five-operator block diagram algebra as the internal DSP wiring system. Without
a composition layer, users cannot build novel synthesizers or complex effect
chains — the sonic palette is limited to pre-wired topologies.

The block diagram algebra provides a mathematically robust, declarative way to
express audio routing (sequential, parallel, split, merge, and recursive)
without imperative buffer management.

## Decision

Implement a `graph` module in `orpheus-dsp` with five Faust-style combinators,
a `Node` trait, and a `Processor` compilation wrapper.

### Key design choices

**Runtime channel counts (u32), not const generics.**
`fundsp` uses typenum const generics for compile-time channel verification. We
chose runtime u32 counts validated at graph construction time. This avoids
typenum's ergonomic pain (complex type signatures, difficult dynamic
composition) while providing the same safety guarantee at the point that
matters: graph assembly, not compilation. Errors are returned as `GraphError`
variants with clear context strings.

**Parameters as signal inputs (Faust model).**
A filter's cutoff frequency is an input channel, not a struct field. `saw()`
has 1 input (freq_hz) and 1 output (audio). `ladder_filter()` has 3 inputs
(audio, cutoff_hz, resonance) and 1 output. The combinator algebra handles
parameter routing through `par` and `split` — no separate parameter system
needed. Trade-off: wiring is more verbose, but maximally composable.

**Block-based `process()` with lazy scratch allocation.**
The `Node` trait operates on slices of `f32` (block processing), not individual
samples. Combinator nodes (Seq, Spl, Mrg) own scratch buffers that grow once on
first `process()` call and never reallocate thereafter. This gives zero-allocation
steady-state rendering while keeping construction lightweight.

**No separate `tick()` method.**
The Recursive combinator calls `process(frames=1)` in a loop for its
one-sample-delay feedback semantics. This avoids trait complexity. If profiling
reveals significant overhead, a `tick()` fast path can be added later.

**Free functions, not operator overloading.**
`seq()`, `par()`, `split()`, `merge()`, `feedback()` — explicit, Result-returning
where validation applies. Operator overloading (e.g., `>>` via `Shr`) can be added
as ergonomic sugar in a future pass.

**Processor as compilation boundary.**
`Processor::new(graph)` documents where construction (which may allocate) ends
and rendering (which must not) begins. Phase 1 delegates to the combinator tree;
a future optimization could flatten into a linear execution schedule with buffer
reuse.

## Consequences

- Orpheus can now express arbitrary DSP topologies as composed graphs
- Existing hand-built primitives (SawOsc, LadderFilter, etc.) are wrapped via
  adapters — no reimplementation needed
- The graph module is self-contained within `orpheus-dsp`; engine integration
  (wiring graph voices into `RenderEngine`) is a follow-up task
- Language-level syntax for graph construction is out of scope (the design doc
  states these operators are "internal to the DSP layer")
- The FM synth example in integration tests demonstrates the spec's acceptance
  criterion: a 2-oscillator FM patch with filter, proving the system works
