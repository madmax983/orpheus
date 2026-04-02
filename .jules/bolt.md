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

## 2025-05-18 - Zero-cost Event Fragment Boundaries
**Learning:** Refactoring a function to accept a generic iterator (like `impl Iterator<Item = &'a TimeSpan>`) instead of a slice of slices can eliminate upstream heap allocations (like `.collect::<Vec<_>>()`). However, it is crucial to properly thread the `capacity_estimate` to `Vec::with_capacity` inside the refactored function, and keep the returned item's lifetimes bounded by `'a` (i.e. `Option<Vec<&'a Rational>>`). Modifying the mathematical logic of pseudo-random number generators by converting `(rng_state as usize)` to a "safe" `.try_from().unwrap_or(usize::MAX)` destroys the uniformity on 32-bit platforms.
**Action:** When removing `.collect::<Vec<_>>()`, change function signatures to use `impl Iterator`. Pass explicit `capacity_estimate` when the size of the iterator isn't easily accessible. Never "fix" LCG truncations with `try_from`—instead, suppress the lint locally via `#[allow(clippy::cast_possible_truncation)]`. Ensure `clippy::semicolon_if_nothing_returned` warnings are fixed by adding semicolons, not broad module-level suppressions.
