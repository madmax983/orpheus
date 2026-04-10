import re

with open('crates/orpheus-lang/src/midi_input.rs', 'r') as f:
    content = f.read()

# Fix cc_normalized
pattern1 = r"    if let Some\(atomic_val\) = state\(\)\.cc_values\.get\(controller as usize\) \{\n        let raw = atomic_val\.load\(Ordering::Relaxed\);\n        f64::from\(raw\) \/ 127\.0\n    \} else \{\n        0\.0\n    \}"
replacement1 = r"""    state().cc_values.get(controller as usize).map_or(0.0, |atomic_val| {
        let raw = atomic_val.load(Ordering::Relaxed);
        f64::from(raw) / 127.0
    })"""
content = re.sub(pattern1, replacement1, content)

# Fix drain_note_events
pattern2 = r"    if let Ok\(mut queue\) = state\(\)\.note_events\.lock\(\) \{\n        queue\.drain\(\.\.\)\.collect\(\)\n    \} else \{\n        Vec::new\(\)\n    \}"
replacement2 = r"    state().note_events.lock().map_or_else(|_| Vec::new(), |mut queue| queue.drain(..).collect())"
content = re.sub(pattern2, replacement2, content)

with open('crates/orpheus-lang/src/midi_input.rs', 'w') as f:
    f.write(content)
