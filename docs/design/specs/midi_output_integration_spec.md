# MIDI Output Integration Spec

## 👤 User Story
"As a Live Coder, I want to route Orpheus patterns to external MIDI devices, so that I can sequence hardware synthesizers or external DAWs using the Orpheus pattern engine."

## ❓ So What? (Business Problem)
Orpheus currently requires all synthesis to be done internally via its custom DSP engine. While educational, this alienates musicians with existing hardware workflows or who rely on external software instruments (VSTs), severely limiting adoption. By supporting MIDI out, Orpheus can immediately act as a powerful standalone sequencer, unlocking integration with the broader music production ecosystem and increasing its utility exponentially.

## 📊 Metric Definition
- **Success** = Event scheduling jitter for MIDI output is < 2ms.
- **Success** = Users can discover and route to connected MIDI devices via the REPL.

## ✅ Acceptance Criteria
- Must provide a way to discover available MIDI output ports from the REPL (e.g., `:midi-ports`).
- Must introduce a pattern target mechanism (e.g., `midiOut("Device Name", channel_number)`) alongside internal synth targets.
- Must accurately translate Orpheus temporal events (note start and duration) into matched MIDI Note On and Note Off messages.
- Must support mapping pattern control signals (like `cc(74, 0.5)`) to MIDI Control Change (CC) messages.
- Must handle cycle-boundary interruptions cleanly (e.g., sending Note Off for sustained notes if the pattern changes or stops).

## 🚫 Out of Scope
- MIDI Input (controlling Orpheus via an external MIDI keyboard) is Phase 2.
- MIDI Clock Output / Sync (handled separately by Ableton Link).
- SysEx (System Exclusive) message generation.
- MPE (MIDI Polyphonic Expression).
