# 🔭 Vantage: Spec for Global Swing and Groove Templates

## 👤 User Story
"As a Beatmaker and Live Coder, I want to apply global swing percentages and extract/apply groove templates to my sequences, so that my patterns have a more organic, human, and off-grid feel without manually offsetting the rational time spans of every single event."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus evaluates all temporal patterns with perfect exact-rational mathematical precision. While this is mathematically pure and excellent for standard grid-based sequencing, it produces a rigid, "machine-like" feel that is undesirable for many genres of music (such as Hip-Hop, House, and Lo-Fi). To introduce swing, a user currently would have to manually adjust the time offsets for specific beats, which is tedious and clutters the code. Complexity is a cost; utility is revenue. By introducing a declarative global swing feature and the ability to apply groove templates, we allow users to separate the composition of note events from the rhythmic "feel" of the track. This makes Orpheus a significantly more musical and expressive tool.

## 🎯 Metric Definition
- **Success** = Users can apply a global `swing(amount)` transformation (where 0% is straight and 100% is maximum triplet swing) or a `groove("MPC60")` template to a pattern, causing the underlying exact-rational events to be dynamically micro-shifted during evaluation, with zero dropped notes and less than 5% performance overhead per cycle.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Events are locked to absolute exact-rational time fractions (`1/4`, `1/8`, etc). There is no built-in temporal distortion function for swing.
- **Competitors (Ableton Live, TidalCycles, MPC):** Ableton Live features a robust Groove Pool for applying micro-timing and velocity shifts. TidalCycles has `swingBy` and `nudge`. Hardware samplers like the MPC are famous for their unique swing algorithms.
- **The Gap:** Orpheus needs a temporal transformation layer that can take a strictly quantized exact-rational stream and apply a deterministic timing offset (and optionally, velocity offset) based on the event's position within a cycle or subdivision, effectively warping the timeline.

## ✅ Acceptance Criteria
- Must introduce a `swing(amount, subdivision)` function in the language that delays the even subdivisions (e.g., the second 16th note in a 8th note pair) by the specified `amount`.
- Must introduce a `groove(template_name)` function to apply predefined micro-timing maps.
- Must ensure that applying swing does not alter the logical length or duration of the overall cycle, only the internal event onsets and durations.
- Must correctly handle nested or overlapping patterns (e.g., swinging a snare while a ride cymbal remains straight).
- Must include a visual indication in the TUI (e.g., in the transport or mixer pane) if a global swing value is applied.
- Must gracefully handle swing values exceeding 100% or negative swing (rushing the beat).

## 🚫 Out of Scope
- Real-time audio analysis to extract a groove template from an external live audio input. Phase 1 is strictly pre-defined templates and standard shuffle-swing.
- Individual note micro-timing nudges within the TUI interface (piano roll style). All swing must be declared via code.
