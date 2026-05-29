# Spec for Hardware Controller Mapping

## 👤 User Story
As a live-coding performer, I want to map MIDI hardware controller knobs and faders directly to Orpheus pattern parameters (like gain, cutoff, and speed) so that I can perform expressive, continuous musical changes without having to type every single parameter update during a live set.

## ❓ So What? (The Business Problem)
Live coding relies heavily on typing speed. Continuous, nuanced changes (like a slow filter sweep over 16 bars or fading out a track) are tedious and mathematically heavy to express in code on the fly. This limits the performer's ability to be expressive and responsive to the crowd. By allowing physical hardware (MIDI controllers) to drive parameters, we bridge the gap between "programming as composition" and "physical performance," making Orpheus a more practical and attractive tool for traditional electronic musicians transitioning into live coding.

## 📏 Metric Definition
Success =
- A user can map a MIDI CC input to a pattern parameter (e.g., `lpf`) in fewer than 15 seconds.
- Continuous hardware parameter sweeps reflect in the audio DSP engine with < 10ms latency.
- 0% increase in audio thread dropouts during heavy MIDI CC influx.

## 🕳️ Gap Analysis
- **Market (DAWs):** Ableton Live and Bitwig excel at MIDI mapping (CMD+M / CTRL+M -> click -> twist). It is a gold standard for performance.
- **Market (Live Coding):** Sonic Pi and TidalCycles support MIDI/OSC input, but it often requires writing boilerplate code to listen to specific control change (CC) channels and mapping them manually to variables.
- **Our Gap:** We currently have no mechanism for a parameter (like `gain(0.8)`) to be replaced by a live external signal (`gain(cc(45))`). We need a declarative, zero-boilerplate way to inject MIDI CC streams into our pattern evaluation engine.

## ✅ Acceptance Criteria
- Must provide a syntax for binding a MIDI CC channel to a DSP parameter (e.g., `lpf(cc(74))`).
- Must allow optional scaling of the incoming 0-127 MIDI values to standard DSP ranges (e.g., `cc(74, 200, 5000)` for frequency).
- Must handle rapid influx of MIDI CC messages lock-free, passing the latest value to the DSP engine on each block without panicking or allocating.
- Must persist defined MIDI mappings across REPL reloads for the duration of the session.

## 🚫 Out of Scope
- Auto-detecting and mapping specific branded hardware controllers (e.g., Novation Launchpad, APC40) automatically out of the box (Phase 2).
- Bi-directional MIDI mapping (sending LED feedback back to the controller).
- Pitch bend and aftertouch mapping (sticking to standard Control Change [CC] for Phase 1).
