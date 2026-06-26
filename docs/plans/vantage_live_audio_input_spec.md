# 🔭 Vantage: Spec for Live Audio Input

## 👤 User Story
"As a Live Coder and Performer, I want to route live audio from external sources (like a microphone, electric guitar, or hardware synthesizer) into Orpheus, so that I can process them with Orpheus's built-in effects, DSP graph, and rhythmic pattern gates as part of my live set."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a "closed box" instrument. It can generate its own sound and load static sample files, but it cannot listen to the real world. Many electronic musicians and live coders perform with hybrid setups involving acoustic instruments or vocals. By lacking live audio input, Orpheus forces performers to run a separate DAW or mixer just to apply effects to their physical instruments. Complexity is a cost; utility is a revenue. Adding live audio input expands Orpheus from a sequencer into a central live performance hub and multi-effects processor, vastly increasing its utility for hybrid electronic artists.

## 🎯 Metric Definition
- **Success** = Orpheus can capture stereo or mono audio from a system audio input device, route it into the internal DSP graph with < 5ms of latency, and allow applying built-in pattern transformations (e.g., gating, filtering, delaying) to the live stream without dropouts or allocations on the audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation is limited to internal oscillators, loaded WAV samples, or plugin hosts. There is no way to capture real-time audio from the OS microphone/line-in.
- **Competitors (SuperCollider, Sonic Pi, Ableton Live):** Ableton Live is the industry standard for this (Audio Tracks). SuperCollider handles `SoundIn` effortlessly. Sonic Pi allows live audio routing via `live_audio :mic`.
- **The Gap:** Orpheus needs a new language primitive to represent a live audio stream as a continuous sound source, seamlessly integrating it into the existing `Pattern` and effects architecture.

## ✅ Acceptance Criteria
- Must introduce a REPL/TUI command to list (`:audio in list`) and connect to (`:audio in connect "Device Name"`) available system audio input devices.
- Must provide a language primitive (e.g., `in(1)` or `live_audio("mic")`) that can be bound to a track or routed through the mixer just like a synth or sample pattern.
- Must allow standard Orpheus effects (e.g., `lpf`, `reverb`, `delay`) to process the incoming live audio.
- Must support rhythmic gating or slicing of the live audio using Orpheus patterns (e.g., applying `struct` or boolean masks to rhythmically mute the live feed).
- Must handle audio input on a lock-free real-time path to ensure no xruns or blocking occur when bringing audio from the OS into the Orpheus render graph.

## 🚫 Out of Scope
- Real-time pitch tracking or audio-to-MIDI conversion (e.g., singing a melody to control a synth).
- Live looping or sampling (recording the input into a buffer for playback later). Phase 1 is strictly real-time passthrough processing.
