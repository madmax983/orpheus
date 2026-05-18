# 🔭 Vantage: Spec for Dynamic Macro Control Mapping

## 👤 User Story
"As a Live Coder and Performer, I want to create custom named 'macro' variables (e.g., `tension`, `energy`, `buildup`) and map them to multiple underlying DSP parameters simultaneously, so that I can control complex, multi-layered sonic transitions during a live set by tweaking a single high-level value."

## ❓ The "So What?" (Business Problem)
During a live performance, making sweeping musical transitions—such as moving from a sparse, dark verse into a bright, chaotic drop—requires changing dozens of parameters at once. Opening the filter cutoff, increasing the reverb decay, speeding up the delay rate, and raising the distortion drive. In Orpheus currently, a performer must type out and update each of these values individually across multiple lines of code, or rely on complex, brittle LFOs.

This introduces a massive cognitive load and makes spontaneous, performative builds nearly impossible. Complexity is a cost; utility is revenue. By allowing users to define a single "macro" variable that mathematically drives multiple underlying parameters, we abstract the technical complexity of the DSP graph into high-level musical intent. This transforms Orpheus from a static pattern sequencer into a highly playable performance instrument.

## 🎯 Metric Definition
- **Success** = Users can define a macro variable (e.g., `~energy = 0.5`), bind it to multiple pattern or DSP parameters (e.g., `cutoff = ~energy * 10000`, `reverb_mix = ~energy`), and update that macro value in the REPL or TUI. The underlying audio engine must instantly reflect the changes across all mapped parameters within <10ms, without requiring the re-evaluation of the entire sequence block or causing audio dropouts.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Parameters are hardcoded values or driven by internal pattern functions (like `sine`). There is no mechanism to link multiple distinct pattern parameters to a single, easily mutable global variable.
- **Competitors (Ableton Live, Bitwig, SuperCollider):** Ableton Live’s "Audio Effect Racks" have 8 macro knobs that can be assigned to any parameter with custom ranges. Bitwig’s modulators work similarly. SuperCollider uses global `Bus.control` or environment variables to achieve this.
- **The Gap:** Orpheus needs a globally accessible, real-time mutable control bus system that the pattern evaluator can reference natively, updating the DSP graph asynchronously without stopping the transport or completely rewriting the AST.

## ✅ Acceptance Criteria
- Must introduce a syntax for defining global, mutable control macros in the REPL/TUI (e.g., `:macro set energy 0.8`).
- Must allow pattern parameters and DSP effect arguments to read from these macros (e.g., `cutoff=~energy` or `macro("energy")`).
- Must support basic mathematical scaling at the binding site (e.g., mapping a `0.0` to `1.0` macro to a `200` to `8000` Hz filter cutoff).
- Must ensure that when a macro is updated in the UI/REPL, the DSP engine smoothly ramps the corresponding parameters to their new values to avoid audio clicking or zippering.
- Must provide a visual summary of active macros and their current values in the ratatui TUI.

## 🚫 Out of Scope
- MIDI hardware mapping to these macros. Phase 1 focuses on creating the internal software architecture and REPL/TUI interface. Hardware control is a separate initiative.
- Complex non-linear transfer functions for the macro mapping (e.g., custom curve mapping). Phase 1 supports linear scaling or direct value pass-through.
- LFO modulation of macros. Phase 1 targets manual, user-driven updates.
