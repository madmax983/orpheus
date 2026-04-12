with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

content = content.replace('"`{}` requires finite numeric values",\n                    effect_name', '"`{effect_name}` requires finite numeric values"')

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)

print("Fix complete")
