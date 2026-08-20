# 🔭 Vantage: Spec for Audio-Rate Pattern Control of Voice Parameters

## 👤 User Story
"As a Sound Designer, I want to map rapidly changing pattern data (like continuous LFOs or complex sub-step generative modulations) directly to synthesizer parameters at audio rates, so that I can create evolving, alias-free, and hyper-detailed modulations that exceed the current 32-breakpoint limitation of the note trigger envelope."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has robust pattern-level sequencing for notes and discrete parameters, and it supports smooth breakpoint-rate parameter control (up to 32 breakpoints sent per note for intra-note interpolation). However, the engine integration of `graph/` is marked as `[~] partial` in the roadmap specifically because it lacks true audio-rate pattern control of voice parameters. If a user tries to drive a filter cutoff with a 50Hz sine wave generated at the language level, the 32-breakpoint approximation introduces aliasing and zipper noise. Without audio-rate modulation, Orpheus's DSP graph cannot rival true modular environments (like Max/MSP or Faust) for complex sound design. Complexity is a cost; utility is revenue. Bridging the gap between the exact-rational pattern time and the DSP sample time unlocks professional-grade synthesis capabilities.

## 🎯 Metric Definition
- **Success** = Users can define continuous/audio-rate modulations in the pattern language (e.g., `cutoff(sine * 1000 + 500)`) and the DSP graph interpolates or renders these at the block rate or sample rate without exceeding a 15% increase in baseline CPU overhead per voice, and strictly without allocating memory on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** `p1`..`p4` are evaluated and baked down into a maximum of 32 linear breakpoints per note at trigger time. This is "control rate" or lower.
- **Competitors (Faust, SuperCollider):** Faust natively compiles everything to sample-rate C++ or uses block-rate optimizations transparently. SuperCollider explicitly separates `.kr` (control rate) and `.ar` (audio rate) signals.
- **The Gap:** The roadmap explicitly calls out: `remaining: audio-rate pattern control of voice parameters`. We need a mechanism to efficiently stream or lazily evaluate high-density parameter data from the pattern engine to the DSP voice graph.

## ✅ Acceptance Criteria
- Must introduce a mechanism for `p1`..`p4` (or dedicated new audio-rate parameters) to support evaluation resolutions higher than the current 32 breakpoints.
- Must ensure that evaluating these high-resolution patterns does not cause dropouts or GC pauses on the real-time audio thread.
- Must provide smooth, alias-free modulation for common voice parameters (cutoff, delay time, FM index) when driven by continuous functions (sine, saw, noise) at up to 100Hz.

## 🚫 Out of Scope
- Evaluating the full `pest` AST dynamically on the audio thread. Phase 1 must likely rely on pre-baking high-density buffers, streaming from a non-RT thread, or utilizing an optimized, lock-free mini-evaluator for primitive math functions.
- Feedback loops between the audio rate output and the pattern rate input.
