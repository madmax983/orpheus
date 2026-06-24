import sys

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    text = f.read()

search1 = """    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn with_tuning(self, table: &TuningTable) -> Self {"""
replace1 = """    fn with_tuning(self, table: &TuningTable) -> Self {"""

text = text.replace(search1, replace1)

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(text)
