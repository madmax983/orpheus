import re

with open('crates/orpheus-dsp/src/effects/delay.rs', 'r') as f:
    content = f.read()

content = content.replace("        state.process_frame(1.0, 1.0);", "        let _ = state.process_frame(1.0, 1.0);")

with open('crates/orpheus-dsp/src/effects/delay.rs', 'w') as f:
    f.write(content)

with open('crates/orpheus-dsp/src/effects/reverb.rs', 'r') as f:
    content = f.read()

content = content.replace("        state.process_frame(1.0, 1.0);", "        let _ = state.process_frame(1.0, 1.0);")

with open('crates/orpheus-dsp/src/effects/reverb.rs', 'w') as f:
    f.write(content)
