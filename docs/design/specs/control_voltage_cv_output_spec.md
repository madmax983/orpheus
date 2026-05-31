# 🔭 Vantage: Spec for Control Voltage (CV) Output

## 👤 User Story
"As a Hardware-Integrated Live Coder, I want to output Control Voltage (CV) signals directly from Orpheus patterns to a DC-coupled audio interface, so that I can sequence, trigger, and modulate my external modular synthesizers (e.g., Eurorack) using Orpheus's precise and expressive pattern language."

## ❓ The "So What?" (Business Problem)
Live coding often exists purely "in the box," separating performers from tactile, analog hardware. While MIDI support allows basic integration with hardware synths, it suffers from jitter, low resolution (128 steps), and a rigid note-on/note-off paradigm. Modular synthesizers thrive on continuous, high-resolution analog voltages (CV) for pitch, gates, triggers, and complex LFO/envelope modulation.

By sending raw floating-point DSP output straight to DC-coupled audio interfaces, Orpheus bridges the gap between concise digital pattern manipulation and rich analog sound generation. Complexity is a cost, but unlocking an entire hardware ecosystem without needing a complex middleware application is massive utility. This allows performers to use Orpheus as a "super-sequencer" or "super-LFO" for their modular rigs, significantly increasing the platform's value for hardware enthusiasts.

## 🎯 Metric Definition
- **Success** = Orpheus can map specific pattern variables (e.g., pitch, envelopes, triggers) to arbitrary mono audio output channels with sample-accurate timing. The CV signal must remain stable (DC offset holding precisely) without high-pass filtering artifacts, and latency between code execution and voltage output must match the standard audio buffer latency (< 10ms).

## 🔍 Gap Analysis
- **Current State (Orpheus):** Orpheus processes audio internally and outputs stereo sound. It has no mechanism to map arbitrary un-attenuated low-frequency control signals (like LFOs or triggers) directly to auxiliary audio channels.
- **Competitors (TidalCycles, SuperCollider, Bitwig Studio):** SuperCollider can send CV via regular audio buses using DC-coupled interfaces. Bitwig Studio has deep, native hardware integration with dedicated CV/Gate devices. TidalCycles typically uses SuperCollider's CV capabilities or MIDI-to-CV hardware.
- **The Gap:** Orpheus needs a dedicated syntax to target specific multi-channel audio interface outputs and a specialized DSP node that passes control signals (like 1V/Octave pitch, 5V gates) without AC coupling or safety-limiting, ensuring exact voltage translation.

## ✅ Acceptance Criteria
- Must introduce a syntax/command to map a pattern to a specific physical output channel (e.g., `cv_out(3, pattern)`).
- Must provide conversion helpers (e.g., `v_oct(note)` to translate MIDI note numbers to 1V/Octave voltage scales).
- Must support sample-accurate gate/trigger patterns (e.g., generating a 5ms 5V pulse for a `bd` pattern).
- Must bypass the master bus safety limiter for channels designated as CV outputs, allowing signals to sustain static voltages (DC) safely within the interface's limits.
- Must cleanly reset to 0V when the pattern is stopped to prevent hanging notes or continuous gating on hardware.

## 🚫 Out of Scope
- Automatic calibration or tuning of external oscillators (Phase 2).
- Decoding incoming CV signals from the modular synth into Orpheus parameters (CV Input is out of scope for Phase 1).
- Support for non-DC-coupled audio interfaces (the hardware limitation is on the user).