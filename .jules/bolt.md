**[AST Cloning Bottlenecks]**
**Learning:** Extracting specific keys from owned module environments (like `type_bindings` or `value_bindings` in `orpheus-lang`) using `.get(&key).cloned()` forces expensive deep copies of complex, heavily-nested AST structures (`Type` and `Value`).
**Action:** When transferring ownership of specific items out of a uniquely owned collection (e.g., an imported module during loading), use `.remove(&key)` instead of cloning.

**[Intermediate Iteration Collect]**
**Learning:** Chaining `.iter().map().collect()` to produce a `Vec` where size is known (like `layers.len()`) forces an intermediate collection vector if the resulting items could be pre-allocated properly. Although `collect()` may reserve capacity, manually pre-allocating a `Vec::with_capacity` and pushing reduces overhead when dealing with fallible operations (`Result`).
**Action:** When evaluating child nodes into a collection, use a pre-allocated vector and a simple loop to reduce intermediate heap allocation overhead.
**[Reserve Vector Capacity]**
**Learning:** Appending items to a `Vec` inside a loop on hot paths (like Orpheus's evaluator) without pre-allocating capacity causes redundant heap re-allocations. In `eval.rs::append_unsorted_shifted`, `combined.push(new_event)` is called `base_events.len()` times but capacity isn't reserved.
**Action:** Always use `.reserve(len)` before the loop when the exact number of elements to be added is known.
**[Event Cloning Bottlenecks]**
**Learning:** When creating a modified struct from an old one (like Orpheus's `Event`), using `let mut new_event = event.clone()` followed by mutations incurs a full, expensive deep copy of all fields, including complex payload enums.
**Action:** Reconstruct the struct explicitly by transferring ownership or cloning only the specific fields required (e.g., `value: event.value.clone()`) while using references to generate new values for other fields, to eliminate unnecessary deep cloning.
