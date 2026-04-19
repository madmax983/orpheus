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

**Refactoring unstable `let_chains`**
**Learning:** `clippy::collapsible_if` or compiler errors regarding the unstable `let_chains` feature (`if let Some(x) = y && condition`) should not be resolved using nested `if` statements with `#[allow(clippy::collapsible_if)]`. This creates unnecessary nesting and introduces artificial suppressions that go against the "Nesting is the mind-killer" philosophy.
**Action:** Use `Option::is_some_and` (e.g., `if y.is_some_and(|val| condition)`) instead to maintain a clean, flat structure.

**Extracting Match Contexts with Borrow Checking**
**Learning:** When refactoring large `match` expressions to fix `clippy::too_many_lines`, do not consolidate context structs that contain mutable references (e.g., `&mut [T]`) before the `match` statement. Eager instantiation moves the references, causing borrow checker 'moved value' errors on branches that do not use the context struct but try to access the underlying references directly.
**Action:** Instantiate context structs locally within the specific match arms that actually require them, or re-apply `#[allow(clippy::too_many_lines)]` if safe extraction is not possible without significant refactoring.

**[Refactor `builtin_value` String Match]**
**Learning:** Extracting monolithic `match` statements over string literals into categorical helpers and sequencing them via `.or_else()` provides a clean, safe, and readable refactor that maintains zero logic changes and is very easy to read, without triggering clippy.
**Action:** Use `.or_else()` chaining when breaking up massive lookup matches into multiple logical helpers.

**[Avoid Splitting Config Structs]**
**Learning:** When trying to resolve `too_many_lines`, avoid destroying existing `Context` or configuration structs into many individual parameters, as this actively violates idiomatic parameter grouping rules and can cause `too_many_arguments` clippy warnings.
**Action:** When a function takes a Context struct, leave it intact. Look for other opportunities to shorten the function, such as extracting logical blocks out of the main function entirely, while still passing the Context struct to the new helpers.
