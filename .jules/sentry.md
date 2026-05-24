## 2024-05-24 - [Clippy Needless Range Loop]
**Learning:** Using a range loop exclusively to index into a collection (e.g., `for i in 50..55 { frames[i] = 1.0; }`) triggers the `-D clippy::needless-range-loop` lint.
**Action:** Use iterators over slices (e.g., `for frame in frames.iter_mut().take(55).skip(50) { *frame = 1.0; }`) instead of range-based indexing to comply with strict clippy constraints.
