# 🔭 Vantage: Spec for Live Sampling and Auto-Slicing

## 👤 User Story
"As a Beatmaker and Live Performer, I want to route live audio input into my session, record it dynamically into buffers, and have Orpheus automatically detect transients within those buffers so that I can instantly chop, sequence, and manipulate live acoustic performances or incoming loops without manual slice calculation or interrupting my flow."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has two major limitations regarding external audio: it only operates on static WAV files loaded at startup, and slicing those files requires manual, rigid fractional math. This creates a high-friction barrier between the performer and the organic world. If a musician wants to beatbox a rhythm and immediately loop/chop it, they cannot do so within Orpheus. They are restricted to pre-meditated material. Complexity is a cost; utility is revenue. Bridging the gap between the digital sequencer and live acoustic input, while simultaneously automating the tedious process of finding chop points, transforms Orpheus from a closed-loop step sequencer into an interactive, real-time performance hub and powerful sampler. It drastically lowers the barrier to entry for working with raw audio.

## 🎯 Metric Definition
- **Success** = Users can record live audio input into a named buffer via a command (e.g., `:record 4`), and Orpheus automatically analyzes that buffer offline (off the audio thread) to detect transients with >90% accuracy in <100ms. The user can immediately sequence these slices using an `onset(index)` function with zero audio dropouts, xruns, or UI latency. Round-trip audio latency for the live input must remain <10ms.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation is limited to built-in DSP or pre-loaded WAV files. Slicing is manual (`slice(start, end)`) or mathematically subdivided (`slice_idx(i, n)`), completely unaware of the actual audio content.
- **Competitors (Ableton Live, Renoise, SuperCollider, TidalCycles):** Ableton Live excels at live looping and transient-based slicing ("Slice to New MIDI Track"). Renoise features robust auto-slicing. SuperCollider provides raw tools for buffer recording and onset detection but requires complex setup.
- **The Gap:** Orpheus lacks both a real-time audio input ingestion layer and an analysis step to identify musical onset points. It needs an integrated workflow to capture a buffer and instantly expose its rhythmic regions to the pattern language.

## ✅ Acceptance Criteria
- Must introduce a mechanism to route system audio input into the Orpheus mixer chain.
- Must provide a command to record a specific duration of live input (e.g., 1 cycle or 4 beats) into a named, in-memory sample buffer.
- Must run a fast transient detection algorithm (e.g., spectral flux) automatically on newly recorded buffers (or dynamically loaded files) on a background thread.
- Must expose detected slices to the pattern language, e.g., via a new function `onset(index)` that plays the slice from the `index`th transient to the next.
- Must allow the newly recorded and sliced buffer to be played back instantly and processed by existing effect buses.
- Must handle file I/O and analysis without blocking or allocating on the real-time audio thread.

## 🚫 Out of Scope
- Multi-channel input routing (e.g., separate routing for 8 ADAT inputs). Phase 1 targets the default stereo/mono input.
- Real-time transient detection on the live stream *before* it is recorded to a buffer. Phase 1 analyzes static buffers.
- Complex beat-warping to conform sliced loops to a new tempo. Phase 1 provides the slice boundaries; the user sequences them.
