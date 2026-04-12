## 2024-04-12 - Eliminate intermediate allocations during string serialization
**Learning:** Mapping iterators to `String` and then collecting them into a `Vec<String>` just to `join` them generates `n + 1` heap allocations.
**Action:** Use `String::with_capacity` and `std::fmt::Write::write_fmt` via the `write!` macro to format and append strings directly into a single buffer, reducing total allocations to 1.
