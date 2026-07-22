# 🔭 Vantage: Spec for Audio-Rate Pattern Control

* 👤 **User Story:** As a Sound Designer, I want to control synthesizer parameters at audio rate using patterns, so that I can create smooth, continuous modulation without audible stepping.
* **The So What?:** Currently, parameter control is limited to discrete breakpoints. This prevents the creation of fluid, high-frequency modulation, which is a staple in electronic music. Adding this unlocks a new tier of professional sound design.
* **Metric Definition:** Success = Pattern parameter changes produce continuous audio-rate signals with zero audible quantization artifacts.
* **Gap Analysis:** Standard Faust graphs allow audio-rate modulation, but Orpheus patterns only send breakpoint automation to voice parameters. We need parity with standard modular synthesis environments.
* ✅ **Acceptance Criteria:**
  - Users can route pattern-driven control signals to voice parameters.
  - The parameter modulation is evaluated continuously at audio rate.
* 🚫 **Out of Scope:** Audio-rate pattern evaluation for structural patterns.
