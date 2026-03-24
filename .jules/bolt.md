**Vector Reuse in Grouping Loops**
**Learning:** When grouping elements in a hot evaluation loop, allocating a new `Vec::new()` for each group causes redundant heap allocations. However, `extend(cluster.drain(..))` triggered a `clippy` lint (`clippy::extend-with-drain`). The idiomatic and performant way to transfer elements from one vector to another while retaining the capacity of the source vector for reuse is `dest_vec.append(&mut source_vec)`. This correctly avoids allocations across loop iterations.
**Action:** When gathering items in a loop to add to a larger collection, declare `let mut cluster = Vec::new()` outside the loop, `cluster.clear()` inside the loop, and use `dest.append(&mut cluster)` to move items without dropping the allocated capacity.

## 2025-05-18 - Reuse event cluster buffers
**Learning:** When grouping elements in a hot evaluation loop, allocating a new `Vec::new()` for each group causes redundant heap allocations. Hoisting the buffer allocation outside the loop and using `.clear()` reuses the capacity.
**Action:** When gathering items in a loop to add to a larger collection or process, declare `let mut cluster = Vec::new()` outside the loop, `cluster.clear()` inside the loop, and pass a slice `&cluster` or `&mut cluster` to helper functions instead of passing by value.

**Remove deep clone of bindings in type inference**
**Learning:** Found an unnecessary `BTreeMap::clone()` in `infer_into_bindings` that caused a full heap allocation and deep copy of the REPL environment on every inference pass. `inferencer.infer_statements(...)` returns a `Result`, so `?` cannot be used safely if we want to restore bindings upon error.
**Action:** Use `std::mem::take(bindings)` to move the `BTreeMap` into the `Inferencer` without allocating. Store the result of `infer_statements` in a local variable, unconditionally restore `*bindings = inferencer.user_bindings;`, and then return the result. This optimizes the hot REPL path without losing state on syntax/type errors.
