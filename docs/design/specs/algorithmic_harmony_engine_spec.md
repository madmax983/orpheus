# 🔭 Vantage: Spec for Algorithmic Harmony Engine

## 👤 User Story
"As a Composer, I want an Algorithmic Harmony Engine, so that I can easily apply diatonic scales, chord progressions, and functional harmony to raw numeric or interval patterns without having to manually specify every absolute pitch."

## ❓ The "So What?" (Business Problem)
Currently, users are restricted to hard-coding explicit pitches or simple numeric offsets for melodies. This creates high friction when trying to modulate keys or dynamically transpose a sequence. Music isn't a collection of static, absolute frequencies; it's relational. If Orpheus lacks a way to map abstract patterns onto musical scales and chords, complex harmonic compositions become tedious to write and read. Complexity is a cost; utility is revenue. Implementing an Algorithmic Harmony Engine unlocks rapid, generative melodic exploration, making Orpheus drastically more expressive and usable for harmonic composition.

## 🎯 Metric Definition
- **Success** = Users can define a base key/scale (e.g., `scale("C major")`) and pipe a sequence of integer degrees (e.g., `0 2 4`) through it, which is automatically converted to the correct absolute frequencies or MIDI note numbers. The conversion logic must run in real-time within the pattern evaluation step, introducing < 1ms of latency per cycle resolution.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Melodic information is handled via exact string literals or semitone offsets. There is no concept of a "scale degree" mapping.
- **Competitors (TidalCycles, Sonic Pi):** TidalCycles has scale functions (`scale "major" "0 2 4"`). Sonic Pi extensively uses scale and chord lookups for sequencing.
- **The Gap:** Orpheus needs a semantic layer that bridges the gap between abstract integer sequences (degrees) and absolute musical pitches, respecting key signatures and functional harmony.

## ✅ Acceptance Criteria
- Must introduce a new `scale(name)` function in the pattern language that accepts common scale names (e.g., "major", "minor", "dorian", "pentatonic").
- Must introduce a `degree(pattern)` transform that maps integers to the active scale.
- Must support automatic transposition when a pattern is shifted (e.g., `+ 2` shifts by two scale degrees, not two semitones, when applied before resolution).
- Must resolve the resulting scale degrees into absolute frequencies (Hz) for the DSP engine or MIDI note numbers for MIDI export.
- Must execute the scale mapping logic efficiently during pattern evaluation, maintaining lock-free guarantees if evaluated on the audio thread.

## 🚫 Out of Scope
- Microtonal or non-Western scales (Phase 2). This phase focuses strictly on 12-TET diatonic/standard scales.
- Automated voice-leading or chord inversion generation. Users must still specify the discrete degrees for chords.
