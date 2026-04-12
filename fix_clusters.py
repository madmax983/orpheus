import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

# Remove the impl PatternRuntimeValue for f64 helper
remove_pattern = r"    fn process_clusters<F, R>\([\s\S]*?Ok\(result\)\n    \}\n"
content = re.sub(remove_pattern, "", content)

# Inject the helper outside of any impl block (e.g. before impl PatternRuntimeValue for f64)
helper_code = """
fn process_events_in_clusters<T, F, R>(
    mut events: Vec<Event<T>>,
    effect_name: &str,
    mut apply_effect: F,
) -> Result<Vec<Event<T>>, EvalError>
where
    T: PatternRuntimeValue,
    F: FnMut(&mut [Event<T>]) -> Result<R, EvalError>,
    R: IntoIterator<Item = Event<T>>,
{
    sort_events(&mut events);
    let mut result = Vec::with_capacity(events.len());
    let mut index = 0;

    while index < events.len() {
        let start_index = index;
        let span = events[start_index].part.clone();
        while index < events.len() && events[index].part == span {
            if !events[index].value.is_finite() {
                return Err(EvalError::new(format!(
                    "`{}` requires finite numeric values",
                    effect_name
                )));
            }
            index += 1;
        }

        let cluster_events = apply_effect(&mut events[start_index..index])?;
        let cluster_events_vec: Vec<_> = cluster_events.into_iter().collect();
        if result.len() + cluster_events_vec.len() > 100_000 {
            return Err(EvalError::new(
                "evaluation exceeded the maximum allowed event limit",
            ));
        }
        result.extend(cluster_events_vec);
    }

    sort_events(&mut result);
    Ok(result)
}

"""

impl_pattern = r"(impl PatternRuntimeValue for f64 \{)"
content = re.sub(impl_pattern, helper_code + r"\1", content, count=1)

content = content.replace('Self::process_clusters', 'process_events_in_clusters')

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)

print("Fix complete")
