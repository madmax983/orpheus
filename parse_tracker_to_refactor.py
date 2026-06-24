import sys
with open('crates/orpheus-lang/src/tracker.rs', 'r') as f:
    text = f.read()

def find_block(text, start_pattern, end_pattern):
    start = text.find(start_pattern)
    if start == -1: return None
    end = text.find(end_pattern, start)
    if end == -1: return None
    return text[start:end+len(end_pattern)]

sample_grid = find_block(text, "let mut grid: Vec<Vec<Option<String>>>", "        }")
print(sample_grid)
