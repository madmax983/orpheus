import re

file_path = "crates/orpheus-lang/src/value.rs"

with open(file_path, "r") as f:
    content = f.read()

content = content.replace("pub enum Value", "pub(crate) enum Value")

with open(file_path, "w") as f:
    f.write(content)
