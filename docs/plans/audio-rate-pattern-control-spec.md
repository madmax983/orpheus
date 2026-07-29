# Audio-Rate Pattern Control

👤 **User Story:**
As a Live Coder, I want audio-rate pattern control of voice parameters, so that I can fully express continuous, high-resolution sonic changes within the language.

**Business Problem:** The roadmap currently notes "remaining: audio-rate pattern control of voice parameters" as an incomplete feature. Completing this closes the final gap in the voice DSL's pattern integration, maximizing the utility of the existing parameter automation architecture.

**Success Metric:** Success = Audio-rate pattern evaluation is supported for voice parameters without requiring literal audio-rate evaluation of the entire pattern language.

**Gap Analysis:** The system currently supports intra-note parameter control via interpolated breakpoint data (up to 32 breakpoints per note). We lack true audio-rate control from the pattern side.

✅ **Acceptance Criteria:**
- Must implement the "audio-rate pattern control of voice parameters" roadmap item.
- Must not require audio-rate evaluation of the entire pattern language.

🚫 **Out of Scope:**
- Literal audio-rate pattern evaluation (this is intentionally out of scope per ADR 0012).
