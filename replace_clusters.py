import re

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    content = f.read()

helper_code = """
    fn process_clusters<F, R>(
        mut events: Vec<Event<Self>>,
        effect_name: &str,
        mut apply_effect: F,
    ) -> Result<Vec<Event<Self>>, EvalError>
    where
        F: FnMut(&mut [Event<Self>]) -> Result<R, EvalError>,
        R: IntoIterator<Item = Event<Self>>,
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
            let mut cluster_events_vec: Vec<_> = cluster_events.into_iter().collect();
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

roll_pattern = r"    fn roll_events\([\s\S]*?Ok\(rolled\)\n    \}"
roll_replacement = """    fn roll_events(
        events: Vec<Event<Self>>,
        steps: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Self::process_clusters(events, "roll", |cluster| {
            roll_event_cluster(cluster, steps)
        })
    }"""

strum_pattern = r"    fn strum_events\([\s\S]*?Ok\(events\)\n    \}"
strum_replacement = """    fn strum_events(events: Vec<Event<Self>>) -> Result<Vec<Event<Self>>, EvalError> {
        Self::process_clusters(events, "strum", |cluster| {
            strum_event_cluster(cluster)?;
            Ok(cluster.to_vec())
        })
    }"""

arp_pattern = r"    fn arp_events\([\s\S]*?Ok\(arped\)\n    \}"
arp_replacement = """    fn arp_events(
        events: Vec<Event<Self>>,
        steps: u32,
        direction: ArpDirectionValue,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Self::process_clusters(events, "arp", |cluster| {
            arp_event_cluster(cluster, steps, direction)
        })
    }"""

invert_pattern = r"    fn invert_events\([\s\S]*?Ok\(events\)\n    \}"
invert_replacement = """    fn invert_events(
        events: Vec<Event<Self>>,
        count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Self::process_clusters(events, "invert", |cluster| {
            invert_event_cluster(cluster, count)?;
            Ok(cluster.to_vec())
        })
    }"""

drop_pattern = r"    fn drop_events\([\s\S]*?Ok\(events\)\n    \}"
drop_replacement = """    fn drop_events(
        events: Vec<Event<Self>>,
        count: u32,
    ) -> Result<Vec<Event<Self>>, EvalError> {
        Self::process_clusters(events, "drop", |cluster| {
            drop_event_cluster(cluster, count)?;
            Ok(cluster.to_vec())
        })
    }"""


# First, locate `impl PatternRuntimeValue for f64` to insert the helper.
# It starts around line 1026.
impl_pattern = r"(impl PatternRuntimeValue for f64 \{)"
content = re.sub(impl_pattern, r"\1" + helper_code, content, count=1)


# Note: we need to restrict substitutions to the `f64` impl because there's a dummy impl for `SampleEvent` above it.
f64_impl_start = content.find("impl PatternRuntimeValue for f64 {")

before_f64 = content[:f64_impl_start]
in_f64 = content[f64_impl_start:]

in_f64 = re.sub(roll_pattern, roll_replacement, in_f64)
in_f64 = re.sub(strum_pattern, strum_replacement, in_f64)
in_f64 = re.sub(arp_pattern, arp_replacement, in_f64)
in_f64 = re.sub(invert_pattern, invert_replacement, in_f64)
in_f64 = re.sub(drop_pattern, drop_replacement, in_f64)

content = before_f64 + in_f64

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(content)

print("Replacement complete")
