# Audio-Rate Pattern Control Specification

## 👤 User Story
As a Live Coder, I want to route patterns to control voice parameters at audio rates, so that I can create continuously evolving, rich timbres rather than being limited to breakpoint interpolation.

## The "So What?" (Business Problem)
We lack true continuous high-rate modulation from the REPL, limiting sound design to breakpoint rates. This restricts users from implementing FM/AM synthesis directly via patterns.

## Metric Definition
Success = Users can route audio-rate signals to voice parameters in their patterns without any audible degradation or dropouts in real-time audio playback quality.

## Gap Analysis
Competitors allow continuous audio-rate parameter modulation natively, while our engine is currently limited to breakpoint-rate interpolation.

## ✅ Acceptance Criteria
- Must allow users to route audio-rate patterns to control voice parameters continuously.
- Must ensure smooth integration between patterns and audio-rate control without breaking performance (maintains zero-allocation, lock-free audio thread constraints).

## 🚫 Out of Scope
- Literal audio-rate pattern evaluation.
