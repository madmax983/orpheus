//! The `dot_export` module provides export functionality for Pedal graphs to Graphviz DOT.
//!
//! This allows visualizing the signal flow of Pedal Values.
use std::io::Write;
use std::path::Path;

use crate::error::EvalError;
use crate::pedal::{PedalNodeKind, PedalValue, SignalKind};

/// Exports a pedal graph's internal evaluated plan to a Graphviz DOT file.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_pedal_value_to_dot;
///
/// let source = "my_graph = graph { wet = input |> clip(model=silicon_hard) ; wet |> output }";
/// let env = eval_module(source, ReplMode::Strict).unwrap();
/// let pedal = env.get("my_graph").unwrap().as_pedal().unwrap();
///
/// let path = std::env::temp_dir().join("pedal.dot");
/// export_pedal_value_to_dot(pedal, &path).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the file cannot be written.
pub fn export_pedal_value_to_dot(
    pedal: &PedalValue,
    path: impl AsRef<Path>,
) -> Result<(), EvalError> {
    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "digraph PedalGraph {{")?;
    writeln!(file, "    rankdir=LR;")?;
    writeln!(
        file,
        "    node [shape=box, style=filled, fontname=\"Helvetica\"];"
    )?;
    writeln!(file, "    edge [fontname=\"Helvetica\"];")?;

    let plan = pedal.plan();

    // Iterate over bindings to create nodes
    for binding in plan.bindings() {
        let node = binding.node();
        let name = binding.name();
        let label = format!("{}: {}", name, node.kind());

        let color = match node.signal_kind() {
            SignalKind::Audio => "lightblue",
            SignalKind::Control => "lightyellow",
        };

        let shape = match node.kind() {
            PedalNodeKind::Input | PedalNodeKind::Output => "invhouse",
            PedalNodeKind::Constant | PedalNodeKind::Reference => "ellipse",
            _ => "box",
        };

        writeln!(
            file,
            "    \"{name}\" [label=\"{label}\", fillcolor=\"{color}\", shape=\"{shape}\"];"
        )?;
    }

    let result_node = plan.result();
    let result_label = format!("result: {}", result_node.kind());
    let result_color = match result_node.signal_kind() {
        SignalKind::Audio => "lightblue",
        SignalKind::Control => "lightyellow",
    };
    writeln!(
        file,
        "    \"result\" [label=\"{result_label}\", fillcolor=\"{result_color}\", shape=\"invhouse\"];"
    )?;

    // Add edges by parsing the summary for binding names
    // The summary looks like: `clip(input, model=silicon_hard)` or `(foo * bar)`
    let mut valid_names: Vec<String> = plan
        .bindings()
        .iter()
        .map(|b| b.name().to_string())
        .collect();
    valid_names.push("input".to_string());

    // Sort names by length descending so that we match longest names first to prevent partial matches
    valid_names.sort_by_key(|b| std::cmp::Reverse(b.len()));

    for binding in plan.bindings() {
        let summary = binding.node().summary();
        let binding_name = binding.name();

        let tokens = summary.split(|c: char| !c.is_alphanumeric() && c != '_');

        let mut matched_names = std::collections::HashSet::new();
        for token in tokens {
            if valid_names.contains(&token.to_string()) && token != binding_name {
                matched_names.insert(token.to_string());
            }
        }

        for src_name in matched_names {
            writeln!(file, "    \"{src_name}\" -> \"{binding_name}\";")?;
        }
    }

    let result_summary = result_node.summary();
    let tokens = result_summary.split(|c: char| !c.is_alphanumeric() && c != '_');
    let mut matched_names = std::collections::HashSet::new();

    for token in tokens {
        if valid_names.contains(&token.to_string()) {
            matched_names.insert(token.to_string());
        }
    }

    for src_name in matched_names {
        writeln!(file, "    \"{src_name}\" -> \"result\";")?;
    }

    writeln!(file, "}}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn dot_exporter_generates_valid_format() {
        let source = "my_graph = graph { wet = input |> clip(model=silicon_hard) ; wet |> output }";
        let module = eval_module(source, ReplMode::Strict).unwrap();
        let pedal = module.get("my_graph").unwrap().as_pedal().unwrap();

        let path = std::env::temp_dir().join("test_pedal.dot");
        export_pedal_value_to_dot(pedal, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("digraph PedalGraph {"));
        assert!(content.contains("\"wet\" [label=\"wet: stage\""));
        assert!(content.contains("\"input\" -> \"wet\""));
        assert!(content.contains("\"wet\" -> \"result\""));
    }

    #[test]
    fn dot_exporter_does_not_partially_match_names() {
        // e.g. "in" should not match "input"
        let source =
            "my_graph = graph { in = input |> clip ; wet = in |> lpf(400) ; wet |> output }";
        let module = eval_module(source, ReplMode::Strict).unwrap();
        let pedal = module.get("my_graph").unwrap().as_pedal().unwrap();

        let path = std::env::temp_dir().join("test_pedal_partial.dot");
        export_pedal_value_to_dot(pedal, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"in\" -> \"wet\""));
        assert!(content.contains("\"input\" -> \"in\""));
        // Make sure `in` wasn't matched inside `input` by verifying we don't have something weird like `in -> in`
        assert!(!content.contains("\"in\" -> \"in\""));
    }

    #[test]
    fn export_pedal_value_to_dot_io_error() {
        let source = "my_graph = graph { wet = input |> clip ; wet |> output }";
        let module = eval_module(source, ReplMode::Strict).unwrap();
        let pedal = module.get("my_graph").unwrap().as_pedal().unwrap();

        assert_eq!(
            export_pedal_value_to_dot(pedal, "/invalid_directory/invalid_file.dot")
                .unwrap_err()
                .to_string(),
            "file not found"
        );
    }
}
