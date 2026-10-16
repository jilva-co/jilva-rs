//! Renderer buffer model (`docs/ARCHITECTURE.md` §3.7).
//!
//! **Phase-1 status:** only the single-cell vocabulary exists. The buffer,
//! the diff/patch algorithm, damage tracking, and flush policy are RFC
//! detail — this module deliberately implements no diffing so it cannot fake
//! a renderer. Rendering stays a pure function of explicit inputs; there are
//! no hidden globals here.
//!
//! Limitation, stated plainly: [`Cell`] holds one Unicode scalar value.
//! Grapheme-cluster segmentation, double-width handling, and the width-table
//! authority (considerations §8.3–§8.4) are future design work, not modelled.

/// One terminal cell: a single symbol awaiting shaping-aware width rules.
///
/// [`Cell::blank`] (a space) is the neutral ground state used when diffing
/// later needs an "empty vs. styled-blank" baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The symbol displayed in this cell (grapheme support is future work).
    pub symbol: char,
}

impl Cell {
    /// Creates a cell holding the given symbol.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Cell;
    ///
    /// assert!(!Cell::new('x').is_blank());
    /// ```
    #[must_use]
    pub const fn new(symbol: char) -> Self {
        Self { symbol }
    }

    /// Returns the blank (space) cell used as the buffer ground state.
    #[must_use]
    pub const fn blank() -> Self {
        Self { symbol: ' ' }
    }

    /// Returns `true` when this cell holds a space.
    #[must_use]
    pub const fn is_blank(self) -> bool {
        self.symbol as u32 == ' ' as u32
    }
}

impl Default for Cell {
    /// Returns the blank ground-state cell.
    fn default() -> Self {
        Self::blank()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_is_the_default_ground_state() {
        assert_eq!(Cell::default(), Cell::blank());
        assert!(Cell::blank().is_blank());
        assert!(!Cell::new('x').is_blank());
    }
}
