# 🔭 Vantage: Spec for Dynamic Macro Controls

## 👤 User Story
"As a Live Performer, I want to map multiple DSP parameters (like filter cutoff, resonance, and delay feedback) to a single named macro control, so that I can perform complex, sweeping sonic changes with a single command or MIDI knob turn during a live set."

## ❓ The "So What?" (Business Problem)
Live coding shines in creating complex, algorithmic sequences, but can feel rigid when attempting expressive, multi-parameter performance gestures. Currently, if a user wants to build tension by simultaneously opening a filter, increasing resonance, and boosting a send effect, they must write complex code for three separate properties. This is nearly impossible to execute smoothly in real-time. By introducing "Dynamic Macro Controls" (similar to Ableton's Audio Effect Racks), Orpheus bridges the gap between composition and performance. Complexity is a cost; utility is revenue. Allowing users to consolidate multi-dimensional parameter changes into a single, intuitive macro exponentially increases the software's utility as a highly expressive, real-time instrument.

## 🎯 Metric Definition
- **Success** = Users can define a macro (e.g., `let build_up = macro(cutoff: 200..8000, res: 0.1..0.8)`) and apply it to a pattern (e.g., `|> build_up(0.5)`). The audio engine must smoothly interpolate all mapped parameters simultaneously with <5ms overhead per cycle, zero allocations on the real-time thread, and no audio dropouts.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Parameters are isolated. Each must be modulated individually (e.g., `|> cutoff(400) |> res(0.5)`).
- **Competitors (Ableton Live, Bitwig, Native Instruments):** All major DAWs feature "Macro Knobs" that map one input to multiple destination parameters with custom ranges. Live coding tools often require users to manually write the scaling math for every parameter.
- **The Gap:** Orpheus lacks a unified abstraction to link a single performance gesture (a value from 0.0 to 1.0) to multiple underlying DSP properties with customizable scaling and inversion.

## ✅ Acceptance Criteria
- Must introduce a syntax to define a macro (e.g., `macro(param1: min..max, param2: min..max)`).
- Must support applying the macro as a standard pattern transformation, where the input value (0.0 to 1.0) drives all mapped parameters.
- Must support inverted mappings (e.g., `param: 1.0..0.0`).
- Must allow macros to be modulated by other patterns (e.g., `|> build_up(sine)` or `|> build_up(cc(1))`).
- Must resolve the macro parameter routing efficiently, performing the scaling math without blocking the real-time audio thread.

## 🚫 Out of Scope
- Non-linear scaling curves (e.g., exponential or logarithmic macro mappings). Phase 1 is strictly linear mapping between `min` and `max`.
- Mapping macros to structural changes (like muting tracks or changing tempo). Phase 1 focuses strictly on continuous DSP parameters.
