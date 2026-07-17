# 🔭 Vantage: Spec for Audio-Rate Pattern Control

## 👤 User Story
"As a Sound Designer, I want to use fast-moving pattern functions to directly modulate synthesizer parameters at audio rates, so that I can create complex timbres and aggressive textural sound design directly from the pattern language."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus supports breakpoint-rate pattern control for parameters (like interpolating between points every frame). However, to achieve FM synthesis, true AM, or extreme timbral modulation within the pattern language, parameter changes need to happen at true audio-rate (e.g. 44.1kHz). The lack of audio-rate modulation limits Orpheus's utility for advanced sound design, pushing users to external synthesizers rather than doing everything "in the box". By bringing audio-rate control of voice parameters into the pattern language, we enable an entirely new class of textures and behaviors directly from code, massively increasing the expressive ceiling of the tool. Utility is revenue; expanding the sonic territory is critical.

## 🎯 Metric Definition
- **Success** = Users can supply audio-rate pattern streams to voice parameters, with the engine properly resolving these as continuous audio-rate modulation instead of stepped or frame-interpolated values, maintaining low latency and zero dropouts.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Sub-note structure interpolates linearly every frame (breakpoint-rate control). True audio-rate pattern evaluation is not yet supported for voice parameters.
- **The Gap:** The engine lacks a mechanism to natively flow audio-rate signals directly from pattern space into voice parameter modulators without performance loss or downsampling.

## ✅ Acceptance Criteria
- Must extend the pattern language to support audio-rate control of voice parameters.
- Must provide a mechanism allowing audio-rate signal data to drive voice parameters.
- Must ensure the system handles audio-rate data streams without performance regressions.

## 🚫 Out of Scope
- Audio-rate modulation of global mixer parameters (Phase 1 will focus strictly on voice parameters).
