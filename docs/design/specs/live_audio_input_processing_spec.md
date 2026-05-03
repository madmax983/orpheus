# 🔭 Vantage: Spec for Live Audio Input Processing

## 👤 User Story
"As a Live Coder and Performer, I want to route live audio from an external microphone or instrument (like a guitar) into Orpheus, so that I can process real-time external sounds using my coded DSP patterns, creating a hybrid acoustic/electronic performance."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a "closed box" instrument. It can only generate sound internally via synthesizers or by playing back pre-recorded sample files. This entirely excludes instrumentalists, vocalists, or field-recordists from interacting with the environment in real-time. By not supporting live audio input, Orpheus isolates itself from acoustic performers and hybrid electronic/acoustic setups. Complexity is a cost, but bridging the gap to the physical world is a massive utility multiplier. Allowing Orpheus to act as a programmable multi-effects processor for external instruments dramatically widens its appeal and usability on stage, transforming it from a pure synthesizer into an interactive performance hub.

## 🎯 Metric Definition
- **Success** = Users can connect an external audio interface, configure an input channel in the REPL (e.g., `:audio in 1`), and route that live audio stream through standard Orpheus effects chains (delay, reverb, filters) with an input-to-output round-trip latency of <10ms, maintaining stable playback without audio dropouts on the real-time thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The `RenderEngine` only reads data from internal DSP graphs or `SampleBank` memory. CPAL is configured only for an output stream.
- **Competitors (SuperCollider, Max/MSP, Ableton Live):** SuperCollider and Max/MSP excel at this, acting as powerful live effects processors (e.g., `SoundIn.ar` in SC). Ableton Live routes audio inputs natively to tracks.
- **The Gap:** Orpheus needs a CPAL input stream running concurrently with the output stream, an intermediary ring buffer to safely pass input frames to the DSP processing thread, and a new DSP node type (`AudioInNode`) to inject those frames into the `RenderEngine` graph.

## ✅ Acceptance Criteria
- Must initialize a concurrent CPAL input stream when the user starts Orpheus (or via a specific setup command).
- Must provide a lock-free, bounded ring buffer to shuttle incoming audio frames from the hardware input callback to the `RenderEngine` output callback.
- Must introduce a new language primitive or built-in function (e.g., `audioIn(channel)` or `mic()`) that can be used exactly like an oscillator or sample source in a pattern.
- Must ensure that routing live audio input does not introduce memory allocations on the audio thread.
- Must handle differing sample rates between input and output devices gracefully (or strictly require matching rates for Phase 1).

## 🚫 Out of Scope
- Advanced pitch-tracking or envelope following of the incoming audio (Phase 1 is strictly passing audio data through the effects graph).
- Live looping or sampling (recording the input to a buffer for later playback). Phase 1 is purely real-time pass-through processing.
