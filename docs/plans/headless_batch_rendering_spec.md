# 🔭 Vantage: Spec for Headless Batch Rendering (CLI)

## 👤 User Story
"As a Sound Designer and Technical Audio Artist, I want to invoke Orpheus from the command line in headless mode to render an `.ode` script directly to a `.wav` file without launching the TUI or connecting to audio hardware, so that I can integrate Orpheus into automated build pipelines, game engine asset generation, and continuous integration workflows."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is tightly coupled to its interactive REPL and real-time audio hardware. While ideal for live performance, this is a massive bottleneck for production at scale. If an audio team wants to generate 50 variations of a procedural laser sound for a game, doing it manually via the TUI is prohibitive. Complexity is a cost; utility is revenue. By decoupling the engine from the UI and audio drivers, and providing a fast, offline batch rendering mode, Orpheus transforms from a localized performance instrument into a scriptable, scalable audio compiler. This opens the door to enterprise automation, sample pack generation, and CI/CD integration.

## 🎯 Metric Definition
- **Success** = Running `orpheus render synth_bass.ode --duration 16 --output bass.wav` completes offline rendering at >10x real-time speed on a standard CPU, resulting in a sample-accurate stereo WAV file, with zero UI components instantiated and zero reliance on system audio drivers (ALSA/CoreAudio).

## 🔍 Gap Analysis
- **Current State (Orpheus):** The application assumes a real-time context. Executing it requires an active audio driver and initializes terminal UI components or REPL loops.
- **Competitors (Csound, SuperCollider, Faust):** Csound and Faust have legendary headless compilation capabilities, allowing them to be embedded in automated pipelines seamlessly. SuperCollider's `sclang` can run headless but requires careful server management.
- **The Gap:** Orpheus lacks an alternative execution path that skips hardware initialization, parses the script, and routes the final DSP graph into a purely mathematical offline buffer that writes to disk.

## ✅ Acceptance Criteria
- Must introduce a `render` subcommand (or similar flags) to the main Orpheus CLI.
- Must execute the provided `.ode` script and evaluate its patterns into an offline DSP context instead of the real-time mixer.
- Must accept a `--duration` argument (in cycles or seconds) to define the length of the render for infinite patterns.
- Must render the output to a specified `.wav` file as fast as the CPU allows, completely bypassing real-time clock synchronization.
- Must run cleanly on CI servers (e.g., GitHub Actions) without requiring dummy audio drivers or terminal emulators.
- Must exit with a standard `0` code on success, or non-zero with clear diagnostic text on `stderr` if a parsing or runtime error occurs in the script.

## 🚫 Out of Scope
- Rendering individual stems or multi-track buses (Phase 1 focuses on the master stereo output).
- Background daemons, RPC servers, or hot-folder watching (Phase 1 is a simple, synchronous run-to-completion process).
- GUI progress bars (Phase 1 relies strictly on standard `stdout` logging for progress).
