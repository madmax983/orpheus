# 🔭 Vantage: Spec for Microtonal and Custom Tuning Support

## 👤 User Story
"As a Composer, I want to use custom tuning systems (like Bohlen-Pierce or 19-TET) and load standard Scala (.scl) files, so that I can explore non-Western musical scales and microtonal harmonies within my Orpheus patterns."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is strictly locked to the 12-Tone Equal Temperament (12-TET) Western scale. While this is standard for traditional DAWs, it severely alienates experimental musicians, non-Western composers, and the academic computer music community—a key demographic for code-based generative music. By forcing all mathematical pitch manipulation through a rigid 12-TET filter, we limit the sonic palette. Complexity is a cost; utility is a revenue. Supporting microtonal scales unlocks a massive new expressive dimension and establishes Orpheus as a serious tool for contemporary algorithmic composition, differentiating it from basic step sequencers.

## 🎯 Metric Definition
- **Success** = Users can load a valid `.scl` file via a new `tuning("file.scl")` global configuration. Evaluated patterns outputting integer values (e.g., `n(run(8))`) map successfully to the loaded scale degrees instead of standard MIDI semitones, driving the DSP oscillators at the exact calculated floating-point frequencies without increasing per-cycle query overhead by more than 5%.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Pitch literals (`c4`, `fs4`) and integer note values map rigidly to MIDI note numbers (where 60 = C4). Oscillators use fixed formulas like `440.0 * 2.0_f32.powf((midi_note - 69.0) / 12.0)`.
- **Competitors (SuperCollider, TidalCycles, Strudel):** SuperCollider supports arbitrary frequency math natively. TidalCycles has deep support for scale degrees and custom tuning systems. Ableton Live 12 recently added massive native support for microtonal tuning systems.
- **The Gap:** Orpheus needs a tuning resolution layer between the pattern evaluator (which thinks in integers/pitch classes) and the DSP engine (which needs precise floating-point Hz frequencies), as well as a parser for standard tuning files.

## ✅ Acceptance Criteria
- Must introduce a `tuning` parser capable of reading standard `.scl` (Scala) file formats.
- Must provide a global or per-pattern modifier (e.g., `n(0..7) |> tuning("slendro.scl")`) that maps integer values to the custom scale degrees.
- Must modify the `orpheus-dsp` synthesis primitives to accept arbitrary floating-point frequency inputs (`Hz`) directly instead of assuming integer MIDI notes.
- Must calculate base frequencies correctly so that `0` maps to the defined root frequency of the scale.
- Must handle out-of-bounds scale degrees smoothly by wrapping to higher/lower octaves as defined by the Scala file's interval of equivalence.

## 🚫 Out of Scope
- Dynamic real-time morphing between two different tuning systems mid-note.
- Support for keyboard mapping files (`.kbm`). Phase 1 will map integers directly to scale degrees sequentially.
- Polyphonic pitch bend micro-tuning (MPE). Phase 1 handles static frequency assignment per event at initialization.
