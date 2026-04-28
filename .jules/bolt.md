**[AST Cloning Bottlenecks]**
**Learning:** Extracting specific keys from owned module environments (like `type_bindings` or `value_bindings` in `orpheus-lang`) using `.get(&key).cloned()` forces expensive deep copies of complex, heavily-nested AST structures (`Type` and `Value`).
**Action:** When transferring ownership of specific items out of a uniquely owned collection (e.g., an imported module during loading), use `.remove(&key)` instead of cloning.

**[Intermediate Iteration Collect]**
**Learning:** Chaining `.iter().map().collect()` to produce a `Vec` where size is known (like `layers.len()`) forces an intermediate collection vector if the resulting items could be pre-allocated properly. Although `collect()` may reserve capacity, manually pre-allocating a `Vec::with_capacity` and pushing reduces overhead when dealing with fallible operations (`Result`).
**Action:** When evaluating child nodes into a collection, use a pre-allocated vector and a simple loop to reduce intermediate heap allocation overhead.
**[Reserve Vector Capacity]**
**Learning:** Appending items to a `Vec` inside a loop on hot paths (like Orpheus's evaluator) without pre-allocating capacity causes redundant heap re-allocations. In `eval.rs::append_unsorted_shifted`, `combined.push(new_event)` is called `base_events.len()` times but capacity isn't reserved.
**Action:** Always use `.reserve(len)` before the loop when the exact number of elements to be added is known.
**[sort_unstable_by instead of sort_by]**\n**Learning:** In Rust, `sort_by` allocates memory and requires extra overhead to guarantee that equal elements preserve their original relative order. For sorting pattern events in Orpheus, concurrent events often have no inherent order, making stable sorting unnecessary and slower on hot paths.\n**Action:** Use `sort_unstable_by` instead of `sort_by` for collections where the order of equal elements does not matter, to avoid allocations and reduce sorting overhead.
**[UserFn Allocation Optimization]**
**Learning:** Passing a large `UserFn` containing `BTreeMap` and AST structures by value in the AST evaluator (`apply_user_function`) and copying it multiple times across the type environment causes a massive number of deep clone allocations per function application, severely hitting performance.
**Action:** Use `Arc<UserFn>` within `FunctionValue::User` instead. If partial application is required, use `Arc::make_mut` to selectively mutate or clone only when the reference count is greater than 1, turning O(N) heap allocations into O(1) atomic increments in the hot path.
