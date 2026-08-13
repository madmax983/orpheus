//! Havoc property-based testing and fuzzing targets for the `orpheus-dsp` crate to uncover panics or math anomalies.

use loom::sync::Arc;
use loom::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use loom::thread;

#[derive(Debug, Default)]
struct SharedTransportLoom {
    publish_epoch: AtomicU64,
    current_frame: AtomicU64,
    current_cycle_start_frame: AtomicU64,
    is_poisoned: AtomicBool,
}

struct PublishGuardLoom<'a> {
    transport: &'a SharedTransportLoom,
    completed: bool,
}

impl Drop for PublishGuardLoom<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.transport.is_poisoned.store(true, Ordering::Release);
        }
    }
}

impl SharedTransportLoom {
    fn new() -> Self {
        Self {
            publish_epoch: AtomicU64::new(0),
            current_frame: AtomicU64::new(0),
            current_cycle_start_frame: AtomicU64::new(0),
            is_poisoned: AtomicBool::new(false),
        }
    }

    fn publish(&self, current_frame: u64, current_cycle_start_frame: u64) {
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
        loom::sync::atomic::fence(Ordering::Release);

        let mut guard = PublishGuardLoom {
            transport: self,
            completed: false,
        };

        self.current_frame.store(current_frame, Ordering::Relaxed);
        self.current_cycle_start_frame
            .store(current_cycle_start_frame, Ordering::Relaxed);

        guard.completed = true;

        loom::sync::atomic::fence(Ordering::Release);
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64) {
        let mut spins = 0;
        loop {
            if self.is_poisoned.load(Ordering::Acquire) {
                return (0, 0); // fallback state
            }

            spins += 1;
            assert!(
                spins <= 10,
                "Livelock detected: spun too many times waiting for even epoch"
            );

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

#[test]
fn havoc_test_transport_panic_livelock() {
    loom::model(|| {
        let transport = Arc::new(SharedTransportLoom::new());

        let t1 = transport.clone();
        let _ = thread::spawn(move || {
            // Simulate panicking in the middle of publish
            t1.publish_epoch.fetch_add(1, Ordering::Relaxed);
            loom::sync::atomic::fence(Ordering::Release);

            let mut _guard = PublishGuardLoom {
                transport: &t1,
                completed: false,
            };

            t1.current_frame.store(100, Ordering::Relaxed);
            // thread dies here, _guard is dropped and sets is_poisoned
        })
        .join();

        thread::spawn(move || {
            // Because is_poisoned is set, this should immediately return the fallback (0, 0)
            // instead of livelocking.
            let (cf, ccsf) = transport.snapshot();
            assert_eq!(cf, 0);
            assert_eq!(ccsf, 0);
        })
        .join()
        .unwrap();
    });
}
