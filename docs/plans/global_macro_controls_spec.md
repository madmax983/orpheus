# 🔭 Vantage: Spec for Global Macro Controls

## 👤 User Story
"As a Live Coder, I want to map a single, global macro parameter to multiple different track parameters simultaneously, so that I can create massive, synchronized build-ups and drops during a live performance with a single command or physical knob turn."

## ❓ The "So What?" (Business Problem)
Currently, if a performer wants to execute a complex musical transition—like opening a filter on a synth pad, increasing the decay on a hi-hat, and turning up a master reverb send simultaneously—they must manually type and evaluate multiple separate lines of code. This is slow, error-prone under pressure, and breaks the musical tension because the changes happen sequentially. Complexity is a cost; utility is revenue. Adding global macro controls gives the performer the "big knob" experience common in DJ software and DAWs. It allows for highly expressive, synchronized parameter sweeps that transform Orpheus into a fluid live performance instrument, dramatically increasing its value for on-stage improvisation.

## 🎯 Metric Definition
- **Success** = Users can define a global macro variable (e.g., `$tension`) and map it to any number of distinct track parameters. Updating the macro variable instantly reflects across all mapped properties simultaneously on the next audio frame, with zero audio dropouts or locking, evaluating in <1ms per audio block.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Parameters are mapped to static values or track-specific sequenced patterns. There is no built-in reactive state mechanism to bind multiple disjoint tracks to a single master variable.
- **Competitors (Ableton Live, Bitwig, Renoise):** Ableton and Bitwig feature "Macro Controls" prominently on Instrument and Effect Racks. Renoise has global meta-devices that map one slider to multiple track DSP parameters.
- **The Gap:** Orpheus lacks a reactive variable system where a single global value change automatically propagates to and scales multiple dependent functions within the DSP graph.

## ✅ Acceptance Criteria
- Must introduce syntax to declare and update a global macro variable (e.g., `let tension = 0.0`).
- Must introduce a mapping function to translate the macro range to the target parameter's range (e.g., `|> cutoff(scale(tension, 0, 1, 200, 8000))`).
- Must instantly propagate changes of the macro variable to all dependent parameters simultaneously across all active tracks.
- Must perform the parameter updates using lock-free data structures (like `ArcSwap` or atomics) to ensure the real-time audio thread is never blocked.
- Must allow MIDI CC inputs to map directly to a global macro variable.

## 🚫 Out of Scope
- Internal LFO or Envelope modulation of macros. Phase 1 is strictly for manual modification (via code evaluation or MIDI input).
- Non-linear mapping curves (e.g., logarithmic or exponential maps) for Phase 1. Simple linear scaling is sufficient for MVP.
- A dedicated GUI for macro knobs. Control remains via text or MIDI.
