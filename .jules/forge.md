**[Resolve clippy warnings across orpheus-dsp and orpheus-lang]
**Learning:** `match` arms evaluating `Value` types with similar error output logic can be refactored into identical match arms and then collapsed, reducing redundant evaluation arms. However, combining variants like `Value::String(_)` with `Value::Pedal(_)` causes `clippy::match_same_arms` warnings to disappear but must still ensure correct fallback.
**Action:** When seeing `clippy::match_same_arms`, use `|` matching for multiple variants sharing identical logic. Use `#allow(clippy::large_enum_variant)` for recursive enums in the parser `Value` to avoid excessive boxing that disrupts the AST interface if boxing is not immediately intended.

**Learning:** `clippy::format_push_string` indicates using `write!` or `writeln!` over `push_str(&format!(...))` avoids multiple String allocations.
**Action:** Always use `use std::fmt::Write;` and `writeln!(buffer, ...)` to write into strings efficiently without extra heap allocations.

**Learning:** `f32::log` is more optimal and accurate than dividing `f32::ln()` by another `f32::ln()`.
**Action:** Address `clippy::suboptimal_flops` using `.log(base)`.

**Learning:** When asserting bounds and strict comparisons on floating-point arithmetic (e.g. `assert_eq!(..., 0.0)`), use an epsilon comparison to prevent floating point inaccuracies.
**Action:** Address `clippy::float_cmp` using `(a - b).abs() < f32::EPSILON`.
