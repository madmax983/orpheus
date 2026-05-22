//! Structural explanation traits for the REPL environment.
//!
//! This module provides the `Explain` trait, which allows complex, opaque runtime
//! values (like plugin configurations, stateful graphs, and functions) to render
//! themselves as human-readable diagnostic tables when queried with the `:explain`
//! command in the REPL.

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
