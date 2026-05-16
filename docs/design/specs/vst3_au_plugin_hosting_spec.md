# 🔭 Vantage: Spec for VST3 and AU Plugin Hosting

## 👤 User Story
"As a Music Producer and Live Coder, I want to load standard VST3 and AudioUnit (AU) plugins directly within Orpheus, so that I can use my existing library of high-quality software synthesizers and professional audio effects as part of my code-driven compositions."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus relies solely on its internal DSP engine and sample playback for sound generation and processing. While powerful, users have invested significant money and time into third-party VST/AU plugins (like Serum, Kontakt, or FabFilter). Forcing users to recreate sounds from scratch or abandon their favorite tools creates a massive barrier to adoption. If Orpheus cannot host these plugins, it remains an isolated environment rather than a professional studio centerpiece. Complexity is a cost; utility is revenue. Bridging the gap between code-based sequencing and industry-standard plugin formats exponentially multiplies Orpheus's utility. It positions Orpheus as a programmable DAW, drastically increasing its appeal to professional electronic musicians.

## 🎯 Metric Definition
- **Success** = Users can load a 64-bit VST3 or AU plugin via a language command (e.g., `vst("Serum")`), route pattern note events to its MIDI input with <2ms jitter, and route its stereo audio output back into the Orpheus mixer chain, with the plugin running stably on the real-time audio thread without causing xruns or UI freezes.

## 🔍 Gap Analysis
- **Current State (Orpheus):** A closed audio ecosystem. Sound can only be generated or modified using built-in Rust DSP algorithms or raw WAV samples. No external plugin support exists.
- **Competitors (Ableton Live, Renoise, Bitwig, TidalCycles):** All DAWs (Ableton, Bitwig) and modern trackers (Renoise) host VST/AU natively. TidalCycles relies on SuperCollider (SuperDirt), which has experimental or clunky VST support via third-party wrappers, often pushing users toward external MIDI hardware.
- **The Gap:** Orpheus needs a cross-platform plugin hosting layer capable of instantiating VST3 and AU bundles, exposing their parameters to the language runtime, and managing their audio/MIDI buffers within the existing exact-rational scheduling engine.

## ✅ Acceptance Criteria
- Must introduce a command or language primitive to instantiate a VST3 or AU plugin from standard OS paths (e.g., `/Library/Audio/Plug-Ins/VST3` or `C:\Program Files\Common Files\VST3`).
- Must provide a way to send Note On/Off events from `Pattern<Note>` directly to the plugin's internal MIDI/event queue.
- Must capture the stereo audio output of the plugin and route it into the standard Orpheus mixer infrastructure (buses, tracks).
- Must allow exposing and modulating a subset of the plugin's automatable parameters via the pattern language (e.g., `|> p("Cutoff", slow(4, sine))`).
- Must run the plugin's processing loop on the real-time audio thread securely, without allocating memory or holding locks during the `process` call.

## 🚫 Out of Scope
- VST2 Support. VST2 is officially deprecated by Steinberg and is legacy. Phase 1 targets VST3 and AU only.
- Hosting the plugin's graphical user interface (GUI). Phase 1 is strictly headless DSP hosting; users must rely on macro parameter mapping rather than opening a floating window.
- Complex routing like sidechaining inputs into the plugin or supporting surround sound (Phase 1 is strictly stereo out).
