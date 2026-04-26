import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

# Generate map_inner function string (we have it from previous script)
match = re.search(r"enum PatternRuntime<T> \{(.*?)\n\}", content, re.DOTALL | re.MULTILINE)
variants_text = match.group(1)
variants = []
current_variant = None
fields = []
is_struct = False

for line in variants_text.split("\n"):
    line = line.strip()
    if not line or line.startswith("//"):
        continue
    if line.endswith("{"):
        current_variant = line.split("{")[0].strip()
        is_struct = True
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

map_inner_code = """
    pub(crate) fn map_inner(self, mut f: impl FnMut(PatternRuntime<T>) -> PatternRuntime<T>) -> Self {
        match self {
"""

for v in variants:
    name = v["name"]
    if v["type"] == "tuple":
        pass
    elif v["type"] == "struct":
        if "inner" in v["fields"]:
            fields_str = ", ".join(v["fields"])
            construct_fields = []
            for field in v["fields"]:
                if field == "inner":
                    construct_fields.append("inner: Box::new(f(*inner))")
                else:
                    construct_fields.append(f"{field}")
            construct_str = ", ".join(construct_fields)
            map_inner_code += f"            Self::{name} {{ {fields_str} }} => Self::{name} {{ {construct_str} }},\n"

map_inner_code += """
            // Variants without an `inner` field
            other => other,
        }
    }
"""

# Find `impl<T> PatternRuntime<T> {` and insert map_inner
impl_idx = content.find("impl<T> PatternRuntime<T> {")
insert_idx = content.find("{", impl_idx) + 1
content = content[:insert_idx] + map_inner_code + content[insert_idx:]

# Replace rewrite_pitch_with_tuning
old_func_match = re.search(r"fn rewrite_pitch_with_tuning<T: Clone>\(.*?\}\n\}", content, re.DOTALL | re.MULTILINE)
new_func = """fn rewrite_pitch_with_tuning<T: Clone>(
    pattern: PatternRuntime<T>,
    table: &TuningTable,
) -> PatternRuntime<T> {
    match pattern {
        PatternRuntime::Pitch { semitones, inner } => PatternRuntime::TunedPitch {
            semitones,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::PitchPattern { control, inner } => PatternRuntime::TunedPitchPattern {
            control,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::TunedPitch { semitones, inner, .. } => PatternRuntime::TunedPitch {
            semitones,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::TunedPitchPattern { control, inner, .. } => PatternRuntime::TunedPitchPattern {
            control,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::Stack(layers) => PatternRuntime::Stack(
            layers
                .into_iter()
                .map(|layer| rewrite_pitch_with_tuning(layer, table))
                .collect(),
        ),
        other => other.map_inner(|inner| rewrite_pitch_with_tuning(inner, table)),
    }
}"""
content = content[:old_func_match.start()] + new_func + content[old_func_match.end():]

# Remove #[allow(clippy::too_many_lines, clippy::match_same_arms)]
allow_str = "#[allow(clippy::too_many_lines, clippy::match_same_arms)]\n"
if allow_str in content:
    content = content.replace(allow_str, "")

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)

print("Applied patch.")
