**Vector Reuse in Grouping Loops**
**Learning:** When grouping elements in a hot evaluation loop, allocating a new `Vec::new()` for each group causes redundant heap allocations. However, `extend(cluster.drain(..))` triggered a `clippy` lint (`clippy::extend-with-drain`). The idiomatic and performant way to transfer elements from one vector to another while retaining the capacity of the source vector for reuse is `dest_vec.append(&mut source_vec)`. This correctly avoids allocations across loop iterations.
**Action:** When gathering items in a loop to add to a larger collection, declare `let mut cluster = Vec::new()` outside the loop, `cluster.clear()` inside the loop, and use `dest.append(&mut cluster)` to move items without dropping the allocated capacity.

## 2025-05-18 - Reuse event cluster buffers
**Learning:** When grouping elements in a hot evaluation loop, allocating a new `Vec::new()` for each group causes redundant heap allocations. Hoisting the buffer allocation outside the loop and using `.clear()` reuses the capacity.
**Action:** When gathering items in a loop to add to a larger collection or process, declare `let mut cluster = Vec::new()` outside the loop, `cluster.clear()` inside the loop, and pass a slice `&cluster` or `&mut cluster` to helper functions instead of passing by value.

**Remove deep clone of bindings in type inference**
**Learning:** Found an unnecessary `BTreeMap::clone()` in `infer_into_bindings` that caused a full heap allocation and deep copy of the REPL environment on every inference pass. `inferencer.infer_statements(...)` returns a `Result`, so `?` cannot be used safely if we want to restore bindings upon error.
**Action:** Use `std::mem::take(bindings)` to move the `BTreeMap` into the `Inferencer` without allocating. Store the result of `infer_statements` in a local variable, unconditionally restore `*bindings = inferencer.user_bindings;`, and then return the result. This optimizes the hot REPL path without losing state on syntax/type errors.

**Use `Arc<str>` instead of `Box<str>` for deep immutability on hot paths**
**Learning:** In `orpheus-lang/src/value.rs`, the `SampleEvent` struct contained a `sample: Box<str>` field. Because pattern evaluation transforms (like `adjust_gain`, `adjust_pan`) clone the `SampleEvent` repeatedly on the hot path, `Box<str>` forces a deep memory allocation and string copy every time. By replacing `Box<str>` with `std::sync::Arc<str>`, the clone becomes a simple atomic increment. This significantly reduces heap allocations while maintaining thread safety (`Send + Sync`).
**Action:** When a struct containing strings is cloned repeatedly but the strings are never mutated, use `std::sync::Arc<str>` (or similar interning primitives) instead of `Box<str>` or `String` to avoid costly memory allocations.

## YYYY-MM-DD - [Remove intermediate heap allocations]
**Learning:** `try_fold` works perfectly for removing intermediate allocations but requires a bit of turbofish boilerplate (`Ok::<_, Error>(acc)`). A simple `for` loop with a pre-allocated vector achieves the exact same performance win with more readable syntax.
**Action:** Use simple `for` loops with pre-allocated vectors (`Vec::with_capacity()`) over `.try_fold()` when aggregating items that can fail to improve readability while maintaining the same performance.
