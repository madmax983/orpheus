import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

# First let's get the definition of PatternRuntime
match = re.search(r"enum PatternRuntime<T> \{(.*?)\n\}", content, re.DOTALL | re.MULTILINE)
if not match:
    print("Failed to find PatternRuntime")
    exit(1)

variants_text = match.group(1)

# Parse the variants and their fields
variants = []
current_variant = None
fields = []
is_struct = False
is_tuple = False

for line in variants_text.split("\n"):
    line = line.strip()
    if not line or line.startswith("//"):
        continue

    if line.endswith("{"):
        current_variant = line.split("{")[0].strip()
        is_struct = True
    elif line.endswith("("): # It might be a tuple but formatted on multiple lines, let's assume simple tuple for now
        pass
    elif "(" in line and line.endswith("),"):
        name = line.split("(")[0].strip()
        variants.append({"name": name, "type": "tuple", "fields": []})
    elif line == "}," or line == "}":
        if current_variant:
            variants.append({"name": current_variant, "type": "struct", "fields": fields})
            current_variant = None
            fields = []
            is_struct = False
    elif is_struct:
        field_name = line.split(":")[0].strip()
        fields.append(field_name)

print("Parsed variants:")
for v in variants:
    print(v)

# Generate map_inner function
code = """
    pub(crate) fn map_inner(self, mut f: impl FnMut(PatternRuntime<T>) -> PatternRuntime<T>) -> Self {
        match self {
"""

for v in variants:
    name = v["name"]
    if v["type"] == "tuple":
        pass # Stream, Cycle, etc. No inner.
    elif v["type"] == "struct":
        if "inner" in v["fields"]:
            # Need to map inner
            fields_str = ", ".join(v["fields"])

            # For construction, we need to apply f to inner
            construct_fields = []
            for field in v["fields"]:
                if field == "inner":
                    construct_fields.append("inner: Box::new(f(*inner))")
                else:
                    construct_fields.append(f"{field}")
            construct_str = ", ".join(construct_fields)

            code += f"            Self::{name} {{ {fields_str} }} => Self::{name} {{ {construct_str} }},\n"

code += """
            // Variants without an `inner` field
            other => other,
        }
    }
"""

print(code)
