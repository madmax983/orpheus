# 🔭 Vantage: Spec for Faust-Style DSP Graph Combinators

## 👤 User Story
"As a Sound Designer and Live Coder, I want a declarative, combinatorial syntax to define and route custom DSP algorithms (like oscillators, filters, and delays) so that I can build completely original synthesizers and complex effect chains directly in code without dealing with low-level audio buffer management or leaving the Orpheus ecosystem."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus relies on a fixed set of built-in DSP effects (like `lpf`, `hpf`, `gain`) and samples. While this is great for getting started quickly, it artificially limits the sonic palette for advanced users. To create novel, un-heard textures, users need the ability to wire fundamental DSP blocks together. A modular, Faust-inspired block diagram algebra provides a mathematically robust, declarative way to express audio routing (sequential, parallel, split, merge, and recursive) without the boilerplate of imperative buffer manipulation. Complexity is a cost; utility is revenue. By giving users the raw building blocks and a clean grammar to connect them, we turn Orpheus from a mere sequencer into a fully-fledged sound design laboratory, drastically increasing its utility for professional sound designers.

## 🎯 Metric Definition
- **Success** = The new DSP graph combinators (`>>` for sequential, `,` for parallel, `<:` for split, `:>` for merge, `~` for recursive) are implemented in `crates/orpheus-dsp/src/graph.rs` and can express a complex synth definition (e.g., a 2-oscillator FM patch with a resonant filter and feedback delay). The resulting graph must compile to a lock-free processing node that runs on the audio thread with < 10% CPU overhead per voice, without any allocations during the render block.

## 🔍 Gap Analysis
- **Current State (Orpheus):** DSP operations are applied as isolated transformations (`|>` pipe operator) on complete pattern streams. There is no concept of sub-graph routing or building new composite DSP blocks from primitives.
- **Competitors (Faust, SuperCollider, ChucK):** Faust uses a highly optimized block diagram algebra. SuperCollider uses a robust object-oriented graph builder (`SynthDef`). ChucK uses the `=>` operator for procedural routing.
- **The Gap:** Orpheus needs a way to define DSP architectures at the block level *before* they are instantiated as voices. We need a DSL embedded in Rust (or exposed to the Orpheus language) that leverages Faust's combinatorial operators to build efficient, zero-allocation audio graphs.

## ✅ Acceptance Criteria
- Must introduce core Faust-style routing combinators in `orpheus-dsp`:
  - **Sequential (`>>`):** Connects the output of A to the input of B.
  - **Parallel (`,`):** Places A and B side-by-side, processing independent channels.
  - **Split (`<:`):** Duplicates a single output into multiple inputs.
  - **Merge (`:>`):** Mixes multiple outputs down to a single input.
  - **Recursive (`~`):** Creates a feedback loop with a one-sample delay.
- Must implement basic DSP primitives (Sine, Saw, Noise, Delay, OnePole, Gain) that conform to a common `Node` trait with well-defined input and output channel counts.
- Must statically (or safely dynamically) verify channel counts at graph construction time (e.g., trying to sequence a 2-output node into a 1-input node should return an error).
- Must provide a way to compile the defined graph into an executable `Processor` that operates strictly on pre-allocated buffers on the real-time audio thread.

## 🚫 Out of Scope
- A complete visual node editor UI (like Max/MSP or PureData). Phase 1 is strictly code-based routing.
- Automatic JIT compilation of the graphs via LLVM (like native Faust). Phase 1 will execute the graph via dynamic dispatch or an AST interpreter optimized for the Orpheus audio engine block size.
- Real-time modification of the *topology* of the graph while a voice is playing. Parameter values (like cutoff frequency) can change, but adding/removing nodes requires instantiating a new voice.
