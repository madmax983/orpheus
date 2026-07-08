//! Grid state for the Orca surface: a fixed-size field of base-36 glyphs.
//!
//! The grid is pure data — it knows nothing about frames, locks, or
//! operators. All dynamic behavior lives in [`super::engine`].

use thiserror::Error;

/// The empty-cell glyph.
pub const EMPTY: char = '.';
/// The bang glyph: a one-frame trigger event on the grid.
pub const BANG: char = '*';
/// The comment glyph: locks its row eastward up to the matching `#`.
pub const COMMENT: char = '#';

/// Errors constructing an Orca grid.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GridError {
    /// The grid must contain at least one row and one column.
    #[error("grid must have at least one row and one column")]
    Empty,
    /// All rows of a grid must share the same width.
    #[error("row {row} has width {found}, expected {expected}")]
    RaggedRow {
        /// Zero-based index of the offending row.
        row: usize,
        /// Width of the first row, which sets the grid width.
        expected: usize,
        /// Width of the offending row.
        found: usize,
    },
    /// Only `.`, `*`, `#`, the IO glyphs (`:` `%` `!` `?` `;` `=` `$`), and
    /// ASCII alphanumerics are valid glyphs.
    #[error("invalid glyph {glyph:?} at ({x}, {y})")]
    InvalidGlyph {
        /// The rejected character.
        glyph: char,
        /// Column of the rejected character.
        x: usize,
        /// Row of the rejected character.
        y: usize,
    },
}

/// Returns `true` when `glyph` may appear on the grid: `.`, `*`, `#`, the IO
/// operator glyphs, or an ASCII alphanumeric.
#[must_use]
pub const fn is_valid_glyph(glyph: char) -> bool {
    matches!(
        glyph,
        EMPTY | BANG | COMMENT | ':' | '%' | '!' | '?' | ';' | '=' | '$'
    ) || glyph.is_ascii_alphanumeric()
}

/// A `width x height` field of glyphs, row-major, `.` meaning empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<char>,
}

impl Grid {
    /// Creates an empty grid of the given dimensions.
    ///
    /// # Errors
    ///
    /// Returns [`GridError::Empty`] when either dimension is zero.
    pub fn new(width: usize, height: usize) -> Result<Self, GridError> {
        if width == 0 || height == 0 {
            return Err(GridError::Empty);
        }
        Ok(Self {
            width,
            height,
            cells: vec![EMPTY; width * height],
        })
    }

    /// Builds a grid from equal-width rows of glyphs.
    ///
    /// # Errors
    ///
    /// Returns [`GridError::Empty`] for zero rows or zero-width rows,
    /// [`GridError::RaggedRow`] when rows differ in width, and
    /// [`GridError::InvalidGlyph`] for characters outside the glyph alphabet.
    pub fn from_rows(rows: &[&str]) -> Result<Self, GridError> {
        let Some(first) = rows.first() else {
            return Err(GridError::Empty);
        };
        let width = first.chars().count();
        let mut grid = Self::new(width, rows.len())?;
        for (y, row) in rows.iter().enumerate() {
            let found = row.chars().count();
            if found != width {
                return Err(GridError::RaggedRow {
                    row: y,
                    expected: width,
                    found,
                });
            }
            for (x, glyph) in row.chars().enumerate() {
                if !is_valid_glyph(glyph) {
                    return Err(GridError::InvalidGlyph { glyph, x, y });
                }
                grid.cells[y * width + x] = glyph;
            }
        }
        Ok(grid)
    }

    /// Number of columns.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Number of rows.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns `true` when `(x, y)` lies on the grid.
    #[must_use]
    pub const fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    /// The glyph at `(x, y)`, or `None` out of bounds.
    #[must_use]
    pub fn glyph_at(&self, x: usize, y: usize) -> Option<char> {
        self.in_bounds(x, y).then(|| self.cells[y * self.width + x])
    }

    /// Writes `glyph` at `(x, y)`; returns `false` (without writing) when the
    /// position is out of bounds or the glyph is not in the alphabet.
    pub fn set(&mut self, x: usize, y: usize, glyph: char) -> bool {
        if !self.in_bounds(x, y) || !is_valid_glyph(glyph) {
            return false;
        }
        self.cells[y * self.width + x] = glyph;
        true
    }

    /// Renders the grid as one `String` per row, for tests and dumps.
    #[must_use]
    pub fn rows(&self) -> Vec<String> {
        self.cells
            .chunks(self.width)
            .map(|row| row.iter().collect())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{BANG, EMPTY, Grid, GridError};

    #[test]
    fn new_grid_is_empty() {
        let grid = Grid::new(3, 2).expect("valid dimensions");
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 2);
        assert_eq!(grid.rows(), vec!["...", "..."]);
        assert_eq!(grid.glyph_at(2, 1), Some(EMPTY));
        assert_eq!(grid.glyph_at(3, 0), None);
        assert_eq!(grid.glyph_at(0, 2), None);
    }

    #[test]
    fn zero_dimension_rejected() {
        assert_eq!(Grid::new(0, 4), Err(GridError::Empty));
        assert_eq!(Grid::new(4, 0), Err(GridError::Empty));
        assert_eq!(Grid::from_rows(&[]), Err(GridError::Empty));
    }

    #[test]
    fn ragged_rows_rejected() {
        assert_eq!(
            Grid::from_rows(&["ab", "abc"]),
            Err(GridError::RaggedRow {
                row: 1,
                expected: 2,
                found: 3,
            })
        );
    }

    #[test]
    fn invalid_glyphs_rejected() {
        assert_eq!(
            Grid::from_rows(&[".@."]),
            Err(GridError::InvalidGlyph {
                glyph: '@',
                x: 1,
                y: 0,
            })
        );
        let mut grid = Grid::new(2, 2).expect("valid dimensions");
        assert!(!grid.set(0, 0, '@'));
        assert_eq!(grid.glyph_at(0, 0), Some(EMPTY));
    }

    #[test]
    fn set_and_get_round_trip() {
        let mut grid = Grid::new(2, 2).expect("valid dimensions");
        assert!(grid.set(1, 0, BANG));
        assert!(grid.set(0, 1, 'E'));
        assert!(!grid.set(2, 0, 'a'));
        assert_eq!(grid.rows(), vec![".*", "E."]);
    }
}
