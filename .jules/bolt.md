**Optimization: Removed intermediate  on hot paths.**
**Learning:** Chaining  creates a temporary heap allocation for both the Vector and the formatted strings inside it, which is especially unnecessary when iterating over iterators of strings that just need joining.
**Action:** Replaced these chains with direct buffered writing into a pre-allocated  using a loop, which significantly reduces allocation overhead.
**[String Joining Optimization]**
**Learning:** Chaining `.collect::<Vec<_>>().join(...)` creates a temporary heap allocation for both the Vector and the strings inside it.
**Action:** Replaced these chains with direct buffered writing into a pre-allocated `String::with_capacity()` using a loop to eliminate intermediate vector allocations.
