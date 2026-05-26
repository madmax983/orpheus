use orpheus_lang::ReplSession;
use orpheus_dsp::EngineHandle;
use std::sync::Arc;
use loom::sync::Mutex;

#[test]
fn test_havoc_repl_concurrency() {
    loom::model(|| {
        let (engine, _) = EngineHandle::split_for_test();
        let session = Arc::new(Mutex::new(ReplSession::with_engine(engine)));

        let s1 = session.clone();
        let t1 = loom::thread::spawn(move || {
            let _ = s1.lock().unwrap().eval_line("a = 1");
        });

        let s2 = session;
        let t2 = loom::thread::spawn(move || {
            let _ = s2.lock().unwrap().eval_line("b = 2");
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
