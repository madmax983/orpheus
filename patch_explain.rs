impl Explain for PluginPatternValue {
    fn explain(&self, binding_name: &str) -> String {
        use comfy_table::{Cell, CellAlignment};
        use crossterm::style::Stylize;

        let title = format!(
            "{} {}",
            "Plugin Pattern Plan:".cyan().bold(),
            binding_name.yellow()
        );

        let mut table = crate::explain::explain_table(["Property", "Value"]);

        table.add_row(vec![
            Cell::new("Type").fg(comfy_table::Color::Cyan),
            Cell::new("Plugin Instrument")
                .fg(comfy_table::Color::Yellow)
                .set_alignment(CellAlignment::Right),
        ]);

        format!("{title}\n{table}")
    }
}

impl Explain for PitchClassSetValue {
    fn explain(&self, binding_name: &str) -> String {
        use comfy_table::{Cell, CellAlignment};
        use crossterm::style::Stylize;

        let title = format!(
            "{} {}",
            "Pitch Class Set Plan:".cyan().bold(),
            binding_name.yellow()
        );

        let mut table = crate::explain::explain_table(["Property", "Value"]);

        table.add_row(vec![
            Cell::new("Type").fg(comfy_table::Color::Cyan),
            Cell::new("Scale/Chord")
                .fg(comfy_table::Color::Yellow)
                .set_alignment(CellAlignment::Right),
        ]);

        format!("{title}\n{table}")
    }
}

impl Explain for ArpDirectionValue {
    fn explain(&self, binding_name: &str) -> String {
        use comfy_table::{Cell, CellAlignment};
        use crossterm::style::Stylize;

        let title = format!(
            "{} {}",
            "Arp Direction Plan:".cyan().bold(),
            binding_name.yellow()
        );

        let mut table = crate::explain::explain_table(["Property", "Value"]);

        table.add_row(vec![
            Cell::new("Type").fg(comfy_table::Color::Cyan),
            Cell::new("Arpeggiator Direction")
                .fg(comfy_table::Color::Yellow)
                .set_alignment(CellAlignment::Right),
        ]);

        format!("{title}\n{table}")
    }
}
