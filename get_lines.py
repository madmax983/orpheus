import sys

def get_block(start_marker, end_marker, filepath):
    with open(filepath, 'r') as f:
        lines = f.readlines()
    start_idx = -1
    for i, line in enumerate(lines):
        if start_marker in line:
            start_idx = i
            break
    if start_idx == -1: return None
    end_idx = -1
    for i in range(start_idx, len(lines)):
        if end_marker in lines[i]:
            end_idx = i
            break
    if end_idx == -1: return None
    return lines[start_idx:end_idx+1]

print("".join(get_block("pub fn whole_number_from_f64", "}\n", "crates/orpheus-lang/src/builtins.rs")))
