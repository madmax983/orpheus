import sys

with open("crates/orpheus-lang/src/builtins.rs", "r") as f:
    text = f.read()

# Replace apply_morse
old_apply_morse = """fn apply_morse(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let text = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`morse` requires a string argument"))?,
        "`morse` string",
    )?;

    let mut nodes = Vec::new();
    for ch in text.to_ascii_uppercase().chars() {
        let code = match ch {
            'A' => ".-", 'B' => "-...", 'C' => "-.-.", 'D' => "-..", 'E' => ".",
            'F' => "..-.", 'G' => "--.", 'H' => "....", 'I' => "..", 'J' => ".---",
            'K' => "-.-", 'L' => ".-..", 'M' => "--", 'N' => "-.", 'O' => "---",
            'P' => ".--.", 'Q' => "--.-", 'R' => ".-.", 'S' => "...", 'T' => "-",
            'U' => "..-", 'V' => "...-", 'W' => ".--", 'X' => "-..-", 'Y' => "-.--",
            'Z' => "--..", '0' => "-----", '1' => ".----", '2' => "..---", '3' => "...--",
            '4' => "....-", '5' => ".....", '6' => "-....", '7' => "--...", '8' => "---..",
            '9' => "----.", ' ' => " ", _ => "",
        };

        if code == " " {
            nodes.push(orpheus_pattern::PatternNode::rest());
            nodes.push(orpheus_pattern::PatternNode::rest());
            nodes.push(orpheus_pattern::PatternNode::rest());
        } else if !code.is_empty() {
            for symbol in code.chars() {
                if symbol == '.' {
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                } else if symbol == '-' {
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                }
                nodes.push(orpheus_pattern::PatternNode::rest());
            }
            // Inter-character space is 3 units (1 already added above, add 2 more)
            nodes.push(orpheus_pattern::PatternNode::rest());
            nodes.push(orpheus_pattern::PatternNode::rest());
        }
    }
    // Remove the very last inter-character space if there are nodes
    if !nodes.is_empty() {
        nodes.pop();
        nodes.pop();
        nodes.pop();
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_nodes(nodes)))
}"""

