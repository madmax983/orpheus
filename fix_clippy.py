import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

# Fix `use_self`
content = content.replace("impl FnMut(PatternRuntime<T>) -> PatternRuntime<T>", "impl FnMut(Self) -> Self")

# Fix `match_same_arms`
old_func = """fn rewrite_pitch_with_tuning<T: Clone>(
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

new_func = """fn rewrite_pitch_with_tuning<T: Clone>(
    pattern: PatternRuntime<T>,
    table: &TuningTable,
) -> PatternRuntime<T> {
    match pattern {
        PatternRuntime::Pitch { semitones, inner } | PatternRuntime::TunedPitch { semitones, inner, .. } => PatternRuntime::TunedPitch {
            semitones,
            tuning: table.clone(),
            inner: Box::new(rewrite_pitch_with_tuning(*inner, table)),
        },
        PatternRuntime::PitchPattern { control, inner } | PatternRuntime::TunedPitchPattern { control, inner, .. } => PatternRuntime::TunedPitchPattern {
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

content = content.replace(old_func, new_func)

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)
