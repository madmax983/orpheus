**[Refactoring Error Handling and Fallbacks]**
**Learning:** Found multiple instances where `unwrap_or(func())` or `if let Some(x) = y { ... } else { ... }` was used, triggering `clippy::or_fun_call` and `clippy::option_if_let_else`. This creates eager evaluation of fallbacks (wasting cycles) and creates clunky, imperative conditional blocks instead of declarative chains.
**Action:** Replace `unwrap_or(func())` with `unwrap_or_else(|| func())` to lazily evaluate the fallback. Replace `if let Some(x) = y` returning simple values with `.map_or(default, |x| ...)` or `.map_or_else(|| default(), |x| ...)`.

**[Formatting Argument Inlining]**
**Learning:** Formatting strings like `format!("{:?}", e)` triggered `clippy::uninlined_format_args`. Separating the format template from the variables requires extra mental tracking.
**Action:** Inline arguments directly inside the format string where possible: `format!("{e:?}")`.

**[Test Assertion Idioms]**
**Learning:** Using `if !condition { panic!("msg"); }` inside tests triggered `clippy::manual_assert`. The manual panic block adds unnecessary visual noise and indentation compared to standard macros.
**Action:** Replace `if !condition { panic!(...) }` with `assert!(condition, ...)`.

**[Flattening Match Arms]**
**Learning:** Enums with multiple variants executing the exact same logic (returning the same value) triggered `clippy::match_same_arms`. This creates unnecessary duplication and stretches the vertical length of the function.
**Action:** Merge identical match arms using the `|` operator (e.g. `VariantA | VariantB => value`).

**[Managing Large Configurations/Match Statements]**
**Learning:** Some functions like `with_builtins()` and `validate_control_events` are inherently long because they define a massive configuration map or evaluate dozens of unique control parameters. Breaking them up artificially into smaller helpers actually *hurts* readability by destroying the unified, declarative layout.
**Action:** When a long function is a giant `match` or configuration loader and cannot be logically extracted without reducing clarity, explicitly apply `#[allow(clippy::too_many_lines)]` rather than forcing an awkward split.