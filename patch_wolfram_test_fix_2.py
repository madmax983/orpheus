with open('crates/orpheus-lang/src/builtins.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if 'assert_eq!(events.len(), 3);' in line:
        new_lines.append('        assert_eq!(events.len(), 5);\n')
    else:
        new_lines.append(line)

with open('crates/orpheus-lang/src/builtins.rs', 'w') as f:
    f.writelines(new_lines)
