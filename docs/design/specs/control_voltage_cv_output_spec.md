# 🔭 Vantage: Spec for Control Voltage (CV) Output

## 👤 User Story
"As a Modular Synth Performer and Live Coder, I want Orpheus to output direct current Control Voltage (CV) signals from its audio interface, so that I can sequence, modulate, and trigger my Eurorack hardware modules directly from pattern code."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus can only produce audio signals intended for speakers or headphones, and send MIDI data to external digital devices. A massive segment of the hardware synth community uses modular synthesizers (Eurorack, semi-modular), which are controlled via analog Control Voltage (CV), not MIDI. Without CV output capabilities, Orpheus cannot integrate into these hybrid hardware setups, forcing users to buy expensive MIDI-to-CV converters or rely on DAWs like Ableton (via CV Tools) or Bitwig (which has native CV support). Complexity is a cost; utility is revenue. By supporting DC-coupled audio interfaces to send CV, Orpheus becomes the ultimate programmable sequencer and modulation source for analog modular systems, capturing a highly engaged, high-spending demographic.

## 🎯 Metric Definition
- **Success** = Users can define a `cv()` or `trigger()` output target in the language, map pattern values (e.g., `0.0` to `1.0` or raw pitch values) to raw DC voltage levels, and route them to specific output channels on a DC-coupled audio interface. The signal must maintain sample-accurate timing alongside the main audio output, without applying AC-coupling filters (like DC-blockers).

## 🔍 Gap Analysis
- **Current State (Orpheus):** The DSP engine outputs AC-coupled audio. DC offset is explicitly avoided, and all outputs are assumed to be standard stereo/multichannel audio.
- **Competitors (Bitwig Studio, Ableton Live, VCV Rack):** Bitwig Studio has best-in-class native hardware CV integration (HW CV Out, HW Clock Out). Ableton Live provides "CV Tools" via Max for Live. VCV Rack is a dedicated virtual modular environment that seamlessly interfaces with hardware via DC-coupled interfaces.
- **The Gap:** Orpheus lacks the ability to generate static or slowly moving DC signals (LFOs, envelopes, pitch CV, gates) and direct them safely to specific hardware outputs without interference from the main mix bus or safety limiters.

## ✅ Acceptance Criteria
- Must introduce a `cv_out(channel, value_pattern)` primitive in the pattern language.
- Must support mapping standard pattern types (e.g., `Pattern<Number>`) to a raw floating-point range (usually `-1.0` to `1.0`) representing the voltage limits of the audio interface.
- Must provide a dedicated hardware routing mechanism in `orpheus-dsp` to bypass the main master bus, avoiding any global DC-blocking filters, limiters, or panning logic.
- Must support 1V/Octave pitch calibration, allowing a `Pattern<Note>` to be converted to a precise DC voltage offset.
- Must support generating sharp, sample-accurate Gate and Trigger signals for firing hardware envelopes.

## 🚫 Out of Scope
- CV Input (Phase 1 is output only).
- Automatic calibration of hardware oscillators (users must manually tune or define offsets initially).
