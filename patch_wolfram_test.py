with open('crates/orpheus-lang/src/builtins.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if 'use crate::eval::{ReplMode, eval_module};' in line:
        new_lines.append('    use crate::{ReplMode, eval_module};\n')
        new_lines.append('    use orpheus_pattern::Rational;\n')
    elif 'let span = orpheus_pattern::TimeSpan::from_cycles(0.0, 1.0).unwrap();' in line:
        new_lines.append('        let span = orpheus_pattern::TimeSpan::new(Rational::zero(), Rational::one()).unwrap();\n')
    elif 'use super::*;' in line:
        pass
    else:
        new_lines.append(line)

with open('crates/orpheus-lang/src/builtins.rs', 'w') as f:
    f.writelines(new_lines)
