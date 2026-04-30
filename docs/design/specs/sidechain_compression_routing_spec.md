# 🔭 Vantage: Spec for Sidechain Compression Routing

## 👤 User Story
"As a Beatmaker and Live Coder, I want the ability to route the audio output of one pattern (like a kick drum) to control the compression amount of another pattern or bus (like a synth bass or pad), so that I can create the rhythmic 'ducking' and pumping effect that is essential for modern dance music."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has basic broadband compression capabilities, but they are localized to a single pattern or bus's own audio signal. The signature "pumping" sound of modern electronic music (Techno, House, EDM) relies almost entirely on *sidechaining*—where a loud sound (usually the kick drum) temporarily ducks the volume of other sustaining elements (like basslines and chords) to clear sonic space and create groove. Without sidechaining, mixes sound muddy and lack rhythmic drive, forcing producers to export stems and mix in a DAW. Complexity is a cost; utility is revenue. Implementing a sidechain routing architecture drastically improves the mix quality natively within Orpheus, allowing for professional-sounding, club-ready live sets without leaving the terminal.

## 🎯 Metric Definition
- **Success** = Users can define a routing where a source track (e.g., `drums`) triggers the gain reduction of a destination track or bus (e.g., `bass` or `synth_bus`) via a sidechain compressor. The routing must accurately track the source's envelope with < 2ms latency, ducking the destination signal synchronously with zero audio dropouts or locking on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Compression is internal to a track. There is no cross-track audio routing for control signals.
- **Competitors (Ableton Live, Bitwig, TidalCycles):** DAWs treat sidechain routing as a fundamental feature (e.g., Ableton's Glue Compressor sidechain input). TidalCycles relies on SuperDirt's complex global audio routing buses to achieve ducking.
- **The Gap:** Orpheus needs an extension to its mixer and DSP graph architecture to support sending a control-rate or audio-rate envelope from a designated "send" track into the sidechain detector input of a compressor node on a "receive" track or bus.

## ✅ Acceptance Criteria
- Must introduce a command or language syntax to establish a sidechain link (e.g., `:sidechain send drums synth_bus` or a `sidechain("drums")` effect modifier in the language).
- Must implement a dual-input compressor DSP node in `orpheus-dsp` that uses Input B (sidechain) to calculate gain reduction applied to Input A (main signal).
- Must extract the amplitude envelope of the sidechain source efficiently on the real-time audio thread.
- Must expose standard compressor controls for the sidechain effect: threshold, ratio, attack, and release.
- Must ensure that routing a sidechain signal does not introduce a feedback loop (e.g., a bus cannot sidechain itself).

## 🚫 Out of Scope
- Multi-band sidechain compression (e.g., only ducking low frequencies). Phase 1 is a broadband ducking effect.
- Lookahead sidechaining (which requires adding delay to the main signal path). Phase 1 relies on fast attack times.
- Sending MIDI triggers for sidechaining. Phase 1 strictly uses the audio amplitude envelope of the source track to trigger the compression.
