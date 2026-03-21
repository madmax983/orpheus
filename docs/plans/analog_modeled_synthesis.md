# 🔭 Vantage: Spec for Analog-Modeled Synthesis

## 👤 User Story
"As a Live Coder, I want built-in, high-quality analog-modeled oscillators (PolyBLEP) and a resonant ladder filter, so that I can synthesize rich, classic electronic sounds (like deep basslines and cutting leads) directly in Orpheus without relying on external samples or DAWs."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus relies almost entirely on sample playback for sound generation. While samples are great for drums and textures, they are inflexible for melodic and harmonic parts where timbre needs to evolve dynamically (e.g., filter sweeps, pulse-width modulation). If users want authentic synthesizer sounds, they must either pre-render a massive library of sample slices or route MIDI out to external gear. By introducing mathematically robust synthesis (PolyBLEP oscillators to prevent aliasing and a Moog-style ladder filter for character), we transform Orpheus into a standalone, powerful synthesizer engine. Complexity is a cost; utility is revenue. True analog-modeled synthesis acts as a massive utility multiplier, allowing for expressive, evolving sound design entirely within the code layer.

## 🎯 Metric Definition
- **Success** = The new `osc` voice types (`saw`, `pulse`, `tri`) and `ladder` filter function operate without perceptible aliasing up to 10kHz, consume < 5% CPU overhead per voice during a dense 16-step sequence, and allow for real-time parameter modulation (e.g., cutoff, resonance) without audio artifacts or dropouts.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sound generation is heavily biased towards WAV sample playback. Built-in synthesis is rudimentary and lacking character or alias-free generation.
- **Competitors (Sonic Pi, SuperCollider):** Both have extensive built-in synthesizers with dozens of algorithms and modeled filters. Sonic Pi specifically offers very accessible analog-style synths (`:tb303`, `:prophet`).
- **The Gap:** Orpheus needs a core suite of virtual analog DSP building blocks (oscillators and filters) that integrate seamlessly into the `Pattern<Note>` and `Pattern<Number>` paradigms, providing rich melodic capability.

## ✅ Acceptance Criteria
- Must introduce a new `synth` or `osc` built-in function to the language (e.g., `synth("saw")`) that responds to `Note` events.
- Must implement PolyBLEP (Polynomial Bandlimited Step) algorithms for sawtooth and pulse/square waves in the `orpheus-dsp` engine to prevent digital aliasing at high frequencies.
- Must implement a 4-pole resonant ladder filter (e.g., a digital Moog ladder model) accessible via the language (e.g., `|> cutoff(800) |> res(0.8)`).
- Must support sequencing synthesizer parameters via standard patterns (e.g., `cutoff(slow(2, seq(400, 2000)))`).
- Must run efficiently on the real-time audio thread without any allocation or locking during block rendering.

## 🚫 Out of Scope
- FM Synthesis or Wavetable Synthesis. Phase 1 is strictly subtractive virtual analog.
- Complex envelope generators (ADSR) exposed to the language. Phase 1 will use a sensible default percussive or gated envelope for notes.
- Polyphony via a single `synth` call (e.g., playing chords through one filter instance). Phase 1 treats each note event as an independent voice instantiation.
