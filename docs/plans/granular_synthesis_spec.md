# 🔭 Vantage: Spec for Granular Synthesis Engine

## 👤 User Story
"As a Sound Designer and Live Coder, I want a built-in granular synthesis engine, so that I can mangle, stretch, and transform audio samples into entirely new, evolving textures and atmospheres without needing external VST plugins or relying solely on traditional time-stretching."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus can trigger and slice standard audio samples. However, conventional sample playback is rigid. When a user wants to drastically slow down a vocal chop without lowering its pitch, or turn a brief snare drum hit into a continuous, ambient drone, they hit a wall. To achieve these modern, experimental sound design techniques, they are forced to process audio in a traditional DAW and bounce it back into Orpheus. Complexity is a cost; utility is revenue. Granular synthesis is a foundational tool in modern electronic music, IDM, and ambient genres. By embedding a granular engine, we unlock infinite textural possibilities from a small set of source samples, dramatically expanding the platform's sonic capabilities and keeping the user entirely within the Orpheus ecosystem.

## 🎯 Metric Definition
- **Success** = Users can apply a granular synthesis function (e.g., `granular("sample", size, density, pitch)`) to an audio sample. The engine must generate up to 64 overlapping grains per voice, with <5% CPU overhead per voice, and without causing any audio dropouts or locking on the real-time audio thread, even when parameters are rapidly modulated per cycle.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation is limited to virtual analog oscillators and standard WAV sample playback (which couples pitch and speed). There is no time-stretching, pitch-shifting, or granular manipulation.
- **Competitors (Ableton Live, SuperCollider, TidalCycles):** SuperCollider has extremely powerful granular UGens (like `TGrains`). Ableton Live's "Texture" warping mode and Granulator MaxForLive device are industry standards. TidalCycles supports basic granular playback via SuperDirt (`chop`, `striate`).
- **The Gap:** Orpheus needs a dedicated DSP primitive that decouples time and pitch by splitting an audio buffer into tiny, envelope-smoothed segments (grains) that can be individually manipulated and reassembled in real-time.

## ✅ Acceptance Criteria
- Must introduce a new `granular("sample_name")` function or modifier in the pattern language.
- Must support sequencing fundamental granular parameters via patterns:
  - `grain_size` (duration of each grain, e.g., 10ms to 200ms)
  - `density` (how many grains are triggered per second)
  - `position` (where in the source sample the grains are read from)
  - `pitch` (independent pitch shift for the grains)
  - `spread` (randomization/jitter applied to position or pitch)
- Must implement the granular processing entirely within the `orpheus-dsp` engine, running on the real-time audio thread.
- Must utilize a pre-allocated pool of grains or a lock-free structure to ensure zero memory allocation occurs during the real-time audio rendering block.
- Must apply smooth windowing (e.g., Hanning or Gaussian envelope) to each grain to prevent audio clicks and popping.

## 🚫 Out of Scope
- Granular synthesis of live audio input streams. Phase 1 focuses strictly on granularizing pre-loaded, static sample buffers.
- Advanced stereo grain spatialization (e.g., complex 3D panning per grain). Phase 1 will support basic random stereo panning spread.
- Visual grain representation in the TUI (e.g., drawing a waveform and showing grain positions).
