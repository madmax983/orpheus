import re

with open('crates/orpheus-lang/src/session.rs', 'r') as f:
    content = f.read()

content = content.replace("    fn export_pattern_value(\n        value: &Value,", "    #[allow(clippy::too_many_lines)]\n    fn export_pattern_value(\n        value: &Value,")

with open('crates/orpheus-lang/src/session.rs', 'w') as f:
    f.write(content)

with open('crates/orpheus-lang/src/value.rs', 'r') as f:
    content = f.read()

content = content.replace("fn apply_event_fragments<'a, T, F, I>(\n    source_events: &'a [Event<T>],\n    control_parts: I,", "#[allow(clippy::needless_pass_by_value, unused_mut)]\nfn apply_event_fragments<'a, T, F, I>(\n    source_events: &'a [Event<T>],\n    mut control_parts: I,")

with open('crates/orpheus-lang/src/value.rs', 'w') as f:
    f.write(content)
