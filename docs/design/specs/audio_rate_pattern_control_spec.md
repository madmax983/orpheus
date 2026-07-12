# 🔭 Vantage: Spec for Audio-Rate Pattern Control

## 👤 User Story
"As a Sound Designer / Live Coder, I want my pattern commands to smoothly modulate synthesizer parameters at audio-rates (like LFOs sweeping a filter cutoff continuously), so that I can create evolving, organic, and complex textures rather than just discrete stepped changes per note."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus patterns can control parameters (`p1`..`p4`) but they do so discretely. They send breakpoints at trigger times or sub-note segments, resulting in a piecewise-linear, control-rate interpolation. While powerful for rhythmic structures, this limits the sonic palette. If a user wants a smooth, continuous sine-wave modulation of a filter cutoff exactly synced to the cycle, they can't do it purely from the pattern language without generating hundreds of sub-note events, which is inefficient and clunky.

By adding continuous audio-rate evaluation/mapping of patterns to DSP inputs, we close the gap between Orpheus and visual modular synthesizers (like VCV Rack) or advanced DSP environments. It turns the pattern language from a mere "sequencer" into a true "modulator."

## 🎯 Success Metrics (Metric Definition)
- **Modulation Quality:** Audio-rate pattern modulation must not exhibit zipper noise or stair-stepping artifacts; it must resolve at the sample rate.
- **Performance:** Adding a single audio-rate modulated parameter to a voice should increase overall CPU load by no more than 1-2% compared to control-rate breakpoint interpolation.
- **Usability:** Users can express an audio-rate LFO in a single line of pattern code without explicit "audio rate" casting.

## ✅ Acceptance Criteria
- **Continuous Evaluation:** The engine must be able to evaluate specific pattern functions (e.g., `sine`, `saw`) continuously over time at the audio block rate, producing a stream of `f32` values per sample.
- **Routing:** These audio-rate streams must seamlessly map to the `p1`..`p4` inputs of a `voice { }` DSP graph, replacing or overriding the piecewise-linear breakpoint interpolation when an audio-rate signal is present.
- **Cycle Sync:** Audio-rate patterns must remain phase-locked to the rational cycle clock. A `sine` over 1 cycle should exactly complete one phase rotation over the cycle's duration.
- **Graceful Degradation:** If a pattern cannot be evaluated at audio rate (e.g., it produces strings or complex discrete structures), it should fall back to the existing breakpoint interpolation or emit a clear error, rather than crashing the audio thread.

## 🚫 Out of Scope
- **Audio-Rate Pattern *Feedback*:** Feeding audio output back into the pattern evaluation logic to change pattern structure dynamically. Pattern evaluation remains strictly a source of modulation, not a sink.
- **Full Pattern Language at Audio Rate:** We will not attempt to evaluate arbitrary, complex discrete transformations (like `euclid` or `jux`) at audio rate. Only a subset of continuous functions (LFOs, envelopes, smooth randoms) will be designated as audio-rate capable.
- **Visual Mapping UI:** Building a visual interface to show what pattern is modulating what parameter. This is purely an engine and language integration feature for now.

## 📊 Gap Analysis
- **Standard Libs / Competitors:**
  - **TidalCycles:** Primarily discrete, step-based modulation. Continuous modulation relies on sending high-density OSC messages, which can jitter.
  - **SuperCollider / Faust:** Excellent audio-rate modulation, but lacks the high-level pattern language integration for easy sequencing.
  - **Max/MSP / Pure Data:** Full control, but node-based, not text-based live coding.
- **Our Gap:** We have the DSP graph (Faust-style) and the pattern language (Tidal-style), but the bridge between them for *continuous* signals is limited to control-rate breakpoints. This spec bridges that final gap.
