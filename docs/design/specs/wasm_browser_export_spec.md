# 🔭 Vantage: Spec for WebAssembly (WASM) Browser Playback Export

## 👤 User Story
"As a Composer, I want to export my Orpheus composition as a standalone WebAssembly module, so that I can share interactive, browser-playable versions of my music online without requiring listeners to install any software or command-line tools."

## ❓ The "So What?" (Business Problem)
Currently, sharing an Orpheus track requires either rendering it to static audio (WAV/FLAC) or sharing the `.ode` source code. Static audio destroys the generative, algorithmic nature of the composition, turning it into a passive artifact. Conversely, sharing source code creates a massive barrier to entry, as the listener must install Rust, build the engine, and run it locally. WebAssembly export bridges this gap, allowing Orpheus to act as a distribution format as much as an authoring tool. This expands the potential audience from "other Orpheus developers" to "anyone with a web browser." It turns algorithmic music into shareable, interactive software artifacts. Complexity is a cost; utility is revenue. This drastically lowers the barrier to consumption and increases the viral potential of Orpheus compositions.

## 🎯 Metric Definition
- **Success** = A standard `.ode` file can be compiled via a CLI command (e.g., `orpheus export --target web`) resulting in a compact `<10MB` `.wasm` file and a minimal `index.html` wrapper that plays the generative composition stably in a modern web browser (Chrome, Firefox, Safari) at 44.1kHz without audio dropouts or xruns.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The system relies entirely on native OS audio APIs (like CPAL) and terminal interfaces (Ratatui). It is strictly a local desktop application with no web distribution capabilities.
- **Competitors (Strudel, Csound, Faust):** Strudel (the JavaScript port of TidalCycles) runs natively in the browser and is extremely easy to share via URLs. Csound and Faust both feature robust WebAssembly compilation targets, allowing their DSP to run seamlessly on the web.
- **The Gap:** Orpheus lacks a web-compatible compilation pipeline. The core DSP and Pattern engines are fundamentally agnostic or mathematically pure, but the audio host and runtime layer currently assume a native, locally-installed environment.

## ✅ Acceptance Criteria
- Must provide a CLI subcommand (e.g., `export-web`) to compile a given `.ode` script and its dependencies into a WASM module.
- Must abstract the audio backend so the DSP engine can be driven by standard Web Audio API constructs instead of native desktop APIs.
- Must handle sample and asset loading asynchronously via standard web network requests instead of local filesystem reads.
- Must provide a basic HTML/JS template that instantiates the WASM module, handles the browser's audio autoplay policy (e.g., waiting for a user click), and provides a basic Play/Stop toggle.

## 🚫 Out of Scope
- A full in-browser IDE, editor, or REPL (Phase 1 is strictly for playback of pre-composed `.ode` files).
- Real-time collaborative editing over WebSockets.
- WebMIDI integration for external controller input (Phase 1 is strictly generative playback).
