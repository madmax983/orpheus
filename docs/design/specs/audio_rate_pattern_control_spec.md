# 🔭 Vantage: Spec for Audio-Rate Pattern Control

## 👤 User Story
As a Sound Designer, I want to control voice parameters at audio rate using pattern expressions, so that I can create complex, continuously evolving modulations (like FM synthesis or fast LFO sweeps) directly from the live-coding language without manually writing Faust DSP blocks.

## 🎯 The "So What?" (Business Problem)
Currently, Orpheus bridges the gap between patterns and DSP with per-note parameters (`p1`-`p4`) and breakpoint interpolation. However, it lacks true audio-rate modulation from the pattern level. Without this, users cannot do classic modular synthesizer patching (like audio-rate FM or AM) purely via pattern expressions. Bridging this gap turns Orpheus into a complete sonic laboratory where structure and sound design are truly unified.

## 📊 Metric Definition
- **Performance:** CPU overhead for a voice utilizing audio-rate pattern control must not exceed 10% more than a static parameter voice.
- **Reliability:** 0 dropped frames during live audio-rate parameter updates.

## 🔍 Gap Analysis
- **Current State:** `p1`-`p4` are evaluated at trigger time or as interpolated breakpoints (sub-note).
- **Market/Standard Libs:** TidalCycles operates mostly at control rate. SuperCollider and Faust allow audio-rate mapping. Orpheus needs to align with Faust's capabilities at the pattern level.

## ✅ Acceptance Criteria
- Must introduce a mechanism to evaluate pattern expressions at audio rate for voice inputs.
- Must cleanly degrade or fallback if audio-rate computation for a given pattern is impossible.
- Must prevent aliasing or zipper noise where parameters are stepped instead of smoothed.
- Must integrate seamlessly with the existing `p1`-`p4` pipeline without breaking legacy breakpoint behavior.

## 🚫 Out of Scope
- Inter-voice audio-rate routing (patching audio from one voice directly into another).
- Cross-network audio-rate modulation (OSC/UDP audio-rate).
