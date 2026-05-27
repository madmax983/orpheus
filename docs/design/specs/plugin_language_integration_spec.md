# 🔭 Vantage: Spec for Plugin Language Integration

## 👤 User Story
"As a Producer and Live Coder, I want to seamlessly integrate external VST3 and AudioUnit plugins directly into my Orpheus patterns, so that I can sequence complex third-party synthesizers and automate their parameters using Orpheus's native timing and transformation syntax."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has a foundational DSP layer (`PluginHost`, `PluginTrackSource`, `PluginProcessor`) capable of hosting VST3/AU descriptors and processing pre-scheduled MIDI and parameter events. However, the Orpheus language lacks syntactic sugar to connect these underlying capabilities. If users cannot cleanly instantiate, sequence, and automate these plugins within their code, the entire plugin hosting architecture remains an unused liability. Providing an intuitive language integration unlocks an immense ecosystem of external sounds, bridging the gap between built-in primitive audio and professional third-party instruments. Complexity is a cost; utility is a revenue.

## 🎯 Metric Definition
- **Success** = Users can successfully parse, evaluate, and hear playback of external VST3/AU plugins via the REPL or `.ode` files using simple function syntax (e.g., `vst("MySynth")` or `au("MySynth")`) and can sequence MIDI notes into them via standard Orpheus pattern juxtaposition.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The `orpheus-dsp` crate contains the plumbing (`plugin_host.rs`) to instantiate headless processors and accept `PluginTrackSource` inputs. The language parser currently only maps `vst(...)` and `au(...)` as raw functions returning unsequenced plugin descriptors.
- **Competitors (TidalCycles, Sonic Pi):** TidalCycles integrates with SuperDirt which handles MIDI out to external synths or internal plugins. Sonic Pi has built-in synths but more complex external MIDI routing.
- **The Gap:** The language needs native evaluation support to convert parsed plugin descriptors into functional sequencer tracks, accepting note events (like `C4`) and mapping them seamlessly to the underlying DSP track source.

## ✅ Acceptance Criteria
- Must define explicit parsing/evaluation semantics for routing note pattern events to a plugin (e.g., `vst("Serum") |> n("C4 E4 G4")` or direct sequencing `vst("Serum", "C4 E4 G4")`).
- Must support passing host automation parameters (e.g., `vst("Serum") |> p("cutoff", sine)`).
- Must define how the plugin state is cached or hot-reloaded across evaluation boundaries to prevent stuttering.
- Must provide clear error messages when a requested plugin cannot be found or instantiated by the underlying engine.

## 🚫 Out of Scope
- Full GUI hosting for plugins (headless processing only for now).
- MIDI CC routing (focus strictly on direct host parameter automation and standard Note On/Off).
- Audio effect plugins (Phase 1 focuses strictly on Instrument/Synth plugins generating audio from notes).
