import sys

with open('crates/orpheus-lang/src/tracker.rs', 'r') as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "fn export_number_pattern_to_tracker" in line:
        print(f"found at {i+1}")
