import re

file_path = "crates/orpheus-lang/src/types/env.rs"

with open(file_path, "r") as f:
    content = f.read()

content = content.replace("pub struct TypeScheme", "pub(crate) struct TypeScheme")
content = content.replace("pub struct TypeEnv", "pub(crate) struct TypeEnv")

with open(file_path, "w") as f:
    f.write(content)
