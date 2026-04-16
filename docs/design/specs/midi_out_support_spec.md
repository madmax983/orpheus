# 🔭 Vantage: Spec for MIDI Out Support

## 👤 User Story
As a live-coding performer and electronic musician, I want to send MIDI notes, Control Change (CC) messages, and clock data from my Orpheus patterns to external hardware synthesizers and software DAWs, so that I can use Orpheus as the central brain to sequence my entire studio setup.

## 🧐 The "So What?" (Business Problem)
**Why do we need this?**
Currently, Orpheus is an island. It generates its own audio internally via `orpheus-dsp` and cannot control external instruments. Many electronic musicians have significant investments in hardware synths or prefer the sound engines of commercial DAWs (like Ableton Live or Logic Pro). By not supporting MIDI Out, we are alienating users who want to use a code-based sequencer but don't want to abandon their existing sound sources.

MIDI Out transforms Orpheus from a standalone "toy" into a professional control center that can integrate into any existing studio ecosystem.

## 📏 Metric Definition
Success is defined by:
- **Latency & Jitter:** MIDI messages must be sent with < 5ms of jitter relative to the internal audio clock.
- **Throughput:** Capable of sending dense polyphonic sequences (e.g., 32nd notes across 16 channels) without dropping messages or choking the audio thread.
- **Adoption/Usage:** At least 30% of active users utilize MIDI Out patterns alongside or instead of internal DSP patterns within 3 months of release.

## 🕳️ Gap Analysis
- **Current State (Orpheus):** Everything stays inside `orpheus-dsp`. There is no concept of a "MIDI event" leaving the system.
- **Competitors (TidalCycles, Sonic Pi, Strudel):** All major live-coding environments support MIDI Out. TidalCycles achieves this via SuperDirt bridging, Sonic Pi has native MIDI functions, and Strudel supports WebMIDI.
- **The Gap:** Orpheus needs a dedicated pattern evaluator that maps Orpheus values (notes, velocity, duration) to system MIDI APIs (CoreMIDI, ALSA, Windows MIDI) while maintaining strict synchronization with the cycle clock.

## ✅ Acceptance Criteria
1. **Device Discovery:** The user must be able to list available MIDI output devices from within the REPL/TUI.
2. **Device Connection:** The user must be able to explicitly connect a pattern to a specific named MIDI output device and channel.
3. **Note Scheduling:** Patterns must correctly translate Orpheus note values, durations, and velocities into paired `Note On` and `Note Off` MIDI messages, scheduled accurately according to the cycle.
4. **CC Support:** Users must be able to sequence Control Change (CC) messages (e.g., sweeping a filter cutoff via a sine wave pattern) mapped to specific CC numbers.
5. **Clock Sync:** Orpheus must optionally send MIDI Clock, Start, Stop, and Continue messages to keep external sequencers in sync with the Orpheus tempo.
6. **Panic Button:** There must be a "Panic" command to send "All Notes Off" and "Reset All Controllers" to handle stuck notes.

## 🚫 Out of Scope (Phase 1)
- **MIDI In:** Receiving MIDI to trigger patterns or control Orpheus parameters.
- **MPE (MIDI Polyphonic Expression):** Advanced per-note modulation.
- **SysEx (System Exclusive):** Sending complex, device-specific configuration blobs.
- **14-bit CCs:** Standard 7-bit CC resolution is sufficient for MVP.
