# 🔭 Vantage: Spec for MIDI Output Integration

## 👤 User Story
As a Live-Coder or Producer, I want to route Orpheus patterns to external hardware synthesizers and software instruments via MIDI, so that I can use my existing hardware gear and DAWs alongside Orpheus's algorithmic composition and exact rational timing.

## The "So What?" Ask
**What business problem does this solve?**
Currently, Orpheus is a closed audio ecosystem. Users can generate patterns, but they are stuck inside Orpheus's internal DSP. By adding MIDI output, Orpheus immediately inherits the sound generation capabilities of the user's entire studio (hardware synths, Ableton Live, Logic, etc.). This transitions Orpheus from an "interesting standalone toy" into a "powerful master sequencer" for professional production workflows. Complexity is shifted from writing DSP in Rust to leveraging existing setups, vastly increasing utility.

## Metric Definition
* **Success =** Jitter < 2ms for generated MIDI note events when routed to a virtual loopback port.
* **Success =** 100% accurate translation of `Note` literal patterns (`C4`, `Eb3`) and velocity (`gain()`) to MIDI Note On/Off messages.
* **Success =** The feature can be enabled/disabled per track without interrupting global audio output.

## Gap Analysis
* **Market Standard (TidalCycles):** Tidal integrates seamlessly with external synths via "SuperDirt" MIDI mapping or raw OSC/MIDI forwarding.
* **Market Standard (Ableton Live):** Every track can seamlessly switch between internal instruments and external MIDI output.
* **Orpheus Current State:** Only internal soft-synths and samplers are supported. There is no way to send pattern data out of the application.

## ✅ Acceptance Criteria
* **Language Integration:** A new keyword/function (e.g., `midi("port_name", channel)`) exists to declare a track as a MIDI output destination instead of an audio DSP destination.
* **Timing Translation:** Orpheus's exact rational time spans must be translated into real-time MIDI Note On and Note Off events on a background thread.
* **Parameter Mapping:**
  - `pitch()` or note literals map to MIDI Note Numbers.
  - `gain()` maps to MIDI Velocity (0-127).
  - `pan()` maps to MIDI CC 10 (Pan) or is ignored based on user preference.
* **Device Discovery:** Users must be able to list available MIDI output ports from the REPL/TUI.
* **Non-Blocking:** MIDI output must not block or stall the internal cpal audio thread.

## 🚫 Out of Scope
* MIDI Clock Sync output (Phase 2).
* MIDI Input mapping/recording (Phase 2).
* Polyphonic Aftertouch or MPE support.
* Receiving MIDI CCs to control internal Orpheus parameters.
