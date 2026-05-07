import re

with open('crates/orpheus-dsp/src/voice.rs', 'r') as f:
    content = f.read()

# Fix next_drum_synth_sample args
search1 = """fn next_drum_synth_sample(
    kind: &VoiceKind,
    frame_index: &mut u32,"""
replace1 = """fn next_drum_synth_sample(
    kind: VoiceKind,
    frame_index: &mut u32,"""

# Fix call site
search2 = """            ActiveVoiceState::DrumSynth {
                kind,
                frame_index,
                duration_frames,
                sample_rate_hz,
                noise_state,
            } => next_drum_synth_sample(
                kind,
                frame_index,
                *duration_frames,
                *sample_rate_hz,
                noise_state,
            ),"""
replace2 = """            ActiveVoiceState::DrumSynth {
                kind,
                frame_index,
                duration_frames,
                sample_rate_hz,
                noise_state,
            } => next_drum_synth_sample(
                *kind,
                frame_index,
                *duration_frames,
                *sample_rate_hz,
                noise_state,
            ),"""

# Fix next_analog_synth_sample allow
search3 = """fn next_analog_synth_sample("""
replace3 = """#[allow(clippy::cast_possible_truncation)]
fn next_analog_synth_sample("""

# Fix next_playback_sample arguments allow
search4 = """#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn next_playback_sample("""
replace4 = """#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn next_playback_sample("""


content = content.replace(search1, replace1)
content = content.replace(search2, replace2)
content = content.replace(search3, replace3)
content = content.replace(search4, replace4)

with open('crates/orpheus-dsp/src/voice.rs', 'w') as f:
    f.write(content)

print("Done")
