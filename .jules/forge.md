**[Rational to Float Conversion]**
**Learning:** Found repetitive logic for converting a `Rational` to `f64` embedded in business logic inside `crates/orpheus-lang/src/eval.rs`, creating a "Boolean Blindness"-like clutter of casting logic.
**Action:** Extract conversions to `impl From<&Rational> for f64` (and its owned counterpart) inside `crates/orpheus-pattern/src/rational.rs`, keeping business logic clean and typed.

**[Boundary overlap computation in Orpheus-lang]**
**Learning:** Found deeply nested duplicated logic for matching overlapping span boundaries across `apply_control_pattern`, `apply_slice_pattern`, and `apply_slice_idx_pattern` causing "God Function"-like behavior.
**Action:** Extract logic into `compute_event_fragment_boundaries` returning `Option<Vec<Rational>>` to flatten structures and remove repetitive boilerplate.

**[Pattern CSV Export Refactoring]**
**Learning:** Found repetitive boilerplate for creating files and looping over events to export to CSV across `export_sample_pattern_to_csv` and `export_number_pattern_to_csv`.
**Action:** Extracted the common setup and writing loop into a generic `export_pattern_events_to_csv` helper function taking a formatting closure to conform to DRY principles.
