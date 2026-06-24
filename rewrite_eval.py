import sys

def rewrite_eval_file(filepath):
    with open(filepath, 'r') as f:
        lines = f.readlines()

    start_idx = -1
    for i, line in enumerate(lines):
        if "fn eval_explicit_expr(" in line:
            start_idx = i
            break

    if start_idx == -1:
        return

    # We will just print the block to verify its length.
    brace_depth = 0
    in_func = False
    func_lines = []

    for i in range(start_idx, len(lines)):
        line = lines[i]
        func_lines.append(line)
        if "{" in line:
            in_func = True
        if in_func:
            brace_depth += line.count("{")
            brace_depth -= line.count("}")
            if brace_depth == 0:
                break

    print(f"eval_explicit_expr length: {len(func_lines)}")

rewrite_eval_file('crates/orpheus-lang/src/eval.rs')
