**[Rational to Float Conversion]**
**Learning:** Found repetitive logic for converting a `Rational` to `f64` embedded in business logic inside `crates/orpheus-lang/src/eval.rs`, creating a "Boolean Blindness"-like clutter of casting logic.
**Action:** Extract conversions to `impl From<&Rational> for f64` (and its owned counterpart) inside `crates/orpheus-pattern/src/rational.rs`, keeping business logic clean and typed.
