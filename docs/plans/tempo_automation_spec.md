# 🔭 Vantage: Spec for Tempo Automation

## 👤 User Story
"As a Composer and Live Performer, I want to automate macro-level tempo changes—such as smooth accelerandos (speeding up) or ritardandos (slowing down) over multiple bars—so that I can build dynamic, breathing musical structures and expressive transitions between sections, rather than being locked to a static grid."

## ❓ The "So What?" (Business Problem)
Electronic music, especially code-based music, often suffers from a feeling of mechanical stiffness. While micro-timing and swing help humanize rhythms within a cycle, macro-level tempo changes are required for larger structural expression (e.g., a "drop" transition, or a dramatic ending). Currently, if a user wants to change the tempo in Orpheus, they must manually issue a global command. This is abrupt and difficult to perform smoothly during a live set. Complexity is a cost; utility is a revenue. By providing a declarative way to automate tempo curves over time, we elevate Orpheus from a static pattern sequencer to an expressive musical instrument capable of conveying large-scale emotional dynamics.

## 🎯 Metric Definition
- **Success** = Users can define a tempo curve using pattern language (e.g., `tempo(line(120, 140, 4))`), which smoothly shifts the global BPM from 120 to 140 over 4 cycles. The underlying rational time model and DSP scheduler must smoothly interpolate these changes without causing audio dropouts, drift, or excessive jitter.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Tempo is a global, static scalar value that applies to the entire transport. There is no primitive or language support for sweeping or modulating the tempo over time.
- **Competitors (Ableton Live, Logic Pro):** Traditional DAWs provide master tempo tracks with extensive breakpoint automation. Code-based environments vary, but often require manual variable manipulation or external MIDI clock sweeps.
- **The Gap:** Orpheus lacks the ability to interpret patterns as global control signals for the transport layer, specifically for dynamically recalculating the cycle-to-real-time mapping.

## ✅ Acceptance Criteria
- Must introduce a way to evaluate a control pattern as the global tempo signal (e.g., `set_tempo(ramp(120, 140, 8))`).
- Must support basic automation curves: linear ramps (`ramp` or `line`) and discrete step sequencing.
- Must ensure that the audio engine's block scheduler dynamically adjusts to the shifting tempo without causing xruns or audio clicks.
- Must ensure that pattern queries remain deterministic and accurate relative to the changing real-world time.
- Must reflect the currently changing tempo smoothly in the TUI transport view.

## 🚫 Out of Scope
- Swing and micro-timing (already covered in a separate spec).
- Tempo automation syncing via Ableton Link. (Phase 1 focuses on automating the internal clock. Link sync with automation is a complex Phase 2 problem).
