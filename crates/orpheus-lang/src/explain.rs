//! Explain mechanism for REPL bindings.
//!
//! This module provides the [`Explain`] trait and helper methods to standardise
//! how the REPL explains a pattern's type signature, structure, and current state
//! to the user in a readable format.

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
