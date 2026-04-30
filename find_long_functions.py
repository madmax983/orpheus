import os
import re

for root, _, files in os.walk('crates'):
    for file in files:
        if not file.endswith('.rs'): continue
        path = os.path.join(root, file)
        with open(path, 'r') as f:
            lines = f.readlines()

        in_func = False
        start_line = 0
        func_name = ""
        brace_count = 0

        for i, line in enumerate(lines):
            if not in_func:
                m = re.match(r'^\s*(pub\s+|pub\(crate\)\s+|pub\(super\)\s+)?(const\s+)?(async\s+)?fn\s+([a-zA-Z0-9_]+)', line)
                if m:
                    in_func = True
                    start_line = i + 1
                    func_name = m.group(4)
                    brace_count = line.count('{') - line.count('}')
            else:
                brace_count += line.count('{') - line.count('}')
                if brace_count <= 0:
                    func_len = i + 1 - start_line
                    if func_len > 50:
                        print(f"{path}:{start_line} - {func_name} ({func_len} lines)")
                    in_func = False
