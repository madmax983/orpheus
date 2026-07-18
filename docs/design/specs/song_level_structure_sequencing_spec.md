# 🔭 Vantage: Spec for Song-Level Structure Sequencing

**User Story:**
As a Composer, I want to sequence defined patterns into discrete, timed sections (like verse and chorus), so that I can arrange a complete song structure rather than endlessly looping a single cycle.

**So What?:**
Live coding environments excel at infinite generative loops, but fall short when users want to capture and export a finished, structured composition. Without a macro-level arrangement tool, Orpheus is restricted to improvisational jamming. Implementing `seq_sections` allows users to durably author and export complete tracks, expanding Orpheus's utility from a live performance tool to a full composition environment. Complexity is a cost; utility is revenue. This feature unlocks the ability to "finish" a track.

**Metric Definition:**
Success = 100% deterministic timeline generation for a `seq_sections` definition, allowing a multi-section `.ode` file to be completely rendered to audio with exact sample-accurate boundaries between sections.

**Gap Analysis:**
- **Competitors (Ableton Live, TidalCycles, Sonic Pi):** Ableton Live features the Arrange View for linear sequencing. Sonic Pi uses sequential imperative `play` / `sleep` commands for structure. TidalCycles is inherently loop-based and lacks native, robust timeline arrangement features, often forcing users to write massive conditional logic or sequence via an external DAW.
- **The Gap:** Orpheus lacks a high-level compositional primitive to transition between different pattern stacks automatically after a fixed number of cycles.

**Acceptance Criteria:**
- Must provide a `section(pattern, cycles)` function to define a finite block of music.
- Must provide a `seq_sections(...)` function that concatenates multiple sections into a single linear timeline.
- Must cleanly transition at the exact cycle boundary without audio artifacts or dropped events.
- Must reset or appropriately handle pattern state (e.g., Markov chains) at the start of a new section.

**Out of Scope:**
- Non-linear scene launching or Ableton-style Session View (this is a separate feature).
- Arbitrary crossfading between sections.
