**[Rational to Float Conversion]**
**Learning:** Found repetitive logic for converting a `Rational` to `f64` embedded in business logic inside `crates/orpheus-lang/src/eval.rs`, creating a "Boolean Blindness"-like clutter of casting logic.
**Action:** Extract conversions to `impl From<&Rational> for f64` (and its owned counterpart) inside `crates/orpheus-pattern/src/rational.rs`, keeping business logic clean and typed.

**[CSV Export Duplication]**
**Learning:** Found massive logic duplication when exporting different pattern types (`SamplePatternValue` and `NumberPatternValue`) to CSV, leading to repetitive file creation, error handling, loop iteration, and span clipping boilerplate.
**Action:** Extracted the core CSV generation logic into a generic `export_pattern_events_to_csv` helper function that accepts a formatting closure for varying payload structures.
