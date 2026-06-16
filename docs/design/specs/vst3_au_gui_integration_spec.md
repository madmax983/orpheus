# 🔭 Vantage: Spec for VST3 and AU Plugin GUI Integration

## 👤 User Story
"As a Music Producer and Live Coder, I want to open and interact with the native graphical user interface (GUI) of hosted VST3 and AU plugins within Orpheus, so that I can visually design sounds, browse presets, and tweak complex parameters without manually mapping every single control to code."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus supports headless hosting of VST3 and AU plugins, which is great for triggering them via code and modulating a few mapped parameters. However, modern plugins (like Serum, Massive, or complex multi-effects) have hundreds of parameters, intricate modulation matrices, and visual preset browsers. Forcing users to design sounds completely blind or guess parameter indices is an enormous friction point. By lacking GUI support, we significantly reduce the value of the plugin integration. Providing a native GUI window bridges the gap between algorithmic sequencing and visual sound design, drastically increasing Orpheus's utility as a comprehensive production environment and encouraging adoption among professional producers who rely on visual feedback.

## 🎯 Metric Definition
- **Success** = Users can open the native GUI window of a loaded VST3/AU plugin via a command (e.g., `:vst gui "Serum"`), interact with it smoothly at 60fps, and have parameter changes immediately reflect in the DSP engine, without causing UI freezes in the main Orpheus TUI or blocking the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Plugins load headlessly. Users can send MIDI and route audio, but they cannot see the plugin's UI. Sound design must happen externally, and presets must be saved/loaded via blind state files.
- **Competitors (Ableton Live, Bitwig, Renoise):** All standard DAWs and trackers provide seamless, floating native GUI windows for plugins alongside their sequencing interfaces.
- **The Gap:** Orpheus needs a windowing abstraction (e.g., using `winit` or similar) capable of embedding the plugin's native view (NSView on macOS, HWND on Windows, X11/Wayland on Linux) on a non-audio, non-TUI UI thread, and safely synchronizing parameter state back and forth.

## ✅ Acceptance Criteria
- Must introduce a command to open the GUI of a specifically loaded plugin track (e.g., `:vst open "Synth1"`).
- Must open a floating desktop window displaying the plugin's native UI.
- Must capture mouse and keyboard input within the plugin window correctly.
- Must synchronize parameter changes made in the GUI back to Orpheus's internal state so they can be saved with the session.
- Must run the windowing event loop on a dedicated thread to ensure the TUI and audio threads are completely unaffected.
- Must support standard resizing if the plugin's UI is resizable.

## 🚫 Out of Scope
- Full state saving/loading of the plugin's internal preset format (e.g., FXB/FXP) in this phase. The focus is strictly on opening the window and basic parameter sync.
- Embedding the plugin GUI *inside* the terminal TUI (impossible). It must be a floating desktop window.
