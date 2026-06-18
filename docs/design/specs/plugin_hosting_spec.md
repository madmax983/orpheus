# 🔭 Vantage: Spec for VST3/AU Plugin Hosting

## 👤 User Story
"As a Composer, I want to host my existing VST3 and AudioUnit plugin instruments within Orpheus, so that I can use my favorite synth libraries alongside Orpheus's pattern sequencing language."

## ❓ The "So What?"
**What business problem does this solve?**
Orpheus currently relies on internal synthesizers and basic sample playback. Professional music production heavily relies on third-party plugins (like Serum, Kontakt, or Arturia). Without the ability to host external instruments, Orpheus remains an isolated toy rather than a serious composition tool that integrates into existing producer workflows. Adding plugin hosting unlocks the entire ecosystem of commercial and open-source instruments, massively expanding Orpheus's utility.

## 📊 Metric Definition
- **Success =** A VST3 plugin can be loaded via the Orpheus language (e.g., `vst("Serum")`), sent scheduled MIDI note events over a cycle, and render stereo audio without dropping frames or exceeding 2ms latency on the audio thread.
- **Success =** Host automation parameters can be modulated using Orpheus patterns.

## 🔍 Gap Analysis
Currently, producers write patterns in DAWs (Ableton, FL Studio) using a piano roll, which limits complex algorithmic or generative composition. They use external plugins. Orpheus has the generative composition language but lacks the external plugins. Bridging this gap brings code-based composition to professional sound sources. Other live-coding environments (like TidalCycles) rely on SuperCollider for sound, which has a high learning curve for plugin integration. Native VST3/AU hosting directly in Orpheus provides a unique, frictionless "code-to-plugin" experience.

## ✅ Acceptance Criteria
- Must support loading VST3 plugins on all platforms.
- Must support loading AudioUnit plugins on macOS.
- Must resolve plugin names against standard OS search paths.
- Must allow scheduling MIDI note events (Note On/Off with velocity) to the plugin.
- Must allow scheduling continuous parameter automation lanes from pattern controls.
- Must perform plugin audio processing directly on the real-time render thread.
- Must handle plugin instantiation safely without blocking the audio thread.
- Must provide a headless testing mode.

## 🚫 Out of Scope
- GUI rendering for plugins (Headless only for now).
- VST2 format support (Deprecated format).
- Plugin effect hosting on audio buses (Instruments only for Phase 1).
- Real-time parameter discovery and reporting back to the REPL.
