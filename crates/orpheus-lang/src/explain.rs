//! Internal formatting trait for generating human-readable REPL summaries.
//!
//! This module defines the [`Explain`] trait used by the `:explain <binding>`
//! command. It allows complex structures like graphs, pedals, and AST components
//! to pretty-print themselves into tabular or descriptive string layouts
//! appropriate for terminal display.

#[must_use]
#[doc(hidden)]
pub trait Explain {
    fn explain(&self, binding_name: &str) -> String;
}

/// Helper method to standardize REPL explanation tables.
#[must_use]
#[doc(hidden)]
pub fn explain_table<const N: usize>(headers: [&str; N]) -> comfy_table::Table {
    use comfy_table::{Cell, Table, presets::UTF8_BORDERS_ONLY};
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
