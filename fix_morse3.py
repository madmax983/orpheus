import sys

with open("crates/orpheus-lang/src/builtins.rs", "r") as f:
    text = f.read()

# Replace apply_morse entirely with an event-based approach.
new_apply_morse = """fn apply_morse(args: Vec<Value>) -> Result<Value, EvalError> {
    let mut args = args.into_iter();
    let text = extract_string(
        args.next()
            .ok_or_else(|| EvalError::new("`morse` requires a string argument"))?,
        "`morse` string",
    )?;

    // Calculate total duration in units to establish the cycle length.
    // Dot = 1 unit. Dash = 3 units.
    // Inter-element gap (between dots and dashes) = 1 unit.
    // Short gap (between letters) = 3 units.
    // Medium gap (between words) = 7 units. (represented by space)

    let mut total_units = 0;
    let mut symbols_and_gaps = Vec::new(); // true for tone, false for silence, followed by length

    let chars: Vec<char> = text.to_ascii_uppercase().chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == ' ' {
            // A space between words means a medium gap of 7 units.
            // But if the previous character added an inter-character gap of 3,
            // we should subtract it and add 7 instead, or just add 4.
            // Let's just keep it simple: push a gap of 7.
            symbols_and_gaps.push((false, 7));
            total_units += 7;
            continue;
        }

        let code = match ch {
            'A' => ".-", 'B' => "-...", 'C' => "-.-.", 'D' => "-..", 'E' => ".",
            'F' => "..-.", 'G' => "--.", 'H' => "....", 'I' => "..", 'J' => ".---",
            'K' => "-.-", 'L' => ".-..", 'M' => "--", 'N' => "-.", 'O' => "---",
            'P' => ".--.", 'Q' => "--.-", 'R' => ".-.", 'S' => "...", 'T' => "-",
            'U' => "..-", 'V' => "...-", 'W' => ".--", 'X' => "-..-", 'Y' => "-.--",
            'Z' => "--..", '0' => "-----", '1' => ".----", '2' => "..---", '3' => "...--",
            '4' => "....-", '5' => ".....", '6' => "-....", '7' => "--...", '8' => "---..",
            '9' => "----.", _ => "",
        };

        if code.is_empty() {
            continue;
        }

        let code_chars: Vec<char> = code.chars().collect();
        for (j, &symbol) in code_chars.iter().enumerate() {
            if symbol == '.' {
                symbols_and_gaps.push((true, 1));
                total_units += 1;
            } else if symbol == '-' {
                symbols_and_gaps.push((true, 3));
                total_units += 3;
            }

            if j < code_chars.len() - 1 {
                // Inter-element gap
                symbols_and_gaps.push((false, 1));
                total_units += 1;
            }
        }

        if i < chars.len() - 1 && chars[i + 1] != ' ' {
            // Inter-character gap
            symbols_and_gaps.push((false, 3));
            total_units += 3;
        }
    }

    if total_units == 0 {
        return Ok(Value::NumberPattern(NumberPatternValue::from_events(vec![])));
    }

    let mut events = Vec::new();
    let mut current_pos = orpheus_pattern::Rational::zero();

    for (is_tone, length) in symbols_and_gaps {
        let dur = orpheus_pattern::Rational::new(length as i128, total_units as i128)
            .map_err(|_| EvalError::new("invalid rational duration in morse"))?;

        let next_pos = (current_pos + dur)
            .map_err(|_| EvalError::new("rational overflow in morse pattern generation"))?;

        if is_tone {
            let span = orpheus_pattern::TimeSpan::new(current_pos, next_pos)
                .map_err(|_| EvalError::new("invalid timespan in morse pattern generation"))?;
            events.push(orpheus_pattern::Event {
                whole: Some(span),
                part: span,
                value: 1.0,
            });
        }

        current_pos = next_pos;
    }

    Ok(Value::NumberPattern(NumberPatternValue::from_events(events)))
}"""

import re
text = re.sub(r'fn apply_morse\(args: Vec<Value>\) -> Result<Value, EvalError> \{.*?\n}\n', new_apply_morse + "\n", text, flags=re.DOTALL)

with open("crates/orpheus-lang/src/builtins.rs", "w") as f:
    f.write(text)
