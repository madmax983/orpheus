# 🔭 Vantage: Spec for Wavetable Synthesis Engine

## 👤 User Story
"As a Sound Designer and Live Coder, I want a wavetable synthesis engine in Orpheus, so that I can morph smoothly between complex waveforms and design evolving, modern digital timbres (like heavy basslines and shimmering pads) using pattern language modulators."

## ❓ The "So What?" (Business Problem)
Orpheus currently relies on primitive analog-modeled oscillators (saw, square, sine, triangle) and static sample playback. While subtractive synthesis is great for classic sounds, modern electronic music (Dubstep, Drum & Bass, modern IDM, and Pop) heavily relies on the complex, evolving harmonic structures provided by wavetable synthesis (popularized by Massive and Serum). Without this, the sonic palette of Orpheus remains limited, forcing users to rely on external VSTs or DAWs to achieve modern sounds. Complexity is a cost; utility is revenue. Implementing a native wavetable engine massively expands Orpheus's timbral capabilities, attracting a wider demographic of modern electronic music producers to the platform.

## 🎯 Metric Definition
- **Success** = Users can load a 3D wavetable (an array of single-cycle waveforms) and modulate the `position` parameter via the pattern language (e.g., `wt(position=lfo(rate=0.5))`). The engine must interpolate smoothly between adjacent frames without aliasing or audio dropouts, consuming <10% CPU overhead per voice, running completely lock-free on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation includes primitive oscillators (sine, saw, square) and sample playback. There are no mechanisms for smoothly interpolating between different waveform arrays.
- **Competitors (Ableton Live, Serum, SuperCollider):** Serum and Ableton's Wavetable are industry standards for modern sound design. SuperCollider has `Osc`, `VOsc`, and `VOsc3` for wavetable playback and interpolation.
- **The Gap:** Orpheus lacks a DSP primitive that reads from a pre-calculated 2D or 3D buffer of waveforms and provides a modulatable index/position parameter to scan through them at audio rates.

## ✅ Acceptance Criteria
- Must introduce a new `wavetable()` node to the Faust-style DSP graph in `orpheus-dsp`.
- Must support loading standard wavetable files (e.g., WAV files containing multiple single-cycle frames of a fixed length, such as 2048 samples per frame) via the `SampleBank` or a dedicated `WavetableBank`.
- Must expose controllable parameters: `position` (0.0 to 1.0 index of the current waveform frame), and standard oscillator parameters (`frequency`, `phase`).
- Must implement linear or cubic interpolation between adjacent samples within a frame, AND interpolation between adjacent frames (crossfading) to ensure smooth morphing when `position` is modulated.
- Must ensure that loading a wavetable pre-allocates necessary buffers, allowing the real-time audio thread to perform wavetable scanning and interpolation lock-free and allocation-free.

## 🚫 Out of Scope
- A visual wavetable editor or 3D waveform visualizer in the TUI. Phase 1 focuses entirely on the DSP playback engine.
- Wavetable generation via additive synthesis or spectral manipulation in real-time. Phase 1 plays back pre-calculated wavetable files.
- Unison/Supersaw generation built directly into the wavetable node. Users can stack multiple nodes in the DSP graph for unison effects.
