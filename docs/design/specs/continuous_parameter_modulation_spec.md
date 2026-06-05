# 🔭 Vantage: Spec for Continuous Parameter Modulation (LFOs & Envelopes)

## 👤 User Story
"As a Sound Designer and Composer, I want to use continuous signal generators like LFOs and Envelopes to modulate synthesizer parameters (like filter cutoff or pitch) smoothly over time, so that my sounds can evolve organically rather than stepping abruptly at cycle boundaries."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus's pattern language operates on discrete temporal events. While users can sequence a pattern of numbers to change a filter cutoff (e.g., `lpf(400 800 400 1200)`), these changes happen instantaneously at the start of each event. This discrete stepping creates an artificial, "chiptune" or rigid sequencer feel. True expressiveness in electronic music requires continuous, fluid modulation—sweeping a filter, vibrato via LFO, or organic volume swells. If Orpheus cannot support continuous control signals, it is functionally limited to basic step-sequencing. Complexity is a cost, but utility is revenue. Adding a unified way to map continuous modulation sources to any DSP parameter drastically expands the sonic palette, upgrading Orpheus from a basic pattern trigger to a fully modular synthesizer environment.

## 🎯 Metric Definition
- **Success** = Users can define an LFO or Envelope generator in the pattern language (e.g., `|> lpf(lfo(rate=0.5, depth=400, offset=800))`) and apply it to a synthesizer parameter.
- **Success** = The DSP engine processes this continuous modulation at the audio or block control rate without introducing noticeable stepping artifacts or exceeding a 10% CPU overhead per voice.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Control parameters (`Pattern<Number>`) only yield discrete `Number` values. The DSP engine reads these values once per event onset and holds them constant until the next event.
- **Competitors (TidalCycles, Strudel, SuperCollider):** TidalCycles uses "continuous patterns" (like `sine`) that are sampled at the discrete event rate, which can still cause stepping unless heavily oversampled. SuperCollider natively supports continuous audio-rate (ar) and control-rate (kr) modulation routing.
- **The Gap:** Orpheus lacks a representation of continuous signals in both the language type system and the DSP execution model. We need a bridge between the discrete pattern scheduler and the continuous audio processing graph.

## ✅ Acceptance Criteria
- Must introduce a new `Continuous` type or trait in the pattern language to represent signals that change smoothly over time (e.g., `lfo`, `env`).
- Must update the DSP engine to support sample-accurate or block-accurate updating of modulatable parameters (like cutoff, gain, and pitch) based on these continuous signals.
- Must provide language builtins for common modulation sources: `sine`, `tri`, `saw`, and `ramp` (for linear sweeps).
- Must allow these modulation sources to be parameterized by pattern discrete values (e.g., an LFO whose rate is sequenced by a step pattern).
- Must ensure that evaluating and rendering continuous modulation does not allocate memory on the real-time audio thread.

## 🚫 Out of Scope
- Audio-rate modulation for all parameters. To conserve CPU, Phase 1 will implement block-rate (control-rate) modulation for most parameters, interpolating between blocks.
- Drawing custom multi-point envelope shapes in the UI. Phase 1 relies on standard math functions (ADSR, LFO shapes).
- Modulating the time/scheduling engine itself (e.g., continuous tempo automation). Modulation is restricted to DSP parameters.
