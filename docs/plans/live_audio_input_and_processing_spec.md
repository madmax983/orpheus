# 🔭 Vantage: Spec for Live Audio Input and Processing

## 👤 User Story
"As a Live Coder and Performer, I want to route live audio from my hardware instruments (e.g., microphones, guitars, hardware synthesizers) directly into Orpheus, so that I can process them in real-time using pattern-driven effects and integrate acoustic sounds seamlessly into my code-driven compositions."

## ❓ The "So What?" (Business Problem)
Music is rarely created entirely inside the box. Many musicians rely on their voices, acoustic instruments, or favorite analog gear during a performance. Currently, Orpheus acts exclusively as a sound generator, locking out external acoustic input. This forces performers to run separate mixer hardware or complex DAW routing alongside Orpheus, breaking the integrated flow and preventing the algorithmic manipulation of live sounds. Complexity is a cost; utility is revenue. By supporting live audio input and real-time processing, Orpheus becomes a unified performance hub and algorithmic multi-effects processor. This drastically expands the platform's addressable market to include vocalists, instrumentalists, and hardware-focused producers.

## 🎯 Metric Definition
- **Success** = Orpheus can discover and capture audio from active OS audio input devices, route the incoming stereo stream into the exact-rational scheduling engine with <5ms of latency, apply dynamic pattern-based effects to the live signal, and output it alongside internal synthesized tracks without causing xruns or UI freezes.

## 🔍 Gap Analysis
- **Current State (Orpheus):** A closed audio output ecosystem. Sound is only generated internally via built-in DSP algorithms or pre-loaded WAV files. There is no concept of a live audio input stream.
- **Competitors (Ableton Live, SuperCollider, TidalCycles):** Ableton Live excels at live input routing and processing. SuperCollider provides `SoundIn` for low-latency mic processing. TidalCycles, via SuperDirt, can easily grab hardware inputs and apply code-driven effects.
- **The Gap:** Orpheus needs a cross-platform mechanism to open a real-time capture stream from the user's audio interface, treat this continuous stream as a manipulable sound source within the language layer, and expose it to the mixer infrastructure.

## ✅ Acceptance Criteria
- Must introduce a REPL/TUI command to list available audio input devices (e.g., `:audio in list`) and connect to a specific interface.
- Must provide a language primitive (e.g., `sound_in(1)` or `live_audio()`) that acts as a continuous audio source in the pattern language.
- Must allow applying standard pattern-driven effects to the live audio source (e.g., `live_audio() |> delay(0.5) |> lpf(every(4, 400))`).
- Must operate securely on the real-time audio thread without allocating memory or holding locks during the capture or processing loop.
- Must handle input device disconnections gracefully without crashing the active session.

## 🚫 Out of Scope
- Advanced input analysis like real-time pitch detection (Audio-to-MIDI) or onset detection for tempo extraction. Phase 1 is strictly about capturing the audio stream and processing it with effects.
- Multichannel input routing beyond standard stereo pairs (e.g., capturing a 16-channel drum mic setup simultaneously is out of scope for Phase 1).
