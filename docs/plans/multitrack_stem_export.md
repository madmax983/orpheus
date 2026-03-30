# 🔭 Vantage: Spec for Multi-track Stem Export

## 👤 User Story
"As a Producer and Live Coder, I want to export my Orpheus composition as separate, multi-track audio stems, so that I can import them into a traditional DAW (like Ableton Live or Logic Pro) for final mixing, mastering, and detailed arrangement."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus can record the master stereo output to a single file. While useful for capturing a live jam, it creates a massive bottleneck for professional production. If a user wants to apply a specific VST compressor to the drum bus or EQ the bassline precisely, they cannot do it once the audio is summed to stereo. They are forced to solo each part and bounce them out in real-time, one by one—a tedious and error-prone process. Complexity is a cost; utility is revenue. Bridging the gap between the live coding environment (where ideas and structures are born) and the traditional DAW environment (where final polish happens) multiplies Orpheus's utility. It positions Orpheus not just as a standalone toy, but as a powerful ideation engine within a professional studio workflow.

## 🎯 Metric Definition
- **Success** = Users can execute a single REPL/TUI command (e.g., `:export stems 16`) to offline-render a specified number of cycles of their current session. The system must output individual, sample-accurate `.wav` files for every active pattern layer (and bus) in the session, completing the render faster than real-time playback speed without any audio dropouts in the live session if executed concurrently.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Only real-time, master stereo recording is planned/available. No offline rendering or stem separation exists.
- **Competitors (Renoise, Ableton Live):** All modern DAWs have robust "Export Stems" or "Render Individual Tracks" features. Tracker software like Renoise allows rendering patterns to sample-accurate stems.
- **The Gap:** Orpheus lacks an offline render mode and a mechanism to intercept and write the audio buffers of individual pattern tracks/buses before they hit the master mixing bus.

## ✅ Acceptance Criteria
- Must introduce a command (e.g., `:export stems [cycles]`) to trigger an offline render of the current session state.
- Must render the specified duration (in cycles) exactly, utilizing an offline DSP context so it renders as fast as the CPU allows, not constrained to real-time.
- Must output a separate `.wav` file for each active pattern layer (e.g., `drums.wav`, `bass.wav`, `synth.wav`).
- Must optionally output separate stems for shared effect buses (e.g., `reverb_bus.wav`).
- Must create a new timestamped directory in an `exports/` folder to contain the stems, preventing file overwrites.
- Must not interrupt or glitch the real-time audio thread if the user triggers the export while the transport is running.

## 🚫 Out of Scope
- Rendering MIDI stems. Phase 1 is strictly audio WAV exports.
- Automatic integration with specific DAWs (e.g., generating an Ableton `.als` project file).
- Rendering individual slices/hits within a pattern (e.g., exploding a drum break into 16 separate files). Stems are per-layer over the specified duration.
