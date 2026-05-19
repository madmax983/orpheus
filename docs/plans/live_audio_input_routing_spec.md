# 🔭 Vantage: Spec for Live Audio Input Routing

## 👤 User Story
"As a Live Coder and Instrumentalist, I want to route live audio inputs (such as a microphone, guitar, or external hardware synth) directly into the Orpheus DSP graph alongside my digital patterns, so that I can apply real-time effects (delays, reverbs, granular synthesis) to external acoustic or hardware instruments using the same composition language I use for internal synthesis."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a closed-loop system: it generates sound internally via synthesis or pre-recorded samples. However, many live coders and electronic musicians do not perform purely "in the box"; they play physical instruments or sing while sequencing. If they cannot route their physical instruments through the Orpheus effects chain, they are forced to use a secondary DAW or hardware mixer to manage their performance, splitting their attention and diluting the unified coding experience. By bringing live external audio into the pattern language, Orpheus transforms from a standalone synthesizer into an interactive performance hub and multi-effects processor. This drastically expands the target audience to include hybrid electronic performers, guitarists, and vocalists. Complexity is a cost; utility is revenue. Supporting live input makes Orpheus useful in a vastly broader array of performance contexts.

## 🎯 Metric Definition
- **Success** = Users can define an audio input source (e.g., `input(1)`) and route it through any standard Orpheus effect or pedal pattern with less than 10ms of total round-trip audio latency. The live input must remain completely synchronized with the internal cycle clock and must not cause xruns, dropouts, or UI blocking when patched in or out on the fly.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation strictly originates from internal nodes (analog synth models, sample playback, plugin hosts). There is no mechanism to query or subscribe to the hardware audio interface's input streams.
- **Competitors (Ableton Live, SuperCollider, TidalCycles/SuperDirt):** Ableton Live handles external audio routing via dedicated Audio Tracks seamlessly. SuperCollider provides powerful `SoundIn` UGens for arbitrary microphone routing. TidalCycles supports routing live audio through SuperDirt effects (`# in 1`).
- **The Gap:** Orpheus lacks the capability to instantiate a DSP node that pulls frames from the physical audio input buffer. The pattern language needs a syntax primitive to represent a continuous external signal stream, and the underlying scheduler needs to coordinate the input buffers provided by the audio backend (e.g., CPAL) with the internal DSP render block.

## ✅ Acceptance Criteria
- Must introduce a command or syntax primitive (e.g., `input(channel_index)`) that represents a continuous live audio stream from the system's default audio input device.
- Must support applying standard Orpheus effects (e.g., `|> lpf(2000) |> delay(0.5)`) to the live input stream just as they are applied to samples or synths.
- Must support patching the live input through the new Pedal Behavior DSL for complex effect chains.
- Must gracefully handle situations where the requested input channel does not exist (e.g., failing silently or logging an error, but not panicking or crashing the audio thread).
- Must capture input buffers and pass them to the DSP graph without allocating memory on the audio thread or introducing excessive latency.

## 🚫 Out of Scope
- Multi-device aggregation (e.g., using one USB interface for input and a different interface for output simultaneously). Phase 1 relies on the system's default input device or the device specified during startup.
- Advanced input metering, feedback suppression, or noise gating out-of-the-box. (Users can construct noise gates via DSP graphs, but it is not a first-class feature of the input routing itself).
- Routing audio *out* to external hardware effects (hardware inserts). Phase 1 is strictly about bringing external audio *in* to the internal mixer.
