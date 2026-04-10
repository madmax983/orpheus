**Vec<&Rational> instead of Vec<Rational>**
**Learning:** Returning `Vec<Rational>` to bypass the borrow checker introduces O(N) `.clone()` overhead on hot execution paths, even if the underlying `Rational` type is small.
**Action:** Use multi-lifetime generic bounds (e.g., `'a, 'b, 'c`) on helper functions returning references to match the input iterator's lifetime and avoid falling back to owned copies. Take slices (`&[Event<T>]`) instead of owned vectors (`Vec<Event<T>>`) in callers where elements are iterated over and selectively cloned or modified, ensuring borrowed lifetimes remain valid during processing.

**Merge open spans via IntoIterator instead of Vec collection**
**Learning:** Functions designed to aggregate or merge ordered sequences (like `merge_open_spans`) shouldn't force callers to `.collect()` intermediate lists. `try_query` naturally outputs ordered slices. Intermediate allocations just to satisfy `Vec<T>` function arguments inflate allocation profiles.
**Action:** Refactor collection aggregators to accept `I: Iterator<Item = T>`, removing unnecessary `<Vec<_>>()` boundaries, providing zero-cost sequential evaluation passes without extra heap allocations on evaluation hot paths.

## YYYY-MM-DD - [Optimize Event Fragment Boundary Capacity Allocation]
**Learning:** Calling `.clone().count()` on iterators passed generically as `I: Iterator + Clone` forces an immediate O(N) evaluation simply to estimate capacity. Even when elements are references and cloning is cheap, iterating just to count introduces a measurable latency spike in deep processing pipelines like the DSP event fragments loop.
**Action:** Default to `iter.size_hint()` (specifically `let (lower, upper) = iter.size_hint(); upper.unwrap_or(lower)`) when pre-allocating `Vec::with_capacity` based on an iterator's bounds. This provides instant O(1) allocation bounds and drops the strict requirement for the iterator to be clonable, leading to cleaner signatures and fewer allocations.
**[Fixing Unnecessary Option Re-substitution]**
**Learning:** `clippy::or_fun_call` catches situations where closures substitute `Option::None` but are unnecessarily instantiated. Pre-allocation and avoiding `.clone()` calls significantly impacts garbage collector pauses and allocator waits during real-time rendering.
**Action:** Replace `ok_or_else(|| ... )` with `ok_or(...)` when the error fallback involves simple instantiations. Always run `cargo clippy --all-targets --all-features -- -D warnings` early and often.

**Eliminate `Rational` clones in event fragment boundaries**
**Learning:** Returning `Vec<Rational>` from `compute_event_fragment_boundaries` caused unnecessary `.clone()` calls simply to collect temporal bounds for sorting and deduplication. By refactoring `compute_event_fragment_boundaries` to store and return `Vec<&'a Rational>`, we avoid heap allocating owned clones for bounds that might immediately be discarded after deduplication or clipped during the `apply_event_fragments` window iteration.
**Action:** When collecting structs out of references into temporary Vecs for sorting or filtering, store `&T` instead of `.clone()`ing into `T`. Only clone or convert to owned values at the final step where the owned struct is specifically required.
