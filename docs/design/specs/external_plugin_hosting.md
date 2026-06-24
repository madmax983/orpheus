# 🔭 Vantage: Spec for External Plugin Hosting (VST3/AU)

## 👤 User Story
As an Electronic Musician, I want to host and sequence third-party VST3 and AudioUnit (AU) instruments directly within Orpheus, so that I can use my existing sound libraries (e.g., Serum, Kontakt) without leaving the live-coding environment.

## 💼 So What? (Business Value)
Orpheus's built-in subtractive synths and DSP tools are excellent for learning and experimentation, but professional users have already invested heavily in commercial plugins. By supporting external plugins, we eliminate the "toy" stigma and bridge the gap between algorithmic composition and professional sound design. This significantly increases user retention and adoption among working producers.

## 📊 Metric Definition
- **Success =** 99% of scheduled note events trigger the external plugin within 2ms of the intended cycle-relative time.
- **Success =** Parameter automation (e.g., filter sweeps) updates at block-rate without audio dropouts or xruns.

## 🔍 Gap Analysis
- **Current State:** Orpheus has a rudimentary `plugin_host` substrate that merely mocks VST3/AU descriptors and renders a basic sine wave, rather than dynamically loading shared libraries (`.vst3` / `.component`).
- **Standard Libs / Market:** SuperCollider uses `VSTPlugin`, Ableton Live natively hosts plugins. We must offer a similarly stable hosting environment, but integrated seamlessly with our `Pattern` and cycle-based time models.

## ✅ Acceptance Criteria
- Must dynamically discover and load valid 64-bit VST3 (Windows/macOS/Linux) and AU (macOS) plugins from standard OS search paths.
- Must map Orpheus `PluginNote` events to the corresponding plugin's MIDI/note input.
- Must map Orpheus `PluginParameterLane` events to the plugin's host automation system.
- Must render the plugin's stereo audio output into the Orpheus mixing bus.
- Must handle headless (no GUI) processing safely on the real-time audio thread without allocation or blocking.

## 🚫 Out of Scope
- Rendering native graphical user interfaces (GUIs) for plugins.
- Support for VST2 or CLAP formats (Phase 2).
- Plugin state chunk serialization/deserialization (Phase 2).
