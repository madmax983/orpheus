with open("crates/orpheus-lang/src/eval.rs", "r") as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if line.startswith("#[cfg(test)]"):
        test_start = i
        break

new_lines = lines[:test_start] + lines[-5:] + lines[test_start:-5]

with open("crates/orpheus-lang/src/eval.rs", "w") as f:
    f.writelines(new_lines)
