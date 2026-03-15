## ⚡ Bolt Journal

**Title:** Direct span overlap checking instead of allocating `TimeSpan`
**Learning:** Checking overlap between start and end bounds of two timespans does not require constructing a new `TimeSpan` first. Calling `clip_span` constructed a new `TimeSpan` which performed a `.clone()` on both `Rational` endpoints and heap allocated a new struct, even when only the `is_some()` result was desired. By performing an inline `max`/`min` intersection check (`start < end`), we can eliminate both allocations and `.clone()` overheads completely inside the hot evaluation loop.
**Action:** Next time when evaluating bounded intersections, use raw math (like `max(a.start(), b.start()) < min(a.end(), b.end())`) directly instead of routing through heavy struct constructors if the resulting object will immediately be dropped.
