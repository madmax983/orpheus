impl Explain for Value {
    fn explain(&self, binding_name: &str) -> String {
        match self {
            Self::SamplePattern(val) => val.explain(binding_name),
            Self::NumberPattern(val) => val.explain(binding_name),
            Self::ArpDirection(val) => val.explain(binding_name),
            Self::PitchClassSet(val) => val.explain(binding_name),
            Self::Function(val) => val.explain(binding_name),
            Self::Pedal(val) => val.explain(binding_name),
            Self::PluginPattern(val) => val.explain(binding_name),
            Self::Tuning(val) => val.explain(binding_name),
            Self::String(val) => {
                use comfy_table::{Cell, CellAlignment};
                use crossterm::style::Stylize;

                let title = format!(
                    "{} {}",
                    "String Plan:".cyan().bold(),
                    binding_name.yellow()
                );

                let mut table = crate::explain::explain_table(["Property", "Value"]);

                table.add_row(vec![
                    Cell::new("Type").fg(comfy_table::Color::Cyan),
                    Cell::new("String")
                        .fg(comfy_table::Color::Yellow)
                        .set_alignment(CellAlignment::Right),
                ]);
                table.add_row(vec![
                    Cell::new("Value").fg(comfy_table::Color::Cyan),
                    Cell::new(val.to_string())
                        .fg(comfy_table::Color::Green)
                        .set_alignment(CellAlignment::Right),
                ]);

                format!("{title}\n{table}")
            }
        }
    }
}
