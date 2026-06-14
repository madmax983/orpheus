# 🔭 Vantage: Spec for Audio Input Live Processing

## 👤 User Story
"As a Live Coder and Instrumentalist, I want to route live audio input (e.g., guitar, microphone) into Orpheus and process it using Orpheus's pattern language and effects, so that I can perform collaboratively with the system."

## 🤔 The "So What?" Ask
**What business problem does this solve?**
Orpheus is currently limited to generating its own audio (synthesizers and samples). This isolates the performer. By accepting live audio input, Orpheus transforms from a standalone sequencer into an integrated live-performance instrument and multi-effects processor. This vastly expands the addressable market to include instrumentalists and vocalists who want to augment their live sound with code-driven transformations.

## 📏 Metric Definition
- **Success:** Audio input latency is strictly < 10ms round-trip (input -> DSP -> output) at a 256-sample buffer size for 99% of cycles.
- **Success:** Users can successfully route `in(1)` (input channel 1) through an effect chain (e.g., `in(1) |> lpf(400) |> delay(0.5)`) seamlessly with synthetically generated tracks.
- **Success:** Zero audio thread panics or buffer underruns introduced by the live input stream under normal load.

## 🕵️ Gap Analysis
- **Market:** Ableton Live and SuperCollider excel at this. TidalCycles often delegates this to SuperDirt. Orpheus needs this natively.
- **Current State:** Orpheus's `orpheus-dsp` only generates audio out; `cpal` integration only initializes an output stream.
- **Standard Libs:** We can leverage `cpal`'s input stream capabilities, but we need to manage the synchronization between the asynchronous input callback and the output DSP graph safely.

## ✅ Acceptance Criteria
- Must be able to capture at least one mono or stereo audio input stream via the configured audio interface.
- Must provide a language primitive (e.g., `in(channel)`) that acts as a continuous audio source in the pattern language.
- Must be able to pipe the input source through all existing Orpheus effects (e.g., filters, delays, reverbs) using the standard pipe (`|>`) operator.
- Must handle temporary audio dropouts or hardware disconnections gracefully without crashing the whole application.
- Must operate within the existing cycle-based timing model, ensuring patterned effect controls align with the incoming audio stream.

## 🚫 Out of Scope
- Phase 1: Real-time pitch detection or onset detection (triggering patterns based on audio input).
- Phase 1: Auto-looping or live-sampling (recording the input to a buffer to play back later).
- Phase 1: Complex multi-channel routing (more than 2 input channels).