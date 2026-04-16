with open('crates/orpheus-lang/src/builtins.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if 'let rule_num = rule as u8;' in line:
        new_lines.append('    #[allow(clippy::cast_possible_truncation)]\n')
        new_lines.append(line)
    elif 'let neighborhood = ((left as u8) << 2) | ((center as u8) << 1) | (right as u8);' in line:
        new_lines.append('            let neighborhood = (u8::from(left) << 2) | (u8::from(center) << 1) | u8::from(right);\n')
    else:
        new_lines.append(line)

with open('crates/orpheus-lang/src/builtins.rs', 'w') as f:
    f.writelines(new_lines)
