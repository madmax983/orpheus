#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // We are testing Havoc: malformed MIDI messages should not panic
    let _ = std::panic::catch_unwind(|| {
        orpheus_lang::midi_input_update_from_message_for_fuzz(data);
    });
});
