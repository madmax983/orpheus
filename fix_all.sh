#!/bin/bash
sed -i 's/let capacity = cycle_count.checked_mul(unit_events.len()).ok_or_else(|| {/let capacity = cycle_count.checked_mul(unit_events.len()).ok_or({/' crates/orpheus-pattern/src/cycle.rs
sed -i 's/pub fn with_builtins() -> Self {/#[allow(clippy::too_many_lines)]\n    pub fn with_builtins() -> Self {/' crates/orpheus-lang/src/types/env.rs
sed -i 's/pub const fn new(bindings: BTreeMap<String, Type>) -> Self {/#[must_use]\n    pub const fn new(bindings: BTreeMap<String, Type>) -> Self {/' crates/orpheus-lang/src/types/mod.rs
sed -i 's/fn list_midi_inputs(&self) -> Result<String, String> {/fn list_midi_inputs() -> Result<String, String> {/' crates/orpheus-lang/src/session.rs
sed -i 's/fn list_midi_outputs(&self) -> Result<String, String> {/fn list_midi_outputs() -> Result<String, String> {/' crates/orpheus-lang/src/session.rs
sed -i 's/\["in", "list"\] => self.list_midi_inputs(),/\["in", "list"\] => Self::list_midi_inputs(),/' crates/orpheus-lang/src/session.rs
sed -i 's/\["list"\] => self.list_midi_outputs(),/\["list"\] => Self::list_midi_outputs(),/' crates/orpheus-lang/src/session.rs
sed -i 's/buf\[i\] = ((val & 0x7F) | 0x80) as u8;/#[allow(clippy::cast_possible_truncation)]\n        let byte = ((val \& 0x7F) | 0x80) as u8;\n        buf[i] = byte;/' crates/orpheus-lang/src/midi_export.rs
sed -i 's/fn send_midi_binding(&mut self, binding_name: &str, channel: u8) -> Result<String, String> {/fn send_midi_binding(\&self, binding_name: \&str, channel: u8) -> Result<String, String> {/' crates/orpheus-lang/src/session.rs
sed -i 's/let note = event.value.round().clamp(0.0, 127.0) as u8;/#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]\n            let note = event.value.round().clamp(0.0, 127.0) as u8;/' crates/orpheus-lang/src/session.rs
sed -i 's/let note = event.value.round().clamp(0.0, 127.0) as u8;/#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]\n        let note = event.value.round().clamp(0.0, 127.0) as u8;/' crates/orpheus-lang/src/midi_export.rs
sed -i 's/file.write_all(&(track_data.len() as u32).to_be_bytes())/#[allow(clippy::cast_possible_truncation)]\n    let track_len = track_data.len() as u32;\n    file.write_all(\&track_len.to_be_bytes())/' crates/orpheus-lang/src/midi_export.rs
