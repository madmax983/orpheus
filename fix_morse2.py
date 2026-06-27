import sys

with open("crates/orpheus-lang/src/builtins.rs", "r") as f:
    text = f.read()

# Let's rewrite apply_morse to build a Vec<Event<f64>> manually instead of PatternNode.
# Wait, NumberPatternValue can be built from `Vec<Event<f64>>`?
# Let's check `NumberPatternValue::from_events`
