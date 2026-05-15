# 🔭 Vantage: Spec for VST3/AU Plugin Hosting

## 👤 User Story
"As a Musician, I want to host external VST3/AU plugins natively in Orpheus, so that I can use my existing library of professional software synthesizers and effects within my live coded patterns."

## ❓ So What? (Business Problem)
Currently, Orpheus relies exclusively on its internal custom DSP engine. While this is great for exploring new synthesis methods, many professional musicians have heavily invested in high-quality third-party plugins (like Serum, Kontakt, or FabFilter). Without the ability to host these plugins natively, Orpheus is seen as an isolated toy rather than a professional production tool. By supporting VST3 and AudioUnit plugin hosting directly within the pattern engine, we unlock the entire modern software instrument ecosystem, making Orpheus a viable, professional hub for live performance and composition.

## 📊 Metric Definition
- **Success** = Plugin load time is under 500ms on average.
- **Success** = Audio processing overhead per plugin instance is comparable to mainstream DAWs (e.g., < 5% CPU usage for a standard synth).
- **Success** = 0 audio dropouts or xruns when automating parameters via Orpheus patterns.

## ✅ Acceptance Criteria
- Must be able to instantiate VST3 (Windows/macOS/Linux) and AudioUnit (macOS only) plugins.
- Must provide a syntax in the REPL (e.g., `vst("Serum")`) to target plugin instruments with note patterns.
- Must allow mapping of Orpheus pattern control signals to specific plugin parameters (host automation).
- Must process audio synchronously with the Orpheus clock, supporting sample-accurate automation.
- Must cleanly handle plugin crashes without taking down the entire Orpheus application.

## 🚫 Out of Scope
- Support for legacy VST2 plugins.
- Bridging 32-bit plugins to 64-bit.
- Full custom graphical UI rendering for plugins inside the terminal (plugins will load in floating external windows if UI is requested, but headless by default).
- Support for CLAP plugin format (Phase 2).