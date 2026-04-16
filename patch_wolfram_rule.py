with open('crates/orpheus-lang/src/builtins.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
in_wolfram = False
for line in lines:
    if 'fn apply_wolfram' in line:
        in_wolfram = True
        new_lines.append(line)
        continue

    if in_wolfram and 'Ok(Value::NumberPattern' in line:
        in_wolfram = False
        new_lines.append('''    // Implement Wolfram elementary cellular automaton
    let rule_num = rule as u8;
    let mut current_state = vec![false; steps as usize];
    if steps > 0 {
        current_state[steps as usize / 2] = true; // center pixel
    }

    let mut nodes = Vec::new();
    for _ in 0..steps {
        // Record current state
        for cell in &current_state {
            if *cell {
                nodes.push(orpheus_pattern::PatternNode::atom(1.0));
            } else {
                nodes.push(orpheus_pattern::PatternNode::rest());
            }
        }

        // Calculate next state
        let mut next_state = vec![false; steps as usize];
        for i in 0..steps as usize {
            let left = if i == 0 { false } else { current_state[i - 1] };
            let center = current_state[i];
            let right = if i == steps as usize - 1 { false } else { current_state[i + 1] };

            let neighborhood = ((left as u8) << 2) | ((center as u8) << 1) | (right as u8);
            next_state[i] = (rule_num & (1 << neighborhood)) != 0;
        }
        current_state = next_state;
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)))
''')
        continue

    if in_wolfram and '// Evaluate the automaton' in line:
        continue
    if in_wolfram and 'let mut current_state' in line:
        continue
    if in_wolfram and 'if steps > 0 {' in line:
        continue
    if in_wolfram and 'current_state[' in line:
        continue
    if in_wolfram and '} else {' in line:
        continue
    if in_wolfram and 'nodes.push' in line:
        continue
    if in_wolfram and 'for i in 0..steps {' in line:
        continue
    if in_wolfram and 'let mut nodes = Vec::new();' in line:
        continue
    if in_wolfram and 'if current_state[i as usize] {' in line:
        continue
    if in_wolfram and '}' in line and 'let mut' not in line and 'steps' not in line and 'current' not in line:
        pass # keep parsing

    if in_wolfram:
        pass # Skip the old implementation lines
    else:
        new_lines.append(line)

with open('crates/orpheus-lang/src/builtins.rs', 'w') as f:
    f.writelines(new_lines)
