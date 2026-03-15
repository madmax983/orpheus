
**Pre-allocating Vectors on Hot Paths**
**Learning:** Vectors created dynamically inside hot evaluation loops (like control pattern evaluations) should be pre-allocated using Vec::with_capacity() to minimize heap reallocations.
**Action:** Always check the upper bound length of the source collection when mapping or filtering collections, and initialize Vec::with_capacity(len) accordingly.
