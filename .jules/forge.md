**[Rational to Float Conversion]**
**Learning:** Found repetitive logic for converting a `Rational` to `f64` embedded in business logic inside `crates/orpheus-lang/src/eval.rs`, creating a "Boolean Blindness"-like clutter of casting logic.
**Action:** Extract conversions to `impl From<&Rational> for f64` (and its owned counterpart) inside `crates/orpheus-pattern/src/rational.rs`, keeping business logic clean and typed.

**[Boundary overlap computation in Orpheus-lang]**
**Learning:** Found deeply nested duplicated logic for matching overlapping span boundaries across `apply_control_pattern`, `apply_slice_pattern`, and `apply_slice_idx_pattern` causing "God Function"-like behavior.
**Action:** Extract logic into `compute_event_fragment_boundaries` returning `Option<Vec<Rational>>` to flatten structures and remove repetitive boilerplate.

**Refactoring apply_event_fragments**
**Learning:** Found redundant boilerplate looping code dealing with extracting/applying fragmented control events. Iterators for generating composed `Event<T>` sequences based on boundaries was copy-pasted in multiple functions: `apply_control_pattern`, `apply_slice_pattern`, and `apply_slice_idx_pattern`.
**Action:** Extracted this traversal/computation loop into a `apply_event_fragments` higher-order function, abstracting out the duplicated iteration details to drastically reduce code repetition.
