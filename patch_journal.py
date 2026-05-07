import re

with open('.jules/forge.md', 'r') as f:
    content = f.read()

new_learning = """
**[Extracting Match Arms from God Functions]**
**Learning:** `next_mono_sample` in `voice.rs` had grown into a God Function with over 100 lines by placing complex, distinct generation logic (drums, analog synthesis, sample playback) directly inside the match arms of an `ActiveVoiceState` enum. This made the function hard to read and navigate.
**Action:** Extract each match arm's internal logic into its own focused helper function (e.g., `next_drum_synth_sample`, `next_analog_synth_sample`). Pass only the destructured variables needed for execution from the enum rather than trying to pass `&mut self` to avoid borrow checker conflicts.

"""

with open('.jules/forge.md', 'w') as f:
    f.write(content + new_learning)

print("Done")
