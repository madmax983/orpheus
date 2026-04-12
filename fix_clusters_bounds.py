with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

content = content.replace("    T: PatternRuntimeValue + num_traits::float::Float,\n", "    T: PatternRuntimeValue,\n")

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)

print("Fix complete")
