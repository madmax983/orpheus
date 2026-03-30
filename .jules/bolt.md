**Remove unnecessary vector allocation for piped arguments**
**Learning:** `eval_call_with_args` previously took a `Vec<Value>` for `piped_args`, which forced an empty heap allocation for every standard function call (`eval_call`), and a 1-item allocation for every pipe (`eval_pipe`).
**Action:** Changed the signature to take an `Option<Value>` for the piped argument. This avoids allocating a vector purely to pass an argument that is either missing or singular, providing a zero-cost abstraction for the hot evaluation path.
