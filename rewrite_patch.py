import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

# Search for rewrite_pitch_with_tuning
match = re.search(r"fn rewrite_pitch_with_tuning<T: Clone>\(.*?\}\n\}", content, re.DOTALL | re.MULTILINE)
if match:
    old_func = match.group(0)

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

    # We also need to insert `map_inner` into `impl PatternRuntime<T>`.

    # Find `impl<T> PatternRuntime<T> {`
    impl_idx = content.find("impl<T> PatternRuntime<T> {")
    if impl_idx != -1:
        print("Found impl")
