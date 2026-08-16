# 🔭 Vantage: Spec for Audio-rate Voice Parameter Control

## 👤 **User Story:**
As a Livecoding Musician, I want to control instrument parameters (`p1`..`p4`) at literal audio rates using patterns, so that I can synthesize complex FM or AM timbres without writing custom DSP graph nodes.

## ❓ **The "So What?" (Business Problem)**
Currently, `p1`..`p4` have breakpoint automation (ADR 0012) where intra-note control data is computed on the non-RT query side and shipped with the trigger. This is great for smooth filter sweeps, but insufficient for true audio-rate synthesis. Bridging this gap unlocks modular-synth capabilities directly from the pattern language, increasing the expressive power of the environment.

## 🎯 **Metric Definition**
- **Success =** A pattern can drive a voice parameter (`p1`..`p4`) per audio frame without causing buffer underruns or exceeding real-time constraints.

## 🔍 **Gap Analysis**
- **Current State (ADR 0012):** Breakpoint automation where intra-note control data is computed on the non-RT query side and shipped with the trigger.
- **The Gap (Parity Roadmap):** The roadmap identifies "audio-rate pattern control of voice parameters" as remaining. Literal audio-rate pattern evaluation would require pattern queries on or near the audio thread, which currently contradicts the constraint that the engine never sees patterns and the audio thread is allocation-free per note.

## ✅ **Acceptance Criteria:**
- A continuous pattern assigned to `p1`..`p4` can modulate the target parameter per audio frame.
- The audio thread remains lock-free and allocation-free.
- The implementation resolves the architectural tension between non-RT pattern querying and per-frame audio rendering.

## 🚫 **Out of Scope:**
- Modulating routing snapshot parameters at audio rate.
- Increasing the number of per-note parameters beyond `p1`..`p4`.
