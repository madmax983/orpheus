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

**Extracting TUI Component Rendering**
**Learning:** `clippy::too_many_lines` on UI rendering functions (like those creating Ratatui layouts) is often caused by inlining the setup, layout, and rendering of multiple distinct sections (e.g., tracks and buses) into a single function block.
**Action:** Extract the rendering logic for distinct visual components into separate helper methods that accept a mutable reference to the line buffer (`&mut Vec<Line<'static>>`) and the necessary styling contexts. This flattens the main orchestrator function and groups related UI logic without changing the rendered output.

**Redundant pub(crate)**
**Learning:** `clippy::redundant_pub_crate` warns about `pub(crate)` items inside private modules. Since the module itself is private to the crate, making the item `pub(crate)` is functionally equivalent to making it `pub`, but `pub` is more idiomatic and cleaner.
**Action:** When working in private modules, use `pub` instead of `pub(crate)` for items intended to be accessible throughout the crate. Avoid suppressing the warning with `#[allow(clippy::redundant_pub_crate)]`.

**Extract match arms to method helpers**
**Learning:** `clippy::too_many_lines` on large enum `match` statements can be resolved by pulling the complex arms into individual methods. This flattens the code structure and improves readability, eliminating the need to use `#[allow(clippy::too_many_lines)]`.
**Action:** Extract large match arms into separate private helper methods on the enum.
**Extracting Guard Clauses Without Duplicating Routing Logic**
**Learning:** When flattening nested guard clauses inside large `match` statements (like a REPL command dispatcher where many arms check `if args.is_empty() { Err(...) }`), it is a critical mistake to extract the error condition by duplicating the entire command routing list. Duplicating the match arms into an upfront `if args.is_empty() { match name { ... } }` and a subsequent execution `match name { ... }` violates DRY principles and creates a fragile maintainability trap where adding a new command requires updating two separate lists.
**Action:** When extracting preconditions across multiple match arms, extract the precondition into a single block that handles ONLY the arms requiring that condition (falling through with `_ => {}` for others), and retain the single source-of-truth execution `match` block below it.
**[Title] Refactored massive function with enum match to impl method
**Learning:**  on a function that pattern-matches an enum with 70+ variants (like an AST or ) is often caused by the match statement being a freestanding function instead of a method. Moving the function into an  block as a method (e.g., ) follows Tell, Don't Ask, shrinks the caller function, and makes the enum operations more modular. If splitting the match into 70 different helper methods would destroy readability, it's safe to keep the large match and apply  specifically to the method.
**Action:** When I encounter  on a freestanding function switching over a massive enum, I will implement it as a method directly on that enum instead of writing separate helper functions in the module scope.

**[Refactored massive function with enum match to impl method]**
**Learning:** `clippy::too_many_lines` on a function that pattern-matches an enum with 70+ variants (like an AST or `PatternRuntime`) is often caused by the match statement being a freestanding function instead of a method. Moving the function into an `impl` block as a method (e.g., `fn with_tuning(self, ...)`) follows Tell, Don't Ask, shrinks the caller function, and makes the enum operations more modular. If splitting the match into 70 different helper methods would destroy readability, it's safe to keep the large match and apply `#[allow(clippy::too_many_lines)]` specifically to the method.
**Action:** When I encounter `too_many_lines` on a freestanding function switching over a massive enum, I will implement it as a method directly on that enum instead of writing separate helper functions in the module scope.
**[Execution Order Semantics]
**Learning:** Extracting logic into helper methods can accidentally alter the execution order of operations that produce side-effects or errors (like evaluating an expression versus evaluating a count). In interpreters or evaluators, changing this order fundamentally alters semantics and breaks the 'zero behavior change' refactoring rule.
**Action:** When performing 'Extract Method' refactorings in evaluator code, strictly preserve the original sequential order of evaluations, variable assignments, and error checks to prevent unintended logic changes.

**[Title] Fix Redundant pub(crate)**
**Learning:** `clippy::redundant_pub_crate` warns about `pub(crate)` items inside private modules. Since the module itself is private to the crate, making the item `pub(crate)` is functionally equivalent to making it `pub`, but `pub` is more idiomatic and cleaner.
**Action:** When working in private modules, use `pub` instead of `pub(crate)` for items intended to be accessible throughout the crate. Avoid suppressing the warning with `#[allow(clippy::redundant_pub_crate)]`.

**[Extracting Table Builders to Helper Methods]**
**Learning:** Extracting large `match` blocks or procedural table building logic across multiple files into a shared helper function `explain_table` reduces code duplication and line count, while keeping the output identical.
**Action:** When refactoring multiple structs that use identical boilerplate setup (like `comfy_table::Table::new()`), extract the setup into a centralized `pub fn` to simplify the individual methods.
**[Refactor  pattern match blocks]**\n**Learning:** The  functions for structural pattern combinators in  contained repetitive, verbose match blocks over  returning  for everything except  and .\n**Action:** Extracted the core routing logic into a  helper function, utilizing closures to safely extract mutable closures, and dramatically flattened the , , , , , , , , , , and  functions.

**[Refactor apply_ pattern match blocks]**
**Learning:** The apply_ functions for structural pattern combinators in builtins.rs contained repetitive, verbose match blocks over Value returning EvalError for everything except SamplePattern and NumberPattern.
**Action:** Extracted the core routing logic into a apply_pattern_transform helper function, utilizing closures to safely extract mutable closures, and dramatically flattened the apply_every, apply_when, apply_sometimes, apply_within, apply_mask, apply_roll, apply_fast, apply_slow, apply_shift, apply_rev, and apply_chaos functions.

**[Shared Trait Abstraction]**
**Learning:** Having identical function signatures (like `pub fn explain(&self, binding_name: &str) -> String`) across multiple disjoint types represents a missed opportunity for polymorphic abstractions.
**Action:** Extract identical methods into a shared trait (e.g., `Explain`) and implement it for the relevant types to establish a formal abstraction, grouping any shared helpers (like `explain_table`) in the same module.

**Refactoring `clippy::too_many_lines` on massive matches**
**Learning:** `clippy::too_many_lines` on large enum `match` statements across different methods (like `absolute_cycle` or `try_query_transform` on an AST or Value enum) can be resolved by extracting inner chunks to helper methods and appending `#[allow(clippy::too_many_lines)]` locally if breaking it up too much destroys readability.
**Action:** Extract large portions of the match to private helper methods, such as `try_query_transform_method` or `try_query_audio_effect_method` and suppress the clippy warning there if the match must remain large.

**Extracting Match Arms that mutate State**
**Learning:** Destructuring mutable fields from `&mut self` and modifying them locally avoids passing `&mut self` to helper methods, preventing borrow checker issues.
**Action:** Pass only the destructured fields (and other needed vars) directly to the helper methods rather than the entire `self` struct to satisfy the borrow checker.
**[Refactor eval_slot_patterns to use match]
**Learning:** Sequential `if` conditions checking boolean state on a collection can lead to duplicated mapping logic and obfuscated exclusive conditions.
**Action:** Use a consolidated `match (bool_a, bool_b)` to handle exclusive states, which flattens logic, exposes unreachable states explicitly, and enables mapping via iterators.
