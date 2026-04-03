**Vec<&Rational> instead of Vec<Rational>**
**Learning:** Returning `Vec<Rational>` to bypass the borrow checker introduces O(N) `.clone()` overhead on hot execution paths, even if the underlying `Rational` type is small.
**Action:** Use multi-lifetime generic bounds (e.g., `'a, 'b, 'c`) on helper functions returning references to match the input iterator's lifetime and avoid falling back to owned copies. Take slices (`&[Event<T>]`) instead of owned vectors (`Vec<Event<T>>`) in callers where elements are iterated over and selectively cloned or modified, ensuring borrowed lifetimes remain valid during processing.

**Merge open spans via IntoIterator instead of Vec collection**
**Learning:** Functions designed to aggregate or merge ordered sequences (like `merge_open_spans`) shouldn't force callers to `.collect()` intermediate lists. `try_query` naturally outputs ordered slices. Intermediate allocations just to satisfy `Vec<T>` function arguments inflate allocation profiles.
**Action:** Refactor collection aggregators to accept `I: Iterator<Item = T>`, removing unnecessary `<Vec<_>>()` boundaries, providing zero-cost sequential evaluation passes without extra heap allocations on evaluation hot paths.
