# 🔭 Vantage: Spec for Master Bus Safety Limiter

## 👤 User Story
"As a Live Coder, I want a built-in safety limiter on the master output bus, so that I can experiment with complex DSP graphs, high-feedback delays, and aggressive gain staging without risking hearing damage or blowing out my studio monitors with sudden volume spikes."

## ❓ The "So What?" (Business Problem)
Live coding is inherently experimental and prone to user error. Typing `gain(100)` instead of `gain(1.0)` or accidentally creating a runaway feedback loop in a custom DSP node can instantaneously produce signals at +100dBFS. In a traditional DAW, users can insert a limiter plugin on the master track. In Orpheus, the code *is* the instrument, and there is currently no hard ceiling preventing the audio thread from outputting dangerous floating-point values directly to the system's audio driver. Complexity is a cost; physical harm is an unacceptable liability. A transparent safety limiter ensures user safety and confidence, making the environment welcoming rather than dangerous. Utility here is literally preventing damage.

## 🎯 Metric Definition
- **Success** = The audio engine applies a zero-latency, hard-knee limiter (or clipper) on the final stereo output stage. Any signal exceeding 0.0dBFS (or a configurable threshold like -0.3dBFS) must be gracefully clamped without causing digital clipping artifacts or panics, and this safety measure must introduce < 1% CPU overhead on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The `orpheus-dsp` engine routes the final mix directly to the audio output stream without any amplitude bounds checking or limiting.
- **Competitors (SuperCollider, Sonic Pi, TidalCycles):** SuperCollider offers built-in ways to catch spikes (e.g. Limiter UGens). Sonic Pi has a built-in safety limiter enabled by default. TidalCycles relies on SuperDirt, which generally includes a master limiter in its default setup.
- **The Gap:** Orpheus lacks an automatic safety net at the master output stage, leaving it vulnerable to uncaught mathematical explosions in DSP patterns.

## ✅ Acceptance Criteria
- Must insert a lightweight hard-limiter or clipper at the very end of the `orpheus-dsp` master output chain, immediately before sending frames to the system audio driver.
- Must prevent any output sample value from exceeding the [-1.0, 1.0] float range.
- Must operate completely transparently (no audible effect) when the signal is below the threshold.
- Must be enabled by default for all new sessions.
- Must have a configuration option to disable it (for users who want to print raw floating-point data or route to external safety systems), e.g., via a CLI flag `--unsafe-audio`.

## 🚫 Out of Scope
- A fully-featured mastering limiter with lookahead, adjustable attack/release times, and dithering. This is purely a brick-wall safety clipper to prevent driver overload and hearing damage.
- Visual metering or clipping indicators in the TUI. Phase 1 is purely the DSP safety mechanism.
