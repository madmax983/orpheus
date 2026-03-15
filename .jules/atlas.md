**[Extracted Export Module from Eval Module]
**Tangle:** The `eval.rs` module in `orpheus-lang` crate contained both AST evaluation logic and audio rendering/CSV exporting functionalities, creating a "Blob" anti-pattern.
**Blueprint:** Extracted rendering and exporting functions along with `RenderError` into a new `export.rs` module. Updated `lib.rs` and tests to rely on `export` module properly to enforce better separation of concerns and high cohesion.
