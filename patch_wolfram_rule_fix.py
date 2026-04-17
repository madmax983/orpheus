with open('crates/orpheus-lang/src/builtins.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
in_wolfram = False
for line in lines:
    if 'fn apply_wolfram' in line:
        in_wolfram = True
        new_lines.append(line)
        new_lines.append('''    let mut args = args.into_iter();
    let rule = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`wolfram` requires a rule argument"))?,
        "`wolfram` rule",
        false,
    )?;
    let steps = extract_whole_number(
        args.next()
            .ok_or_else(|| EvalError::new("`wolfram` requires a steps argument"))?,
        "`wolfram` steps",
        true,
    )?;
''')
        continue

    if in_wolfram and 'let rule_num = rule as u8;' in line:
        in_wolfram = False

    if in_wolfram:
        continue

    new_lines.append(line)

with open('crates/orpheus-lang/src/builtins.rs', 'w') as f:
    f.writelines(new_lines)
