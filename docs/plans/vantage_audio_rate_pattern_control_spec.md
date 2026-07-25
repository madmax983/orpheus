# 🔭 Vantage: Spec for Audio-Rate Pattern Control of Voice Parameters

## 👤 **User Story:**
"As a Live Coder, I want my pattern parameter expressions to modulate voice DSP graphs at full audio rates, so that I can achieve smooth, continuous sound design directly from the REPL instead of relying on linear breakpoint interpolation."

## The So What?
Currently, Orpheus supports intra-note parameter automation by sampling the pattern and shipping up to 32 breakpoints per note to the DSP engine, which interpolates them linearly per frame. While efficient, this acts as a control-rate LFO. True audio-rate modulation allows the graph algebra to be driven continuously by the pattern logic. Closing this gap turns the pattern language into a full synthesizer patching environment. Complexity is a cost; utility is revenue.

## Metric Definition
- **Success** = A user can define a parameter control in a pattern that evaluates as an audio-rate signal inside the DSP graph without dropping audio or causing system instability, maintaining total CPU usage within acceptable bounds for standard polyphony.

## Gap Analysis
- **Current State:** The system interpolates `p1`..`p4` breakpoints linearly every frame (ADR 0012).
- **The Gap:** The engine does not yet support continuous, sample-accurate, audio-rate evaluation of pattern parameters.

## ✅ **Acceptance Criteria:**
- Must allow pattern parameters (e.g., `p1`..`p4`) to drive audio-rate modulation of DSP voice nodes.
- Must ensure that any parameter changes compose cleanly with voice stealing (steals must glide smoothly from the stolen note's current value to the new automation envelope without clicks).
- Must accomplish modulation without fragmenting note triggers.

## 🚫 **Out of Scope:**
- Literal audio-rate pattern evaluation (evaluating the entire pattern language parser/AST per sample is intentionally out of scope as noted in the roadmap; the feature focuses on continuous DSP parameter modulation driven by pattern definitions).
