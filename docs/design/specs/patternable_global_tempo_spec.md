# 🔭 Vantage: Spec for Patternable Global Tempo

## 👤 User Story
"As a Composer and Live Coder, I want to control the global BPM (tempo) using the same pattern syntax and functions that I use for audio generation, so that I can create complex tempo changes, ritardandos, accelerandos, and algorithmic time-warping curves over multiple bars without manually typing new BPM values."

## ❓ The "So What?" (Business Problem)
Currently, in most live coding environments and Orpheus, tempo is treated as a static global state, adjustable only via explicit manual transport commands (e.g., `:bpm 120`). Music, however, breathes. Classical music relies heavily on expressive tempo variation (rubato), and modern electronic subgenres explore extreme, elastic time manipulations. If tempo remains a static variable, users are restricted to rigid, grid-locked compositions or tedious manual updates. Complexity is a cost; utility is revenue. By elevating the global tempo to a first-class pattern target, we unify the language model (everything is a pattern) and unlock deeply expressive, algorithmic time manipulation. It changes Orpheus from a rigid drum machine into an elastic, breathing instrument.

## 🎯 Metric Definition
- **Success** = Users can define a global pattern for tempo (e.g., `bpm = 120 130 140 |> slow(4)`) and the pattern engine evaluates this continuously, updating the master transport tempo at the control-rate or per-event boundary without causing audio glitches, drift, or phase misalignment between tracks.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Tempo is a static value held in the transport state. It dictates the duration of a cycle. It cannot be patterned or modulated automatically over time.
- **Competitors (Ableton Live, SuperCollider, TidalCycles):** Ableton Live has master tempo automation lanes. SuperCollider allows TempoClocks to be modulated freely. TidalCycles supports `# cps` (cycles per second) patterning, making algorithmic tempo deeply embedded in its workflow.
- **The Gap:** Orpheus separates transport control from pattern evaluation. The scheduler needs to support dynamic, pattern-driven time advancement where the length of the next cycle is determined by evaluating the tempo pattern itself.

## ✅ Acceptance Criteria
- Must introduce a reserved global binding or specific function (e.g., `bpm(...)` or overriding the `bpm` variable) that accepts a `Pattern<Number>`.
- Must evaluate the tempo pattern continuously to calculate the duration of the upcoming cycle or subdivision.
- Must support standard pattern transformations on the tempo (e.g., `bpm = 120 |> every(4, fast(2)) |> jux(rev)`).
- Must recalculate DSP event scheduling accurately when tempo drifts mid-cycle (e.g., a smooth sine wave applied to BPM).
- Must prevent negative or zero tempo values, capping or clamping them to safe musical boundaries to prevent scheduler infinite loops.

## 🚫 Out of Scope
- Track-specific or per-layer independent tempos (polytempi where tracks drift out of phase). Phase 1 enforces a single, global, but patternable master tempo.
- Real-time tempo extraction/syncing from external audio input.
