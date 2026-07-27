# 🔭 Vantage: Spec for chooseBy

👤 **User Story:**
"As a Live Coder, I want to use an external selector pattern to deterministically pick between a list of pattern options, so that I can drive complex sequencing changes (like selecting drum fills or chord voicings) using structured data rather than pure randomness."

**The So What? (Business Problem)**
Currently, Orpheus has probabilistic choice builtins (`wchoose`, `pchoose`), which are great for generative variation but lack precise deterministic control. When composers want to deliberately index into a list of musical patterns based on a sequence (e.g., using a bassline index to drive a drum pattern choice), they cannot do it. Complexity without control limits compositional utility. By implementing `chooseBy`, we bridge the gap between generative randomness and explicit deterministic structure, allowing users to build complex, interwoven sequences where one pattern directly controls the progression of another.

**Metric Definition**
- Success = Users can call `chooseBy(selector_pattern, [p0, p1, ...])` and the system will exactly output the events of the pattern at the index dictated by `selector_pattern`'s current value, with zero runtime panics on out-of-bounds indices and <1ms evaluation overhead.

**Gap Analysis**
- Current State (Orpheus): The `pchoose` and `wpchoose` functions exist and allocate slots randomly based on site-salted determinism. The gap, explicitly identified in the parity roadmap, is the lack of `chooseBy`, which relies on an *external selector pattern* rather than a random draw.
- Competitors (TidalCycles): TidalCycles fully supports `chooseBy` functionality as a gap identified in the parity roadmap against the current implementation.
- The Gap: Orpheus lacks an explicit external-index-driven pattern selector.

✅ **Acceptance Criteria:**
- Must introduce a new builtin `chooseBy` that takes a continuous or discrete number pattern as the first argument (the selector) and a sequence of pattern arguments as the pool.
- Must sample the `selector_pattern` at the start of each cycle or event slot to determine the integer index.
- Must safely clamp or wrap the selector index if it falls out of the bounds of the provided pattern list.
- Must seamlessly sequence the chosen pattern slice for the duration dictated by the selector event.

🚫 **Out of Scope:**
- Implementing n-dimensional pattern matrices (e.g., selecting across rows and columns). Phase 1 is a 1D list selector.
- Audio-rate indexing (where the index changes per-sample). Indexing happens at pattern evaluation time.
