## 2024-05-16 - 👺 Havoc: Addressed evaluation recursion limit
**The Trigger:** Extremely deep nesting of patterns (like `((((...))))`) or user recursive functions. The AST caps depth at 64, but AST macro expressions like `seq` and `group` were recursively walking without depth checks in the evaluator. The evaluator's depth tracking was only implemented for `apply_function_value` calls but completely ignored for `eval_expr_in_meter`, meaning AST nodes could cause an uncatchable stack overflow.
**The Stack Trace:** standard Rust `SIGABRT` stack overflow (10000+ frames of `eval_expr_in_meter`)
**Reproduction:** `notes = (((((...)))))`
**Comment:** You assumed the AST cap would protect the evaluator, but the evaluator can be called via macros or deep nesting not captured by the AST bounds (or users might supply synthetic trees). I've added a hard check at `eval_expr_in_meter` to stop the bleeding.
