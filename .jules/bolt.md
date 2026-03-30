**Vector Reuse in Grouping Loops**
**Learning:** When grouping elements in a hot evaluation loop, allocating a new `Vec::new()` for each group causes redundant heap allocations. However, `extend(cluster.drain(..))` triggered a `clippy` lint (`clippy::extend-with-drain`). The idiomatic and performant way to transfer elements from one vector to another while retaining the capacity of the source vector for reuse is `dest_vec.append(&mut source_vec)`. This correctly avoids allocations across loop iterations.
**Action:** When gathering items in a loop to add to a larger collection, declare `let mut cluster = Vec::new()` outside the loop, `cluster.clear()` inside the loop, and use `dest.append(&mut cluster)` to move items without dropping the allocated capacity.

**Remove unnecessary BTreeMap clone in REPL evaluation**
**Learning:** Functions that accept a mutable reference to a value (`&mut T`), mutate it internally, and then reassign back to it at the end can avoid deep cloning the value by using `std::mem::take` (if `T` implements `Default`). This temporarily replaces the referenced value with its default, moves the original value, and eliminates a full heap allocation.
**Action:** When a function takes ownership of a `&mut` parameter's inner value only to replace it before returning, prefer `std::mem::take` over `.clone()`.

**Remove `.collect::<Vec<_>>()` on hot paths**
**Learning:** `.collect()` calls during frequent queries like `query_mask` can cause unnecessary heap allocations, degrading pattern evaluation performance.
**Action:** Use slices of the originally collected vectors or directly iterate when checking overlap boundaries to keep evaluation O(N) without constant reallocations.
