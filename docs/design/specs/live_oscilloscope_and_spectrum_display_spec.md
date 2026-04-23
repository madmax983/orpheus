# 🔭 Vantage: Spec for Live Oscilloscope and Spectrum Display

## 👤 User Story
"As a Live Coder, I want a real-time oscilloscope and spectrum analyzer in my terminal UI, so that I can visually confirm that my audio engine is outputting sound, check for clipping, and understand the frequency content of my mix without relying solely on my ears or external metering tools."

## ❓ The "So What?" (Business Problem)
Audio programming is uniquely opaque; if you make a mistake, the result is often just silence or a harsh blast of noise. Currently, an Orpheus user has no visual confirmation that their code is actually generating a signal or what the character of that signal is until it hits their speakers. This opacity creates a frustrating debug loop ("Is the filter fully closed? Is the volume at 0? Is the engine stalled?"). By adding built-in, low-latency visual metering (oscilloscope for amplitude/waveform, spectrum for frequency), we close the feedback loop between code execution and audio output. Complexity is a cost; utility is revenue. Instant visual feedback is a massive utility multiplier that reduces cognitive load, builds confidence during a live performance, and makes the system feel alive.

## 📏 Metric Definition
- **Success** = Real-time audio data is extracted from the render thread and visualized in the ratatui interface as a braille/block character oscilloscope and spectrum analyzer, running at a smooth 60fps refresh rate (<16ms TUI latency), with absolutely zero locks, allocations, or audio dropouts introduced on the critical real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The TUI shows code and text feedback, but there is zero visualization of the actual audio signal being produced. If it's silent, you have to guess why.
- **Competitors (Sonic Pi, SuperCollider):** Both offer built-in oscilloscopes and spectrum analyzers. Sonic Pi's visualizers are a core part of its accessible, instant-feedback philosophy.
- **The Gap:** Orpheus needs a low-overhead, terminal-native way to display high-bandwidth audio information without breaking its strict real-time constraints.

## ✅ Acceptance Criteria
- Must introduce a new pane/widget in the ratatui interface dedicated to visual metering.
- Must provide a toggle between "Oscilloscope" (time domain) and "Spectrum" (frequency domain) views, or display them side-by-side if space permits.
- Must render the visualizations using high-density terminal characters (e.g., braille patterns) to maximize resolution.
- Must capture audio data using a lock-free, allocation-free circular buffer (ring buffer) that is safely read by the TUI thread without blocking the DSP engine.
- Must implement an efficient FFT (Fast Fourier Transform) on the TUI thread for the spectrum analyzer, ensuring the audio thread is not burdened with visualization math.
- Must visually indicate clipping (e.g., turning the waveform red if amplitude exceeds 1.0).

## 🚫 Out of Scope
- A full-featured GUI visualization using WebGL or external rendering libraries. This is strictly a terminal-based (ratatui) feature.
- Multi-channel independent metering. Phase 1 will focus solely on visualizing the stereo master output bus.
- Granular 3D spectrograms or waterfall displays. We are keeping it to 2D time and frequency plots.
