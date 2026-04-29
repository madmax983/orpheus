use loom::sync::Arc;
use loom::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use loom::thread;

#[derive(Debug, Default)]
struct SharedTransportLoom {
    publish_epoch: AtomicU64,
    current_frame: AtomicU64,
    current_cycle_start_frame: AtomicU64,
    poisoned: AtomicBool,
}

struct PublishGuard<'a> {
    transport: &'a SharedTransportLoom,
}

impl Drop for PublishGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.transport.poisoned.store(true, Ordering::Relaxed);
        }
        loom::sync::atomic::fence(Ordering::Release);
        self.transport.publish_epoch.fetch_add(1, Ordering::Relaxed);
    }
}

impl SharedTransportLoom {
    fn new() -> Self {
        Self {
            publish_epoch: AtomicU64::new(0),
            current_frame: AtomicU64::new(0),
            current_cycle_start_frame: AtomicU64::new(0),
            poisoned: AtomicBool::new(false),
        }
    }

    fn publish(&self, current_frame: u64, current_cycle_start_frame: u64) {
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
        loom::sync::atomic::fence(Ordering::Release);

        let _guard = PublishGuard { transport: self };

        self.current_frame.store(current_frame, Ordering::Relaxed);
        self.current_cycle_start_frame
            .store(current_cycle_start_frame, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64) {
        loop {
            if self.poisoned.load(Ordering::Relaxed) {
                return (0, 0); // safe fallback
            }

            let start_epoch = self.publish_epoch.load(Ordering::Relaxed);
            loom::sync::atomic::fence(Ordering::Acquire);

            if !start_epoch.is_multiple_of(2) {
                loom::thread::yield_now();
                continue;
            }

            let cf = self.current_frame.load(Ordering::Relaxed);
            let ccsf = self.current_cycle_start_frame.load(Ordering::Relaxed);

            loom::sync::atomic::fence(Ordering::Acquire);
            let end_epoch = self.publish_epoch.load(Ordering::Relaxed);

            if start_epoch == end_epoch {
                if self.poisoned.load(Ordering::Relaxed) {
                    return (0, 0);
                }
                return (cf, ccsf);
            }
        }
    }
}

#[test]
fn havoc_test_transport() {
    loom::model(|| {
        let transport = Arc::new(SharedTransportLoom::new());

        let t1 = transport.clone();
        thread::spawn(move || {
            t1.publish(100, 100);
        });

        thread::spawn(move || {
            let (cf, ccsf) = transport.snapshot();
            assert!(
                (cf == 0 && ccsf == 0) || (cf == 100 && ccsf == 100),
                "Torn read detected: cf={cf}, ccsf={ccsf}"
            );
        });
    });
}
