**[Rational to Float Conversion]**
**Learning:** Found repetitive logic for converting a `Rational` to `f64` embedded in business logic inside `crates/orpheus-lang/src/eval.rs`, creating a "Boolean Blindness"-like clutter of casting logic.
**Action:** Extract conversions to `impl From<&Rational> for f64` (and its owned counterpart) inside `crates/orpheus-pattern/src/rational.rs`, keeping business logic clean and typed.

**[Boundary overlap computation in Orpheus-lang]**
**Learning:** Found deeply nested duplicated logic for matching overlapping span boundaries across `apply_control_pattern`, `apply_slice_pattern`, and `apply_slice_idx_pattern` causing "God Function"-like behavior.
**Action:** Extract logic into `compute_event_fragment_boundaries` returning `Option<Vec<Rational>>` to flatten structures and remove repetitive boilerplate.

**[Boilerplate Reduction in CSV Exporting]**
**Learning:** Identified duplicate code logic for CSV file initialization, error handling, and event looping in `export.rs` across sample and number pattern exports.
**Action:** Extract CSV boilerplate into a generic `export_pattern_events_to_csv` helper, removing duplication and keeping file I/O operations central.

**[Boilerplate error mapping reduction]**
**Learning:** Found repetitive logic mapping errors to `EvalError` using `.map_err(|e| EvalError::new(e.to_string()))` scattered across string formatting and file exporting operations.
**Action:** Extract conversions to `impl From<std::io::Error> for EvalError` and `impl From<std::fmt::Error> for EvalError` inside `crates/orpheus-lang/src/eval.rs`, simplifying error handling logic and propagating errors cleanly with `?`.

**[Iterator Chains over Loops]**
**Learning:** Found several places in `crates/orpheus-lang/src/eval.rs` where manual loops `for item in items` were pushing into a mutable vector, creating boilerplate and unnecessary state mutation.
**Action:** Replaced these loops with functional iterator chains like `.iter().map().collect()` and `.try_fold()`, which simplifies the code and is more idiomatic Rust.
**[Boilerplate Reduction in JSON Exporting]**
**Learning:** Identified duplicate code logic for JSON file initialization, metadata writing, and event looping in `export.rs` across sample and number pattern exports.
**Action:** Extract JSON boilerplate into a generic `export_pattern_events_to_json` helper, removing duplication and keeping file I/O operations central.
**[Avoid double negations in else branches]**
**Learning:** Found an `if !condition { ... } else { ... }` in `crates/orpheus-lang/src/mixer.rs` that triggers the `clippy::if_not_else` lint, increasing cognitive load when parsing the branches.
**Action:** Flip the condition to be positive (`if condition`) and swap the `if` and `else` blocks.
