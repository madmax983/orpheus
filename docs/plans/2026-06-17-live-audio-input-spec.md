# 🔭 Vantage: Spec for Live Audio Input Processing

## 👤 User Story
"As an Instrumentalist and Live Coder, I want to route live audio from my microphone or guitar input into the Orpheus DSP engine, so that I can process real-time performances with the same sequenced effects, stutters, and routing I use for built-in patterns."

## ❓ The "So What?" (Business Problem)
Orpheus is currently limited to generating its own audio via synthesis or sample playback. However, modern live performances often blur the line between acoustic instrumentation (vocals, guitars) and electronic sequencing. If a guitarist cannot plug into Orpheus and sequence delay throws on their live playing, they will treat Orpheus as a toy or a secondary sequencer, relying instead on Ableton Live as the main hub. Complexity is a cost; utility is revenue. Adding live audio input expands the total addressable market from purely electronic producers to any live performer looking for programmable, code-driven effects.

## 🎯 Metric Definition
- **Success** = Users can define a live audio input source (e.g., `in("Mic 1")`), route it through the exact same DSP combinators (filters, delays, effects) as sequenced patterns, and hear the processed result at the master output with <10ms round-trip latency, without introducing xruns on the audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The `cpal` integration only initializes an output stream. There is no concept of an input stream, nor is there a DSP node capable of consuming an external real-time audio buffer.
- **Competitors (Ableton Live, SuperCollider):** Ableton natively supports tracking external audio with sequenced insert effects. SuperCollider has `SoundIn.ar` for reading from hardware inputs directly into the synthesis graph.
- **The Gap:** Orpheus needs to open a concurrent `cpal` input stream, pipe the incoming audio frames into a lock-free ring buffer, and introduce an `InputNode` in the DSP graph that reads from this buffer during processing.

## ✅ Acceptance Criteria
- Must introduce an `AudioInput` DSP primitive node.
- Must expand the engine integration to optionally open an input stream (if hardware permits) and push frames into a ring buffer.
- Must provide language syntax (e.g., `in("name")`) to represent the live audio stream as an effectable source, replacing the need for an underlying pattern of events.
- Must ensure that routing live audio through complex sequences (like `|> every(4, fast(2))`) applies the effect correctly to the incoming stream.
- Must maintain a lock-free, allocation-free path on both the input capture thread and the output render thread.

## 🚫 Out of Scope
- Phase 1 will not support recording the live input directly to disk; it is strictly for live pass-through processing.
- Advanced pitch detection or audio-to-MIDI conversion.
- Compensating for extreme hardware latency discrepancies (users must rely on standard low-latency ASIO/CoreAudio drivers).