**Iterator-based Zero-Cost Boundaries**
**Learning:** Functions that accept a slice of slices `&[&[T]]` force the caller to collect intermediate iterators into a `Vec` just to pass them by reference. This causes hidden, unnecessary O(N) allocations in hot paths like pattern evaluation.
**Action:** Always prefer `impl Iterator<Item = ...>` over `&[&[T]]` or `&[T]` for internal helper functions, allowing callers to pass chained iterators (`.iter().flat_map(...)`) and achieve zero-cost execution without intermediate heap allocations.
