# 🔭 Vantage: Spec for Audio-Rate Pattern Control of Voice Parameters

👤 **User Story:**
As a sound designer and live coder, I want to use pattern expressions to modulate synth voice parameters at exact audio-rate resolution (per sample), so that I can create complex textures like audio-rate frequency modulation directly from the pattern language without being constrained by control-rate interpolation.

❓ **The "So What?" (Business Problem)**
Currently, parameter automation in Orpheus is limited to control-rate interpolation. While this is sufficient for smooth filter sweeps, it locks users out of classic sound design techniques that require sample-accurate modulation. By bridging pattern control to the engine's audio rate, we unlock a massive new category of expressive capabilities, differentiating Orpheus from setups where the pattern language and DSP are separated by low-resolution message rates.

🎯 **Metric Definition**
- Success = Modulation signals assigned from patterns update at the engine's exact sample rate without audible stepping artifacts.
- Performance Success = No introduced audio dropouts or execution latency regressions; the solution must comfortably run within the real-time audio budget.

🔍 **Gap Analysis**
- Current State: Parameter controls support intra-note sub-structure but rely on linear interpolation at control-rate, which cannot represent high-frequency modulation.
- Standard Libs: SuperCollider handles audio-rate control directly. TidalCycles relies on external synths and is bottlenecked by message rates.
- The Gap: Orpheus has the advantage of a shared process between language and DSP, but lacks the bridge to pass audio-rate pattern representations to the graph voices efficiently.

✅ **Acceptance Criteria:**
- Users must be able to designate a pattern parameter stream to resolve at audio-rate.
- The audio engine must receive and apply this modulation per sample.
- The implementation must gracefully handle the boundary between pattern timing (rational time) and DSP timing (samples).
- Must seamlessly coexist with the existing control-rate breakpoint interpolation for non-audio-rate use cases.

🚫 **Out of Scope:**
- Literal audio-rate pattern evaluation: evaluating the full pattern language AST every sample is computationally unviable and explicitly out of scope.
