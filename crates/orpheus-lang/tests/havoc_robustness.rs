#[cfg(test)]
mod tests {
    use orpheus_lang::{ReplMode, eval_module};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_havoc_eval_proptest(s in "\\PC*") {
            let _ = eval_module(&s, ReplMode::Loose);
        }
    }

    #[test]
    fn test_havoc_loom_session() {
        loom::model(|| {
            let mut env = std::collections::BTreeMap::new();
            let _ = orpheus_lang::eval_into_bindings("a = 1", ReplMode::Loose, &mut env);
        });
    }

    #[test]
    fn test_havoc_midi_output_deadlock() {
        loom::model(|| {
            use loom::thread;
            use std::sync::{Arc, Mutex};

            let connection = Arc::new(Mutex::new(0));
            let conn2 = connection.clone();

            let t1 = thread::spawn(move || {
                if let Ok(mut guard) = conn2.lock() {
                    *guard += 1;
                }
            });

            let t2 = thread::spawn(move || {
                if let Ok(mut guard) = connection.lock() {
                    *guard += 1;
                }
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
