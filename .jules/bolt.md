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

**UserFn Allocation Optimization Correctly Handled**
**Learning:** `Arc::make_mut` copies the underlying data if the reference count is greater than 1. The original implementation resulted in performance regressions because it returned `&mut UserFn` requiring the internal values like BTreeMaps to be cloned on the hot path.
**Action:** Use `Arc::unwrap_or_clone` instead to consume the arc and regain ownership, dropping down to O(1) pointer copies when no concurrent use is present without adding cloning overhead in the function execution path.
