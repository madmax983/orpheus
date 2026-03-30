# 🔭 Vantage: Spec for Richer Effects Chain

## 👤 User Story
"As a Live Coder, I want a richer suite of built-in audio effects (delay, chorus, reverb, compression), so that I can shape the spatial and dynamic qualities of my patterns natively within Orpheus without needing external processing tools."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus outputs relatively dry audio signals (basic synthesis or raw sample playback). To achieve a polished, "mix-ready" sound, users must route audio into a DAW for post-processing. This breaks the self-contained promise of live coding, adding setup complexity and fragmenting the creative workflow. By embedding high-quality core effects directly into the pattern language and DSP engine, we allow performers to create massive, evolving soundscapes entirely through code. Complexity is a cost; utility is a revenue. A richer effects chain acts as a utility multiplier, turning dry loops into immersive musical experiences directly from the terminal.

## 🎯 Metric Definition
- **Success** = 99% of pattern cycles applying `delay`, `reverb`, `chorus`, or `compressor` functions execute without causing audio xruns, adding < 2ms of processing overhead to the real-time DSP block render, and parameters can be modulated per-cycle seamlessly.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Basic gain, panning, and simple filtering. No time-based or dynamic effects.
- **Competitors (TidalCycles/SuperDirt, Strudel):** Offer extensive built-in effects (SuperDirt has a massive library; Strudel leverages WebAudio nodes like Convolver and Delay).
- **The Gap:** Orpheus needs a native, performant suite of essential mix effects integrated tightly with the pattern language, allowing parameters (e.g., reverb `room_size`, delay `feedback`) to be sequenced just like notes.

## ✅ Acceptance Criteria
- Must introduce `delay`, `reverb`, `chorus`, and `compressor` modifiers to the Orpheus language.
- Must support sequencing effect parameters via patterns (e.g., `delay(slow(2, seq(0.2, 0.8)))`).
- Must render effects accurately on the real-time audio thread without allocation or locking.
- Must ensure time-based effects (delay, reverb) have smooth parameter interpolation to prevent clicking when modulated rapidly.
- Must implement the DSP logic directly in Rust (or via safe abstractions) without requiring external VSTs or plugins.

## 🚫 Out of Scope
- Advanced spectral effects (e.g., granular synthesis, FFT-based manipulation). Phase 1 focuses on standard mix tools.
- Third-party VST/AU plugin hosting.
- Multi-band compression. Phase 1 is a broadband compressor.
