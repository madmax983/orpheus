**Iterator-based Zero-Cost Boundaries**
**Learning:** Functions that accept a slice of slices `&[&[T]]` force the caller to collect intermediate iterators into a `Vec` just to pass them by reference. This causes hidden, unnecessary O(N) allocations in hot paths like pattern evaluation.
**Action:** Always prefer `impl Iterator<Item = ...>` over `&[&[T]]` or `&[T]` for internal helper functions, allowing callers to pass chained iterators (`.iter().flat_map(...)`) and achieve zero-cost execution without intermediate heap allocations.
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

**[Drain Iterator to Avoid Clones]**
**Learning:** Calling `.cloned()` on an iterator over strings creates heap allocations. If the strings are no longer needed in the source container (like `remaining_params`), draining the items directly (`drain(..applied)`) allows you to move ownership without deep copies.
**Action:** Replace `iter().take(n).cloned()` with `drain(..n)` when transferring elements from a mutable vector.

**Optimize allocations in `eval_call_with_args` and iterator aggregation in `eval.rs`**
**Learning:** `eval_pipe` unnecessarily allocated a `vec![lhs_value]` to pass as `piped_args` which then underwent `.extend()` causing potential reallocations. Also, iterator chains like `.collect::<Result<Option<Vec<_>>, _>>()` can hide intermediate allocations and make short-circuiting logic opaque.
**Action:** Replaced `piped_args: Vec<Value>` with `piped_arg: Option<Value>` in `eval_call_with_args` and allocated the vector with exact capacity `Vec::with_capacity`. Converted `.collect()` chains to simple `for` loops with pre-allocated vectors to eliminate aggregation overhead and turbofish boilerplate.

**In-place mutation for vector clustering**
**Learning:** Functions that cluster or modify contiguous groups of events (like `strum`, `invert`, and `drop`) often allocate new vectors (`Vec::with_capacity(events.len())`) to accumulate the modified slices using `.extend_from_slice()`. Since the original `events` vector is passed by value and the element count remains the same, we can mutate the clusters in place, avoiding a redundant O(N) allocation entirely.
**Action:** When a function accepts a `Vec<T>` by value and modifies its contents without changing its length, modify the elements in place instead of allocating a new intermediate vector to collect the modified slices.
