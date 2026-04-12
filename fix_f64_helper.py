import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

# First remove any lingering helpers
remove_pattern = r"fn process_events_in_clusters<T, F, R>\([\s\S]*?Ok\(result\)\n\}\n\n"
content = re.sub(remove_pattern, "", content)


# Now inject a helper strictly bound to f64
helper_code = """
#[allow(clippy::needless_pass_by_value)]
fn process_f64_events_in_clusters<F, R>(
    mut events: Vec<Event<f64>>,
    effect_name: &str,
    mut apply_effect: F,
) -> Result<Vec<Event<f64>>, EvalError>
where
    F: FnMut(&mut [Event<f64>]) -> Result<R, EvalError>,
    R: IntoIterator<Item = Event<f64>>,
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

content = content.replace('process_events_in_clusters', 'process_f64_events_in_clusters')

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)

print("Fix complete")
