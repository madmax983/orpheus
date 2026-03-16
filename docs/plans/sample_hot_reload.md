# 🔭 Vantage: Spec for Sample Hot-Reload and Library Scanning

## 👤 User Story
"As a Live Coder, I want to drop new sample files (.wav) into a watched directory and have Orpheus immediately scan, index, and load them into memory, so that I can dynamically evolve my sound palette during a live set without needing to restart the engine or interrupt playback."

## ❓ The "So What?" (Business Problem)
Live coding relies heavily on uninterrupted flow. When an artist is performing or exploring, realizing they lack the right hi-hat or kick drum can be a showstopper. If adding a new sample requires stopping the audio engine, editing a configuration file, or restarting the entire session, the "live" aspect of the performance is broken. Adding real-time sample hot-reloading and directory scanning transforms Orpheus from a static playback tool into a truly dynamic instrument. Complexity is a cost, but restricting creativity is a critical flaw for an expressive tool. By managing hot-reloading behind the scenes, we vastly increase the utility and lifespan of an active Orpheus session.

## 🎯 Definition of Success
- **Success** = 100% of valid audio files added to or modified in a designated "sample library" folder are automatically detected and loaded into the `SampleBank` within 500ms of the file system event, with absolutely zero locks, allocations, or audio dropouts on the high-priority real-time audio thread, making the new sample immediately available to the running REPL/pattern evaluator.

## ✅ Acceptance Criteria
- Must implement a background file watcher (e.g., using `notify`) that monitors a designated sample directory (and its subdirectories) for file creation, modification, or deletion events.
- Must only attempt to parse and load supported audio formats (e.g., `.wav`, `.flac`).
- Must parse the file paths into logical token names (e.g., `samples/drums/kick.wav` becomes `drums/kick`).
- Must decode the audio in a background thread and seamlessly update the active `SampleBank` using lock-free data structures (like `ArcSwap` or similar RCU mechanisms) so the audio thread never blocks.
- Must gracefully handle missing or corrupted files without crashing the application (e.g., logging an error but continuing to play).
- Must automatically purge deleted samples from the `SampleBank` memory.

## 🚫 Out of Scope
- Network-based sample loading or cloud synchronization. Phase 1 focuses strictly on the local file system.
- Pitch detection, tempo analysis, or automatic slicing of long samples. Phase 1 treats all files as one-shot triggers.
- Support for complex metadata tags (ID3, etc.) beyond file path inference.
- Non-audio file watching (like hot-reloading `.ode` scripts). That is a separate feature.