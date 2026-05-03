# 🔭 Vantage: Spec for Audio Recording and Export

## 👤 User Story
As a Composer, I want to record and export my live coding sessions and composed patterns to high-quality audio files (WAV/FLAC) so that I can publish my music, share it with others, or import it into a DAW for further mixing and mastering.

## 🎯 The "So What?" (Business Problem)
A live-coding environment without a way to persist the output as an audio artifact is limited to transient performances. Enabling export turns Orpheus from a "live jam tool" into a viable compositional environment, allowing users to build a portfolio of work and integrate Orpheus into standard audio production pipelines.

## ✅ Acceptance Criteria
- **Quality Options:** Must support exporting uncompressed audio in standard broadcast formats (at least 16-bit/44.1kHz and 24-bit/48kHz WAV, and FLAC).
- **Session Recording:** Users must be able to start and stop recording of the live master output without interrupting the live playback or causing audio dropouts.
- **Offline Rendering:** Users must be able to specify a duration (e.g., in cycles or time) and trigger a faster-than-real-time (offline) render of a composition directly to disk.
- **Success Metric:** Offline rendering of a 3-minute composition should take less than 10 seconds on average hardware. Live recording must maintain 0 dropped frames during the write process.

## 🚫 Out of Scope
- **Stem Export:** Exporting individual patterns or tracks to separate audio files (multitrack export) is deferred to a future phase. Only the master bus output is recorded.
- **MP3/Ogg Export:** Lossy compression formats are out of scope for this initial feature to focus on high-fidelity archival.
- **Built-in Trimming/Editing:** Basic audio editing capabilities (fade in/out, trimming silences) post-record are not included; users should rely on external DAWs.
