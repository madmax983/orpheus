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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_table_with_correct_headers() {
        let headers = ["Col1", "Col2", "Col3"];
        let table = explain_table(headers);

        let header = table.header().unwrap();
        assert_eq!(header.cell_count(), 3);

        let mut iter = header.cell_iter();
        assert_eq!(iter.next().unwrap().content(), "Col1");
        assert_eq!(iter.next().unwrap().content(), "Col2");
        assert_eq!(iter.next().unwrap().content(), "Col3");
    }
}
