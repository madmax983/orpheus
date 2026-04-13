# 🔭 Vantage: Spec for Granular Synthesis Engine

## 👤 User Story
"As a Sound Designer and Live Coder, I want a granular synthesis engine in Orpheus, so that I can chop, stretch, and sculpt audio samples into ambient textures, glitchy rhythms, and completely new timbres in real-time using pattern parameters."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has robust built-in synths and basic sample playback. However, standard sample playback limits sound design to the inherent length and pitch of the original audio file. Granular synthesis takes short pieces (grains) of an audio sample and plays them back in a variety of ways (overlapping, stretched, pitch-shifted, reversed) to create entirely new sounds from a source. This is a foundational technique in modern electronic music, ambient, and IDM. If Orpheus lacks a granular engine, users looking for complex textures and time-stretching capabilities will be forced to use external DAWs or plugins. Complexity is a cost; utility is revenue. Implementing a granular engine exponentially increases the sonic palette available within the environment, cementing Orpheus as a powerful standalone tool for advanced sound design.

## 🎯 Metric Definition
- **Success** = Users can load any existing sample from the sample bank and pass it through a new `grain()` node, with the ability to modulate parameters like grain size, pitch, density, and position using the pattern language. The engine must support at least 50 simultaneous grains per voice with < 15% CPU overhead per voice, running completely lock-free on the real-time audio thread without dropouts or allocations.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation includes primitive oscillators and standard linear sample playback. There are no time-stretching or granular capabilities.
- **Competitors (TidalCycles, SuperCollider, Ableton Live):** TidalCycles integrates deeply with SuperDirt's granular capabilities (e.g., `chop`, `striate`). SuperCollider has a massive granular synthesis ecosystem (`TGrains`, `GrainBuf`). Ableton Live's simpler sampler and Warp modes provide rudimentary granular time-stretching.
- **The Gap:** Orpheus needs a dedicated DSP primitive capable of asynchronous grain scheduling and playback from an existing sample buffer, fully exposed to the pattern language's modulation system.

## ✅ Acceptance Criteria
- Must introduce a new `grain()` node to the Faust-style DSP graph in `orpheus-dsp/src/graph/primitives.rs`.
- Must support defining the sample source via the `SampleBank`.
- Must expose controllable parameters: `position` (where in the sample to read from), `size` (length of the grain), `density` (how often a grain fires), and `pitch` (pitch scaling of the grain).
- Must apply a smooth windowing function (e.g., Hann or Gaussian envelope) to each grain to prevent clicks and pops.
- Must pre-allocate a pool of voices for grain playback at initialization to ensure completely lock-free and allocation-free operation during the `process()` block on the audio thread.
- Must ensure that grain generation supports overlapping grains up to the configured maximum per-voice pool limit.

## 🚫 Out of Scope
- Granular synthesis on live audio input streams (real-time granular capture). Phase 1 is strictly for static audio buffers loaded from disk.
- Pitch tracking or automated transient matching for grains.
- Complex spatialization of individual grains (e.g., random panning per grain). Phase 1 will output standard stereo or mono based on the source buffer.
