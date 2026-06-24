import re

with open("crates/orpheus-lang/src/tracker.rs", "r") as f:
    tracker = f.read()

# I will refactor `export_sample_pattern_to_tracker` to extract formatting
# However, value.rs is the main target as identified in forge.md

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    value_rs = f.read()

import re
matches = re.findall(r'#\[allow\(clippy::too_many_lines\)\]\n\s*fn try_query_', value_rs)
print(len(matches))
