# 🔭 Vantage: Spec for Microtonal and Alternate Tuning Support

## 👤 User Story
"As a Composer and Sound Designer, I want to define and apply custom microtonal scales and alternate tuning systems (like Just Intonation or Bohlen-Pierce) to my patterns, so that I can break free from 12-tone equal temperament (12-TET) and explore entirely new harmonic landscapes within Orpheus."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus assumes all pitch logic and frequency math is based on standard Western 12-tone equal temperament (A4 = 440Hz). This is a massive constraint for global music producers, experimental electronic artists, and contemporary classical composers. By forcing 12-TET, we artificially limit the expressive range of the built-in synthesizers and alienate users who work in non-Western traditions or avant-garde microtonal music. Complexity is a cost, but hardcoding a single cultural tuning system is a product ceiling. Supporting alternate tuning via standard formats (like Scala `.scl` files) transforms Orpheus from a traditional Western sequencer into a culturally agnostic, limitless harmonic playground, dramatically expanding its appeal to experimental and international artists.

## 🎯 Metric Definition
- **Success** = Users can load a custom tuning file (e.g., Scala format `.scl` or an inline array of frequency ratios/cents) and apply it to a pattern globally or per-track. The internal DSP engine must accurately render the exact specified frequencies for notes, with <1ms overhead during pitch calculation, and zero audio dropouts when switching tunings live.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Pitch is tightly coupled to 12-TET. MIDI note numbers translate directly to frequencies using the standard `440.0 * 2^((note - 69)/12)` formula. No mechanism exists to alter the octave division or reference frequency.
- **Competitors (Ableton Live, Bitwig, TidalCycles):** Bitwig has excellent native micro-pitch support. Ableton Live 11+ natively supports Scala and MTS-ESP. TidalCycles can do microtonal math but often requires custom functions or configuring SuperDirt.
- **The Gap:** Orpheus needs a tuning abstraction layer between the pattern language's logical notes (degrees/steps) and the DSP engine's absolute frequencies, allowing users to hot-swap the mapping dictionary.

## ✅ Acceptance Criteria
- Must introduce a command or function to load custom tuning maps from standard formats (e.g., Scala `.scl` files) or define them inline (e.g., `tuning([1.0, 1.1, 1.25, ...])`).
- Must support applying a specific tuning mapping to a pattern (e.g., `|> tune("just_intonation")`).
- Must correctly calculate frequencies for built-in synthesizers (like PolyBLEP saw/pulse) based on the active tuning table rather than hardcoded 12-TET math.
- Must allow changing the reference frequency (e.g., `A4 = 432Hz` or a custom root frequency for ratio-based scales).
- Must handle out-of-bounds note values gracefully (e.g., wrapping them into the correct octave ratios).

## 🚫 Out of Scope
- MIDI Tuning Standard (MTS) sysex messages sent out to external hardware. Phase 1 focuses exclusively on the internal DSP engine's tuning.
- Dynamic tuning adjustments based on harmonic context (e.g., Hermode tuning). Phase 1 is static tuning tables only.
- Complex visual tuning editors or UI widgets. Phase 1 relies on text-based definitions and file loading.