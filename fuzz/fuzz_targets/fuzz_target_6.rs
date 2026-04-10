#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    orpheus_lang::midi_input::update_from_message(data);
});
