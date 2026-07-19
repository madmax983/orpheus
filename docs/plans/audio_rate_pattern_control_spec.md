# 🔭 Vantage: Spec for Audio-Rate Pattern Control

## 👤 User Story
"As a Sound Designer and Live Coder, I want to modulate synthesizer parameters at audio rates using patterns, so that I can create complex FM synthesis, audio-rate filter sweeps, and rich textures directly from the code without resorting to external plugins."

## The "So What?"
Currently, Orpheus supports sub-note pattern control for voice parameters, which is sufficient for envelopes and LFOs but cannot do FM synthesis or audio-rate modulation. This limits the timbral palette of the built-in synth engine. If users want complex textures, they must rely on external synths. Adding audio-rate pattern evaluation makes the internal engine a full-fledged modular synthesizer, increasing its value as an all-in-one performance instrument. Complexity is a cost, but unlocking a new category of sound design is a revenue.

## Metric Definition
- **Success** = Users can route audio-rate signals into voice parameters using pattern syntax. The engine must evaluate these modulations at the audio sample rate without introducing audible artifacts or performance dropouts.

## Gap Analysis
- **Current State (Orpheus):** Sub-note pattern modulations are evaluated at a lower resolution rate and interpolated. They cannot be driven at the sample rate.
- **Competitors (SuperCollider, Max/MSP):** These environments treat audio and control signals uniformly, allowing seamless audio-rate modulation anywhere.
- **The Gap:** Orpheus needs a mechanism to evaluate certain parameter streams at the audio block rate.

## ✅ Acceptance Criteria
- Must introduce a way to evaluate pattern controls at audio rate for voice parameters.
- Must ensure that evaluating these patterns is performant enough to not cause audio glitches.
- Must fall back to standard interpolation for patterns that are not explicitly marked or suited for audio-rate.

## 🚫 Out of Scope
- Audio-rate modulation of global mixer parameters (e.g., master volume). Phase 1 focuses on voice-level parameters.
