# 🔭 Vantage: Spec for Audio-Rate Pattern Control

## 👤 User Story
"As a Sound Designer and Live Coder, I want to modulate synthesizer voice parameters using audio-rate control signals directly from my patterns, so that I can create complex, evolving timbres and precise rhythmic modulations that go beyond parameters being recomputed per block without leaving the pattern language."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus supports parameters that are recomputed per block for voice parameters. While sufficient for macro-level automation, it falls short for audio-rate modulation like FM synthesis, AM, or rapid filter FM driven by the pattern engine itself. If users want true audio-rate modulation, they are forced to hardcode it into the voice DSP graph rather than sequencing it dynamically. Complexity is a cost; utility is revenue. Bridging the gap between the pattern scheduler and the audio-rate DSP graph unlocks a massive new realm of sound design, dramatically increasing Orpheus's utility for advanced electronic music production.

## 🎯 Metric Definition
- **Success** = Users can sequence a parameter at audio rates, and the engine evaluates and applies this modulation on a per-sample basis within the DSP graph, maintaining real-time execution without introducing audio glitches, xruns, or violating the cycle-based time model.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Parameters are evaluated at trigger time or recomputed per block. They do not evaluate per-sample.
- **The Gap:** Orpheus lacks a bridge to evaluate pattern-generated signals at audio rate within the synth voice's process loop, restricting pattern modulation to control rates.

## ✅ Acceptance Criteria
- Must introduce a syntax or method to denote a pattern control stream as audio-rate.
- Must pipe audio-rate pattern data into the voice parameters as continuous audio buffers.
- Must ensure that the audio-rate evaluation occurs safely on the real-time thread, without memory allocations or locking.
- Must fallback gracefully and remain performant when standard control-rate modulation is used.

## 🚫 Out of Scope
- Audio-rate evaluation of all pattern transforms (e.g., fast or slow on audio signals) is not targeted in Phase 1.
- Exposing the entire pattern language to the audio thread; this is strictly about parameter modulation streams.
