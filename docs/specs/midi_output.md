# 🔭 Vantage: Spec for MIDI Output

## 👤 User Story
As a live-coding performer or hardware enthusiast, I want to route Orpheus patterns to output MIDI messages, so that I can sequence external hardware synthesizers, drum machines, and DAWs, using Orpheus as the central brain of a larger studio setup.

## 💼 So What? (Business Problem)
Currently, Orpheus is an island. It only makes sounds through its own internal DSP engine. While this is great for a self-contained experience, the real-world standard for electronic music production relies heavily on integrating with DAWs (like Ableton) and external hardware synths. Without MIDI output, we alienate users who have invested in hardware or prefer external VSTs. Adding MIDI output transforms Orpheus from a standalone "toy" engine into a professional, interoperable composition tool, massively expanding our addressable user base.

## 🎯 Metric Definition (Success Criteria)
- **Acceptance Criteria:**
  - Users can define a pattern and direct it to a named MIDI output port instead of (or alongside) the internal DSP.
  - System accurately translates Orpheus temporal events into MIDI Note On / Note Off messages with correct velocities.
  - System translates continuous control patterns (e.g., Orpheus control functions) into MIDI Control Change (CC) messages.
  - Timing jitter must be imperceptible: MIDI events must be dispatched within < 2ms of their exact calculated cycle time.
- **Success Metric:** At least 20% of active `.ode` projects include a MIDI routing directive within 3 months of launch.

## 🚫 Out of Scope (Phase 1)
- **MIDI Input:** We are not receiving external MIDI clock, Note On, or CC data. This is strictly out.
- **MPE (MIDI Polyphonic Expression):** Standard MIDI 1.0 only.
- **Sysex Messages:** Too complex for the initial spec.
