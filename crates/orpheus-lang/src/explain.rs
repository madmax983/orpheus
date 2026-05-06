use comfy_table::{Cell, Table, presets::UTF8_BORDERS_ONLY};

pub trait Explain {
    fn explain(&self, binding_name: &str) -> String;
}

#[must_use]
pub fn explain_table<const N: usize>(headers: [&str; N]) -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);

    let header_cells: Vec<Cell> = headers
        .into_iter()
        .map(|h| {
            Cell::new(h)
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold)
        })
        .collect();

    table.set_header(header_cells);
    table
}
