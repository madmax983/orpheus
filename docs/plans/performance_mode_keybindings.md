# 🔭 Vantage: Spec for Performance-Mode Keybindings

## 👤 User Story
"As a Live Coder, I want a dedicated 'performance mode' with global keybindings for muting, soloing, and triggering scene changes, so that I can rapidly manipulate the structure of my composition during a live set without having to manually type out and evaluate code for every single arrangement change."

## ❓ The "So What?" (Business Problem)
Live coding balances two cognitive modes: "composition" (writing the structure) and "performance" (manipulating the structure in real-time). Currently, Orpheus is heavily weighted towards composition. If a user wants to quickly drop the bass layer and bring it back 4 bars later, they must navigate to the code, comment out the line or change the gain to 0, and re-evaluate the block. This introduces latency between musical intent and execution, killing the "flow" of a live performance and increasing the risk of syntax errors under pressure.

By introducing a dedicated performance mode with instantaneous keybindings for high-level mixing operations, we bridge the gap between "programming language" and "live instrument." Complexity is a cost; utility is a revenue. Providing immediate tactile control over running patterns is a massive utility multiplier for performing artists, drastically lowering the barrier to entry for executing complex, dynamic sets.

## 📏 Metric Definition
- **Success** = Keybindings (e.g., mute/solo toggles, section switches) register and apply to the audio output within <16ms (1 frame at 60fps) of the keypress, with zero audio dropouts or xruns on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** All arrangement changes require manual code editing and re-evaluation. No immediate tactile feedback.
- **Competitors (TidalCycles, Strudel):** Rely heavily on code manipulation. Some users build custom MIDI controllers or OSC bridges to achieve tactile control, but this requires external setup.
- **DAWs (Ableton Live):** Excel at this with "Session View" clip launching and track muting, usually mapped to MIDI controllers.
- **The Gap:** Orpheus needs a built-in, keyboard-native way to achieve DAW-like session manipulation without requiring external MIDI hardware or OSC routing, keeping the entire workflow within the terminal.

## ✅ Acceptance Criteria
- Must introduce a togglable "Performance Mode" in the ratatui TUI, distinct from the standard "Edit Mode".
- Must provide global, customizable keybindings for muting and soloing specific active pattern layers (e.g., keys 1-9 correspond to layers in the current stack).
- Must provide keybindings for rapid section switching (e.g., jumping between predefined `verse` and `chorus` blocks if `seq_sections` is used).
- Must visually indicate the current mute/solo state and active section clearly in the TUI.
- Must queue all performance operations (mutes, solos, section changes) to apply exactly at the next cycle boundary to ensure musical synchronization, preventing jarring, off-beat changes.

## 🚫 Out of Scope
- MIDI controller mapping. Phase 1 is strictly for QWERTY keyboard bindings within the terminal.
- Live recording of keybinding macros.
- Granular parameter tweaking via keybindings (e.g., smoothly turning a filter knob with the arrow keys). Performance mode is for macro-structural changes (mute, solo, section jumps).
