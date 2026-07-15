## 🔭 Vantage: Spec for Audio-Rate Pattern Control of Voice Parameters

👤 **User Story:** As a sound designer and live coder, I want to use fast pattern structures to modulate instrument parameters at audio rates (e.g., using an LFO pattern to drive a filter cutoff, or a high-speed noise pattern to modulate oscillator pitch), so that I can create complex, evolving timbres and classic synthesis techniques like FM or AM directly from the language without needing custom DSP graphs for every patch.

### ❓ So What? (The Business Problem)
Currently, voice parameter automation is limited to control-rate breakpoint envelopes (up to 32 breakpoints per note). This prevents users from achieving true audio-rate modulation (FM, AM, audio-rate filter sweeps) using standard patterns. To get these sounds, users must drop down into the `graph { }` DSL to define custom, hardcoded oscillator-driven modulators. Enabling audio-rate pattern control bridges the gap between high-level sequencing and low-level DSP, significantly expanding the sonic palette available from the REPL and bringing Orpheus closer to true TidalCycles/Faust parity.

### ✅ Acceptance Criteria:
- Must allow a pattern to drive a voice parameter (`p1`..`p4`) at audio rates (e.g., updating per-sample or per-audio-block).
- Must seamlessly integrate with the existing `voice { }` DSL and graph combinators.
- Must maintain the allocation-free guarantee on the audio thread.
- Must fall back gracefully or cap resolution without panicking if a pattern is impossibly dense.
- Must not break existing breakpoint automation (ADR 0012) for control-rate patterns; audio-rate control should be an opt-in or transparently scaled feature.

### 🚫 Out of Scope:
- Modulating non-voice parameters (e.g., master volume, track send levels) at audio rates.
- Literal pattern evaluation (querying the `PatternRuntime` AST) on the audio thread. (The solution must involve pre-computing buffers or compiling to DSP nodes, not running the language evaluator in the real-time loop).