new_apply_morse = """fn apply_morse(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let text = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`morse` requires a string argument"))?,
        "`morse` string",
    )?;

    let mut nodes = Vec::new();
    for ch in text.to_ascii_uppercase().chars() {
        let code = match ch {
            'A' => ".-", 'B' => "-...", 'C' => "-.-.", 'D' => "-..", 'E' => ".",
            'F' => "..-.", 'G' => "--.", 'H' => "....", 'I' => "..", 'J' => ".---",
            'K' => "-.-", 'L' => ".-..", 'M' => "--", 'N' => "-.", 'O' => "---",
            'P' => ".--.", 'Q' => "--.-", 'R' => ".-.", 'S' => "...", 'T' => "-",
            'U' => "..-", 'V' => "...-", 'W' => ".--", 'X' => "-..-", 'Y' => "-.--",
            'Z' => "--..", '0' => "-----", '1' => ".----", '2' => "..---", '3' => "...--",
            '4' => "....-", '5' => ".....", '6' => "-....", '7' => "--...", '8' => "---..",
            '9' => "----.", ' ' => " ", _ => "",
        };

        if code == " " {
            nodes.push(orpheus_pattern::PatternNode::rest());
            nodes.push(orpheus_pattern::PatternNode::rest());
            nodes.push(orpheus_pattern::PatternNode::rest());
        } else if !code.is_empty() {
            for symbol in code.chars() {
                if symbol == '.' {
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                } else if symbol == '-' {
                    // A dash is a single continuous event that lasts 3 units.
                    // We can model this by putting an atom at the start of a group,
                    // and padding the group with rests so it takes 3x the duration.
                    // Actually, PatternNode doesn't have a direct "duration multiplier"
                    // but we can group them together in a way that respects relative lengths.
                    // Orpheus uses fractional subdivision based on the number of nodes in a group.
                    // If we want a dash to be 3x a dot, we need a common denominator.
                    // Actually, if we emit an event using `atom`, and then `~ ~`, they are three
                    // distinct events, one atom and two rests. The atom's duration is the same
                    // as the rest. But wait, we want a SINGLE tone to hold for 3 time units.
                    // In Orpheus, we can tie notes or we can just emit an atom.
                    // Let's create an `Event` manually if `PatternNode` cannot do ties? No, `PatternNode::atom` is all we have.
                    // If we use an envelope like `legato` later on the synth, the length matters.
                    // But `PatternNode::atom(1.0)` just means "trigger a 1.0 here for this duration".
                    // Wait, `PatternNode` creates events. If we want one event with a longer duration,
                    // it's not possible using just `PatternNode` without an explicit tie notation, which doesn't exist.
                    // However, we CAN just generate a `NumberPatternValue` manually without using `PatternNode`?
                    // Let's check how other things do it. `apply_hex` and `apply_wolfram` use `PatternNode::from_nodes`.
                    // Actually, `from_nodes` makes every node equal duration.
                    // The easiest way to get precise durations is to construct the pattern events directly if we can?
                    // But `NumberPatternValue::from_nodes` is convenient.
                    // A standard way in Tidal to make a note longer is `legato`.
                    // Wait, if a dash is just one `atom(1.0)` and its duration should be 3x a dot...
                    // In a sequence like `[1, ~, ~]`, the `1` lasts for 1 unit, followed by 2 units of rest. It does NOT last for 3 units.
                    // But if the synth has an envelope with decay, triggering a note without a note-off might sound correct if the next note is delayed.
                    // But the reviewer said: "A dash in Morse code is supposed to be a single, continuous tone that lasts for 3 time units. The implementation instead pushes three separate dots... The test assertion was adjusted to 15, contradicting the 9 events mentioned in the comment."
                    // So we must generate exactly 9 events.
                    // How can we generate exactly 9 events with `PatternNode` where a dash is 3x longer than a dot?
                    // A `Group` node divides time equally among its children.
                    // If we have a sequence of items, and we want them to have different durations, we can't easily do it with a flat `Vec` of `PatternNode`s since they all get the same duration.
                    // Wait, `PatternNode` has `from_events`? Let's check `NumberPatternValue` methods.
                    nodes.push(orpheus_pattern::PatternNode::atom(1.0));
                    nodes.push(orpheus_pattern::PatternNode::rest());
                    nodes.push(orpheus_pattern::PatternNode::rest());
                }
                nodes.push(orpheus_pattern::PatternNode::rest());
            }
            // Inter-character space is 3 units (1 already added above, add 2 more)
            nodes.push(orpheus_pattern::PatternNode::rest());
            nodes.push(orpheus_pattern::PatternNode::rest());
        }
    }
    // Remove the very last inter-character space if there are nodes
    if !nodes.is_empty() {
        nodes.pop();
        nodes.pop();
        nodes.pop();
    }

    // Now, we need to convert these nodes to events, and then tie the dashes?
    // Let's just output the `nodes` first. The reviewer complained about the NUMBER of events.
    // If a dash is `atom, rest, rest`, it produces exactly 1 event (the atom) which has a duration of 1 unit.
    // But wait! If the atom has a duration of 1 unit, the sound stops after 1 unit, it's not a continuous tone!
    // We need the dash event to have a duration of 3 units.
    // Let's manually construct the events!
}"""

# Actually, the reviewer specifically pointed out: "The implementation instead pushes three separate dots (`PatternNode::atom(1.0)`) in a row... The letter "O" (---) will play as 9 distinct dots... we need 9 events."
# Let's check what `NumberPatternValue` has. We can construct it from `events`!

text = text.replace(old_apply_morse, new_apply_morse)
with open("crates/orpheus-lang/src/builtins.rs", "w") as f:
    f.write(text)
