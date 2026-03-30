## 2024-03-25 - Replace tuple with Structs in bus FX parsing
**Learning:** Returning multiple primitive values from a parsing function (e.g. `(Rational, f32, f32)`) creates "primitive obsession" / "Boolean blindness" where positional arguments are easily confused. Returning a clearly named struct improves type safety and readability.
**Action:** Identify and replace anonymous tuples with explicitly named structs containing named fields when extracting configuration logic from command-line arguments.
