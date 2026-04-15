**[Title]** Extracted Match Blocks and God Functions to Struct Methods
**Learning:** `clippy::too_many_lines` on functions with large `match` statements over an enum can often be resolved cleanly by moving the match block into an `impl` block on the enum itself. This follows Tell, Don't Ask, shrinks the caller function, and makes the enum operations more modular.
**Action:** When I encounter `too_many_lines` on a function switching over an enum to mutate state, I will implement methods directly on that enum instead of writing separate helper functions in the module scope.

**[Title]** Extracted God Function into Helper Functions
**Learning:** `clippy::too_many_lines` on the `process_stage` function was caused by multiple complex stages inside a match statement.
**Action:** Extracted `Tone`, `Filter`, and `Eq` stages into named helper functions (`process_tone_stage`, `process_filter_stage`, `process_eq_stage`) to reduce nesting and make the main function easier to read, resolving the lint.

**[Title]** Fix general clippy warnings
**Learning:** Clippy catches issues like collapsed if let chains, needless pass by value/borrowing, and redundant pub(crate).
**Action:** Extract guard clauses and apply simple let collapsing and apply const fns.

**Extracted Nested Assertions to Helper Functions**
**Learning:** `clippy::cognitive_complexity` on test functions is often triggered by deeply nested `match` statements or macro calls (`assert!(matches!(...))`) used to validate complex AST structures or deeply initialized configurations.
**Action:** When I encounter `cognitive_complexity` in a test validating a large object tree (like an AST or a large configuration struct), I will extract the nested match logic into focused assertion helpers (e.g., `assert_is_ident`, `assert_is_pipe`) or group the assertions logically into smaller helper functions (`assert_core_defaults`, `assert_fx_defaults`) to flatten the main test function.
**Extracted Match Blocks and God Functions to Struct Methods**
**Learning:** `clippy::too_many_lines` on functions dominated by repeated `writeln!` statements or similar string construction patterns can be condensed using a single large formatted `write!` string to reduce length and overhead.
**Action:** Combine repeated sequential write calls into single format strings where possible.
