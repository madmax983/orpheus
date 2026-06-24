with open("crates/orpheus-lang/src/tracker.rs", "r") as f:
    text = f.read()

# Verify if tracker.rs has clippy::too_many_lines
import re
print("too_many_lines in tracker.rs:", text.count("clippy::too_many_lines"))
