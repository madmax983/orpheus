# 🔭 Vantage: Spec for Live Audio Input Sampling

## 👤 User Story
"As a Live Coder and Performer, I want to route live audio input (like a microphone or guitar) into my session, process it through Orpheus effects, and sample it on the fly, so that I can organically integrate acoustic instruments and live vocals with my sequenced digital patterns."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a closed loop: it sequences internal synthesizers and static, pre-loaded audio files. While powerful for electronic composition, it isolates the performer from the physical acoustic world. If a musician wants to loop a beatbox rhythm, process a guitar solo through Orpheus’s delay buses, or manipulate a live vocal take rhythmically, they have to leave the Orpheus ecosystem or rely on clunky external DAW routing. Complexity is a cost; utility is revenue. Bridging the gap between the digital sequencer and the analog performer expands Orpheus from a standalone drum-machine/synth into a comprehensive, interactive performance hub. It drastically increases the utility for hybrid electronic/acoustic acts.

## 🎯 Metric Definition
- **Success** = Live audio input is successfully captured from the system's default recording device with < 10ms of round-trip latency, can be processed by any standard DSP effect, and can be recorded into an in-memory buffer that can be immediately triggered by the pattern engine, all without introducing xruns or audio thread blocking.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation is limited to built-in synthesis and loading WAV files from disk at startup. There is no concept of a live audio input stream or dynamic buffer recording during a session.
- **Competitors (Sonic Pi, SuperCollider, Ableton Live):** Sonic Pi supports pass-through processing for live audio. SuperCollider has extensive real-time buffer recording and live audio routing. Ableton Live excels at live looping.
- **The Gap:** Orpheus lacks a real-time audio input ingestion layer and the ability to dynamically write to sample buffers while the transport is running, restricting performances to pre-meditated material.

## ✅ Acceptance Criteria
- Must introduce a command or function that routes the system audio input into the Orpheus mixer chain.
- Must allow live input to be processed by existing effect buses.
- Must introduce a mechanism to record a specific duration of live input (e.g., 1 cycle or 4 beats) into a named, in-memory sample buffer.
- Must allow the newly recorded sample buffer to be played back instantly via the standard pattern syntax.
- Must handle hardware sample rate mismatches or input buffer sizes gracefully, prioritizing the output audio thread's stability.

## 🚫 Out of Scope
- Multi-channel input routing (e.g., separate routing for 8 different ADAT inputs). Phase 1 will focus on the default stereo/mono input device.
- Pitch-detection or audio-to-MIDI conversion. Phase 1 is purely audio routing and basic sampling.
- Saving the dynamically recorded buffers to disk as WAV files automatically. Phase 1 keeps them in-memory for the duration of the session.
