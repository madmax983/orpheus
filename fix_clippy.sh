#!/bin/bash
sed -i 's/let mut values = match eval_module(&source, ReplMode::Loose) {/let Ok(mut values) = eval_module(\&source, ReplMode::Loose) else { return };/g' crates/orpheus-lang/tests/fuzz_eval.rs
sed -i '/Ok(v) => v,/d' crates/orpheus-lang/tests/fuzz_eval.rs
sed -i '/Err(_) => return, \/\/ parse errors and eval errors on f/d' crates/orpheus-lang/tests/fuzz_eval.rs
sed -i 's/};\/\/let Ok(mut values)/;/g' crates/orpheus-lang/tests/fuzz_eval.rs

cat << 'INNER_EOF' > /tmp/merge.diff
<<<<<<< SEARCH
            let mut values = match eval_module(&source, ReplMode::Loose) {
                Ok(v) => v,
                Err(_) => return, // parse errors and eval errors on f
            };
            let val = match values.remove("a") {
                Some(v) => v,
                None => return,
            };
            let pat = match val.as_sample_pattern() {
                Some(p) => p,
                None => return,
            };
=======
            let Ok(mut values) = eval_module(&source, ReplMode::Loose) else { return };
            let Some(val) = values.remove("a") else { return };
            let Some(pat) = val.as_sample_pattern() else { return };
>>>>>>> REPLACE
INNER_EOF
