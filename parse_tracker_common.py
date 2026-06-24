import sys

with open('crates/orpheus-lang/src/tracker.rs', 'r') as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "let steps_per_cycle = 16_u32;" in line:
        print(f"found steps_per_cycle at {i+1}")
