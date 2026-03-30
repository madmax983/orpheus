**Inlining clip_span vs LLVM**
**Learning:** Manually inlining a helper function like `clip_span()` to avoid a perceived struct allocation (`TimeSpan`) and `.clone()` calls is often a false micro-optimization. In `clip_span`, the early return on non-overlapping windows already bypassed the allocation, and LLVM trivially inlines small helper functions anyway. Manual inlining violated DRY without a real performance gain.
**Action:** Trust LLVM for small helper functions that return early. Focus on algorithmic changes or reducing guaranteed heap allocations (like `Vec::new()`) inside loops instead.

**Remove unnecessary BTreeMap clone in REPL evaluation**
**Learning:** Functions that accept a mutable reference to a value (`&mut T`), mutate it internally, and then reassign back to it at the end can avoid deep cloning the value by using `std::mem::take` (if `T` implements `Default`). This temporarily replaces the referenced value with its default, moves the original value, and eliminates a full heap allocation.
**Action:** When a function takes ownership of a `&mut` parameter's inner value only to replace it before returning, prefer `std::mem::take` over `.clone()`.

**Remove `.collect::<Vec<_>>()` on hot paths**
**Learning:** `.collect()` calls during frequent queries like `query_mask` can cause unnecessary heap allocations, degrading pattern evaluation performance.
**Action:** Use slices of the originally collected vectors or directly iterate when checking overlap boundaries to keep evaluation O(N) without constant reallocations.
