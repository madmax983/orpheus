# 🔭 Vantage: Spec for Hardware CV/Gate Output

## 👤 User Story
"As a Modular Synth Performer, I want to route Orpheus patterns to DC-coupled audio interfaces as Control Voltage (CV) and Gate signals, so that I can sequence external analog hardware directly from my code."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus exists in a purely digital, in-the-box ecosystem. While MIDI output solves sequencing for modern digital and MIDI-equipped analog gear, a massive segment of modern electronic performance relies on Eurorack and modular synthesizers. Modular synthesis requires high-resolution, continuous analog voltage (CV) and precise triggers (Gate). By lacking CV/Gate output capabilities, Orpheus cannot integrate into the physical, tactile world of modular synthesis, forcing performers to use intermediate, expensive MIDI-to-CV converters which add latency and reduce resolution. Complexity is a cost; utility is revenue. Bridging the gap between declarative, exact-rational coding and raw analog hardware acts as a massive utility multiplier, positioning Orpheus as the ultimate modular sequencer brain.

## 🎯 Metric Definition
- **Success** = Users can assign a pattern layer to a dedicated CV/Gate output channel via a command. The engine must produce 1V/Octave scaled audio signals for pitch and 5V pulse signals for gates, outputting them via a DC-coupled interface with sample-accurate timing and <2ms latency, without causing any blocking on the audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The system outputs strictly standard audio signals intended for speakers. It has no concept of scaling digital values to physical voltage standards.
- **Competitors (Ableton Live, Bitwig, TidalCycles):** Bitwig Studio features robust, native hardware integration (The Grid, HW CV Out). Ableton Live relies on its "CV Tools" suite. TidalCycles typically achieves this by sending OSC/MIDI to external bridging software.
- **The Gap:** Orpheus needs a dedicated DSP node or output routing mechanism that converts `Pattern<Note>` and `Pattern<Number>` into standard, calibrated CV (1V/Oct) and Gate signals, safely routing them directly to specified channels on a DC-coupled interface.

## ✅ Acceptance Criteria
- Must introduce a new routing mechanism or primitive (e.g., `cvOut(channel, signal)`) to send continuous signals and triggers to specific audio output channels.
- Must provide built-in calibration or scaling logic to convert Orpheus's internal pitch representation to the standard 1V/Octave format.
- Must provide logic to generate precise, reliable Gate signals (e.g., a 5V square pulse) when Note events are triggered.
- Must bypass the master bus safety limiter for channels designated as CV, ensuring the raw voltage values are not compressed or clipped unintentionally.
- Must execute the CV/Gate processing within the exact rational scheduling engine, ensuring sample-accurate trigger timing.

## 🚫 Out of Scope
- Hardware calibration tools within the TUI (e.g., measuring interface offset/scaling errors). Phase 1 relies on assumed ideal scaling.
- Audio input processing for CV/Gate (CV *In*). Phase 1 is strictly sequencing external hardware (CV *Out*).
- Support for AC-coupled interfaces (which require complex high-frequency encoding like Expert Sleepers). Phase 1 assumes a natively DC-coupled audio interface.
- 1.2V/Octave (Buchla) or Hz/Volt (Korg/Yamaha) standards. Phase 1 targets the Eurorack 1V/Oct standard.
