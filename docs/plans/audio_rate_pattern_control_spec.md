# 🔭 Vantage: Spec for Audio-Rate Pattern Control

## 👤 User Story
"As an Electronic Musician, I want to modulate synth parameters using audio-rate patterns (like LFOs), so that I can create evolving textures and precise timbral changes within a single note."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus supports breakpoint-rate pattern control for per-note parameters, interpolating linearly per-frame. While sufficient for many use cases, true FM synthesis, complex AM, or high-frequency filter sweeps require modulation at the audio sample rate. Without this, Orpheus is restricted to slower, macroscopic structural changes. Adding audio-rate pattern control bridges the gap between structural sequencing and deep sound design, allowing patterns to act directly as continuous audio signals. This increases utility for sound designers.

## 🎯 Metric Definition
- **Success** = Users can define a pattern that generates an audio-rate signal (e.g., a fast sine wave pattern) and use it to modulate a voice parameter. The system must render this modulation per-sample, accurately sweeping the parameter at the audio rate without introducing buffer-rate artifacts. CPU usage per active voice must increase by less than 5% compared to breakpoint-rate control.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Supports breakpoint-rate parameter automation interpolated per frame. Audio-rate sweeps require manual Faust-level DSL coding inside the voice graph.
- **Competitors (TidalCycles, SuperCollider):** SuperCollider excels at this, allowing arbitrary audio-rate modulation mapping. TidalCycles relies on SuperDirt, which generally maps continuous patterns to control rate, though audio-rate mappings are possible via specific SynthDefs.
- **The Gap:** Orpheus needs a unified way to express true audio-rate control signals directly from the pattern language, lowering them into the DSP engine to drive parameters at the sample rate rather than the control rate.

## ✅ Acceptance Criteria
- Must introduce a mechanism (e.g., a special `ar()` pattern function or modifier) to explicitly define an audio-rate control pattern.
- The language evaluator and engine must lower this audio-rate pattern into a continuous audio-rate stream (e.g., an LFO) connected to the specified voice parameter.
- The DSP engine must update the targeted parameter every sample when driven by an audio-rate control.
- Must gracefully degrade or warn if an overly complex pattern is attempted at audio-rate to protect real-time performance.

## 🚫 Out of Scope
- Evaluating full Tidal-style discrete compositional patterns (e.g., complex euclidean rhythms or nested alternating sequences) strictly at the audio sample rate. This is explicitly out of scope; audio-rate control is for continuous signals (LFOs/envelopes).