import re

with open('crates/orpheus-lang/src/value.rs', 'r') as f:
    content = f.read()

content = content.replace("wrapping_mul(6364136223846793005)", "wrapping_mul(6_364_136_223_846_793_005)")
content = content.replace("wrapping_add(1442695040888963407)", "wrapping_add(1_442_695_040_888_963_407)")
content = content.replace("let j = (rng_state as usize) % (i + 1);", "#[allow(clippy::cast_possible_truncation)]\n            let j = usize::try_from(rng_state).unwrap_or(0) % (i + 1);")
content = content.replace("Some(event.whole.unwrap_or(event.part.clone()))", "Some(event.whole.unwrap_or_else(|| event.part.clone()))")

with open('crates/orpheus-lang/src/value.rs', 'w') as f:
    f.write(content)
