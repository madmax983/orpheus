import sys

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    text = f.read()

search = """    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn with_tuning(self, table: &TuningTable) -> Self {"""
if search in text:
    print("Found with_tuning exactly")
else:
    print("with_tuning not found exactly")
