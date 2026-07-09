import re

content = open("crates/orpheus-lang/src/value.rs").read()
for i, line in enumerate(content.split("\n")):
    if "builtins::" in line or "builtins" in line:
        print(i+1, line)
