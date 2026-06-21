# VST3/AU Plugin Hosting Spec

## 👤 User Story
"As a Composer, I want to load VST3 and AU plugins into Orpheus, so that I can use professional-grade software instruments alongside the built-in DSP engine."

## ❓ So What? (Business Problem)
Currently, Orpheus relies solely on its internal synth engines. Many musicians rely on complex third-party VSTs (like Serum, Kontakt) that took years to develop. Rebuilding these in Faust/Orpheus is impractical. By supporting VST3/AU hosting, we allow users to instantly leverage industry-standard sounds, drastically increasing Orpheus's appeal as a professional sequencing tool rather than just an educational toy.

## 📊 Metric Definition
- **Success** = Plugin instantiation takes < 500ms.
- **Success** = No audio dropouts (xruns) occur when modulating 10+ plugin parameters simultaneously on a modern CPU.

## ✅ Acceptance Criteria
- Must be able to load VST3 and AU plugins via a simple language construct (e.g., `vst("Serum")`).
- Must correctly route internal `notes` events to the plugin.
- Must support parameter automation via `p("ParamName", value)` mapping to standard plugin parameter IDs.
- Must support discovering available plugins via standard OS search paths.
- Must not crash the host if a plugin faults (isolation/graceful error handling if possible).

## 🚫 Out of Scope
- Plugin GUI rendering (Headless only for now).
- VST2 format support (deprecated by Steinberg).
- Advanced MIDI routing to plugins beyond standard note on/off and parameter changes.
