use loom::thread;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Mock the core struct
struct EngineCore {
    current_frame: u64,
    current_cycle_start_frame: u64,
    frames_per_cycle: u64,
    tempo_bpm: f32,
    is_playing: bool,
    pending_pattern: Option<()>,
}

// Mirror the SharedTransport struct but with loom Atomics
struct SharedTransport {
    publish_epoch: loom::sync::atomic::AtomicU64,
    current_frame: loom::sync::atomic::AtomicU64,
    current_cycle_start_frame: loom::sync::atomic::AtomicU64,
    frames_per_cycle: loom::sync::atomic::AtomicU64,
    tempo_bpm_bits: loom::sync::atomic::AtomicU32,
    is_playing: loom::sync::atomic::AtomicBool,
    has_pending_pattern: loom::sync::atomic::AtomicBool,
}

impl SharedTransport {
    fn new() -> Self {
        Self {
            publish_epoch: loom::sync::atomic::AtomicU64::new(0),
            current_frame: loom::sync::atomic::AtomicU64::new(0),
            current_cycle_start_frame: loom::sync::atomic::AtomicU64::new(0),
            frames_per_cycle: loom::sync::atomic::AtomicU64::new(0),
            tempo_bpm_bits: loom::sync::atomic::AtomicU32::new(0),
            is_playing: loom::sync::atomic::AtomicBool::new(false),
            has_pending_pattern: loom::sync::atomic::AtomicBool::new(false),
        }
    }

    fn publish(&self, core: &EngineCore) {
        // We must increment the epoch FIRST, and make sure it is visible
        // BEFORE the payload writes. Using SeqCst for testing the logic easily.
        self.publish_epoch.fetch_add(1, Ordering::SeqCst);

        self.current_frame
            .store(core.current_frame, Ordering::SeqCst);
        self.current_cycle_start_frame
            .store(core.current_cycle_start_frame, Ordering::SeqCst);
        self.frames_per_cycle
            .store(core.frames_per_cycle, Ordering::SeqCst);
        self.tempo_bpm_bits
            .store(core.tempo_bpm.to_bits(), Ordering::SeqCst);
        self.is_playing.store(core.is_playing, Ordering::SeqCst);
        self.has_pending_pattern
            .store(core.pending_pattern.is_some(), Ordering::SeqCst);

        // Increment epoch to even to signal done.
        self.publish_epoch.fetch_add(1, Ordering::SeqCst);
    }

    fn snapshot(&self) -> TransportSnapshot {
        loop {
            let start_epoch = self.publish_epoch.load(Ordering::SeqCst);
            if start_epoch % 2 != 0 {
                loom::sync::atomic::spin_loop_hint();
                continue;
            }

            let snap = TransportSnapshot {
                publish_epoch: start_epoch,
                current_frame: self.current_frame.load(Ordering::SeqCst),
                current_cycle_start_frame: self.current_cycle_start_frame.load(Ordering::SeqCst),
                frames_per_cycle: self.frames_per_cycle.load(Ordering::SeqCst),
                tempo_bpm_bits: self.tempo_bpm_bits.load(Ordering::SeqCst),
                is_playing: self.is_playing.load(Ordering::SeqCst),
                has_pending_pattern: self.has_pending_pattern.load(Ordering::SeqCst),
            };

            let end_epoch = self.publish_epoch.load(Ordering::SeqCst);
            if start_epoch == end_epoch {
                return snap;
            }
        }
    }
}

pub struct TransportSnapshot {
    pub publish_epoch: u64,
    pub current_frame: u64,
    pub current_cycle_start_frame: u64,
    pub frames_per_cycle: u64,
    pub tempo_bpm_bits: u32,
    pub is_playing: bool,
    pub has_pending_pattern: bool,
}

#[test]
fn havoc_transport_torn_read() {
    loom::model(|| {
        let transport = Arc::new(SharedTransport::new());

        let t1 = transport.clone();
        let writer = thread::spawn(move || {
            let core = EngineCore {
                current_frame: 1000,
                current_cycle_start_frame: 500,
                frames_per_cycle: 100,
                tempo_bpm: 120.0,
                is_playing: true,
                pending_pattern: None,
            };
            t1.publish(&core);
        });

        let reader = thread::spawn(move || {
            let snap = transport.snapshot();
            if snap.is_playing {
                assert_eq!(snap.current_frame, 1000, "TORN READ OBSERVED!");
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    });
}
