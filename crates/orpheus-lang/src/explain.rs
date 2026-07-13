#[must_use]
#[doc(hidden)]
pub trait Explain {
    fn explain(&self, binding_name: &str) -> String;
}

/// Helper method to standardize REPL explanation tables.
#[must_use]
#[doc(hidden)]
pub fn explain_table<const N: usize>(headers: [&str; N]) -> comfy_table::Table {
    use comfy_table::{Cell, Table, modifiers, presets::UTF8_FULL};
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(modifiers::UTF8_ROUND_CORNERS)
        .apply_modifier(modifiers::UTF8_SOLID_INNER_BORDERS);

    let header_cells: Vec<Cell> = headers
        .into_iter()
        .map(|h| {
            Cell::new(h)
                .fg(comfy_table::Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold)
        })
        .collect();

    table.set_header(header_cells);
    table
}
