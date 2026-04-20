**[Implement Copy for Rational and TimeSpan]**
**Learning:** Core temporal primitives like `Rational` (two `i128`s) and `TimeSpan` (two `Rational`s) are small enough (32 bytes and 64 bytes respectively) to implement `Copy`. Passing them by reference and relying on `.clone()` adds unnecessary heap/reference indirection overhead on the hottest execution paths inside pattern queries and evaluations.
**Action:** Always derive `Copy` for small, stateless mathematical or temporal primitives and pass them by value to eliminate `.clone()` noise and improve performance.
## YYYY-MM-DD - [Remove clone when formatting f64 to integer degrees]
**Learning:** Found a hotpath where `format!("{value:.0}").parse::<i32>()` is used to truncate floats, creating string allocation per call. Converting it to `value as i32` achieves the same correctly and avoids any strings being copied, providing a free abstraction without lifetimes issues.
**Action:** Use `as i32` instead of formatting numbers to strings when truncating values.

**[Derive Copy for lightweight types to eliminate clone overhead]**
**Learning:** Core temporal primitives like `Rational` and `TimeSpan` in Orpheus only consist of a few numeric types (two `i128` values) but were frequently cloned on hot paths during cycle queries. This led to unnecessary `.clone()` calls causing measurable overhead.
**Action:** For lightweight structs consisting purely of primitive numerical values on hot paths, immediately derive `Copy`. By passing these types by value rather than by reference, we eliminate the need for explicitly calling `.clone()` or relying on reference lifetimes, which leads to cleaner code and avoids the overhead of deep cloning semantics.
