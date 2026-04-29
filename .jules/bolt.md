**[Eliminate Per-Event String Allocations]**
**Learning:** Constructing strings (via `.to_string()`, `.clone()`, or `format!()`) inside hot event-processing loops (e.g., rendering tracker grids or export loops) causes severe memory allocation bottlenecks.
**Action:** Pre-allocate all unique formatted strings into a `Vec<String>` before the loop, and use an inner collection holding string slices (`Vec<Option<&str>>`) to safely reference them during iteration, eliminating repetitive per-event allocations.
