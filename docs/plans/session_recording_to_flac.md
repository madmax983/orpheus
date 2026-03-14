# 🔭 Vantage: Spec for Session Recording to FLAC

## 👤 User Story
"As a Live Coder, I want to record my live session directly to a high-quality audio file format like FLAC, so that I can capture my improvised performances for later listening, publishing, or further production without relying on external routing or recording software."

## ❓ The "So What?" (Business Problem)
Live coding is inherently ephemeral. A session is a performance that evolves over time. Currently, if a user wants to record what they hear from Orpheus, they have to use external loopback tools like OBS, Loopback, or Jack to capture the system audio, which adds configuration complexity and potential latency. By integrating high-quality, lossless recording (FLAC) directly into the engine, we provide an out-of-the-box solution for capturing the final output mix. This reduces the friction between "playing" and "publishing", increasing the standalone utility of Orpheus as a complete performance tool. Complexity is a cost; utility is revenue. Built-in recording is a massive utility multiplier for performing artists.

## 🎯 Definition of Success
- **Success** = 100% of the main stereo output mix is written directly to a `.flac` file on disk with zero dropped frames, zero audio dropouts/xruns on the real-time audio thread, and minimal CPU overhead, seamlessly initiated via a REPL/TUI command.

## ✅ Acceptance Criteria
- Must provide a command in the REPL/TUI (e.g., `:record start [filename]` and `:record stop`) to start and stop recording.
- Must capture the exact stereo output mix being sent to the system audio device.
- Must encode the audio to FLAC format to ensure lossless quality and reasonable file sizes.
- Must handle file I/O and encoding on a separate background thread to ensure absolutely zero locks or allocations occur on the high-priority real-time audio rendering thread.
- Must gracefully stop recording and finalize the FLAC file upon application exit or crash.

## 🚫 Out of Scope
- Multi-track recording (e.g., stem export per pattern/layer). Phase 1 is strictly the master stereo mix.
- Real-time MP3 or OGG Vorbis encoding. FLAC is chosen for lossless quality and CPU efficiency during live capture.
- Scheduled recording based on cycle counts (e.g., "record the next 16 cycles"). Phase 1 is manual start/stop only.
- Automatic uploading to cloud services or streaming platforms.
