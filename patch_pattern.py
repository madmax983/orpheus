with open('crates/orpheus-pattern/src/cycle.rs', 'r') as f:
    content = f.read()

target = """    let capacity = cycle_count.checked_mul(unit_events.len()).ok_or_else(|| {
        PatternError::ArithmeticOverflow {
            operation: "pattern capacity calculation",
        }
    })?;"""

repl = """    let capacity = cycle_count.checked_mul(unit_events.len()).ok_or(PatternError::ArithmeticOverflow {
        operation: "pattern capacity calculation",
    })?;"""

content = content.replace(target, repl)

with open('crates/orpheus-pattern/src/cycle.rs', 'w') as f:
    f.write(content)
