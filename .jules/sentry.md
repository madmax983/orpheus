## 2025-02-27 - Rational Addition Overflow Test

**Learning:** `checked_add` in `Rational` explicitly panics with "rational addition overflowed during checked arithmetic" if an intermediate representation overflows `i128`. This explicitly checks bounds and aborts to avoid undefined behavior or incorrect scaling in exact fraction evaluations, but it lacked test coverage to guarantee the bounds checking branch actually panics correctly.
**Action:** Added `test_rational_addition_overflow_panics` and `test_rational_sub_overflow_reports_error` test methods inside `crates/orpheus-pattern/tests/rational_overflow.rs` to exercise this logic explicitly.
