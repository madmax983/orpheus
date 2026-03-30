with open("crates/orpheus-dsp/src/engine.rs", "r") as f:
    data = f.read()

old_block = """        // Start the write transaction. Relaxed is sufficient because the
        // atomic fence handles the required release semantics.
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);"""

new_block = """        // Start the write transaction. Relaxed is sufficient because the
        // atomic fence handles the required release semantics.
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);"""

# Already using acquire/release but wait.. Let's check the real code

import sys
