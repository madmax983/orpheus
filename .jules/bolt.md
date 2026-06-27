**[FunctionValue Enum Cloning]**
**Learning:** `FunctionValue` enum contains variants with large inner contents (`Arc<UserFn>` with BTreeMaps and `Vec<String>`, or `BuiltinFn` with `Vec<Value>`). Passing it around by value forces deep clones in hot loops like pattern evaluation `apply_unary_transform`.
**Action:** Use a reference-based evaluation path (`apply_function_value_ref`) for such enums inside evaluation loops so that dispatching the method uses `&FunctionValue`, avoiding unnecessary allocations.
