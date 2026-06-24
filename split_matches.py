with open("crates/orpheus-lang/src/value.rs", "r") as f:
    text = f.read()

# I will write a simple python script to find and replace the functions and their bodies

def get_block(text, start_str):
    start_idx = text.find(start_str)
    if start_idx == -1: return -1, -1

    brace_depth = 0
    in_block = False

    for i in range(start_idx, len(text)):
        if text[i] == '{':
            in_block = True
            brace_depth += 1
        elif text[i] == '}':
            brace_depth -= 1
            if in_block and brace_depth == 0:
                return start_idx, i + 1
    return -1, -1

# Let's inspect tracker.rs to extract common logic
with open("crates/orpheus-lang/src/tracker.rs", "r") as f:
    tracker_lines = f.readlines()
