# 🔭 Vantage: Spec for Auto-Slicing and Transient Detection

## 👤 User Story
"As a Beatmaker and Live Coder, I want Orpheus to automatically detect transients in drum loops and long audio files so that I can easily sequence, rearrange, and rhythmically chop samples using a simple index, without having to manually calculate start and end time slices."

## ❓ The "So What?" (Business Problem)
Currently, users can slice samples in Orpheus using `slice(start, end)` or `slice_idx(index, segments)`. This approach works reasonably well for perfectly chopped, mathematically even breaks (like a perfectly quantized 16-step amen break). However, real-world audio, organic drum loops, and vocal phrases rarely have perfectly spaced transients. If a user wants to chop an unquantized drum loop, they have to manually guess the precise `start` and `end` fraction for every single hit, which is extremely tedious and breaks the flow of live coding. By introducing transient detection and auto-slicing, we eliminate this friction. The platform goes from requiring pre-processed, mathematically perfect sample packs to being able to instantly ingest and musicalize arbitrary audio. Complexity is a cost; utility is revenue. This feature drastically lowers the barrier to entry for working with raw audio and makes Orpheus a far more potent sampler.

## 🎯 Metric Definition
- **Success** = Orpheus can analyze a loaded WAV file offline (during the initial load or a hot-reload), detect transient onset points with >90% accuracy for percussive material, and expose these regions to the pattern language via a simple `chop()` or `onset()` command, allowing users to sequence the detected slices with zero audio dropouts or real-time CPU spikes.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Slicing is entirely manual or based on equal-length mathematical subdivisions (`slice_idx(i, n)`). There is no awareness of the actual audio content.
- **Competitors (Ableton Live, Renoise, TidalCycles):** Ableton Live is famous for its "Slice to New MIDI Track" feature based on transients. Renoise has robust auto-slicing. TidalCycles has `chop` (mathematical) but often relies on pre-sliced sample directories for organic chopping, though SuperDirt can be configured for onset detection.
- **The Gap:** Orpheus needs an offline analysis step during sample loading that identifies onset points and automatically populates the `regions` metadata in the `samples.ron` manifest (or an in-memory equivalent), allowing users to immediately trigger those slices.

## ✅ Acceptance Criteria
- Must introduce a fast transient detection algorithm (e.g., based on spectral flux or energy difference) that runs on the REPL/loader thread when a sample is ingested, avoiding the real-time audio thread.
- Must provide a way to trigger auto-slicing for a file, perhaps via a suffix in the loader or a command like `:analyze "breaks/amen.wav"`.
- Must expose the detected slices to the pattern language, e.g., via a new function `onset(index)` that plays the slice from the `index`th transient to the next.
- Must support applying standard transformations (pitch, gain, filter) to these dynamically generated slices.
- Must complete the analysis of a standard 4-second drum break in under 100ms.

## 🚫 Out of Scope
- Real-time transient detection on live audio input streams. Phase 1 is strictly for static audio files loaded from disk.
- Complex beat-warping or time-stretching to conform the sliced loop to a new tempo. Phase 1 simply provides the slice boundaries; the user sequences them.
- Manual adjustment of transient markers via a graphical UI. Phase 1 relies on sensible algorithm defaults or global threshold parameters.
