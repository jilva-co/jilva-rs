//! Layout region types (`docs/ARCHITECTURE.md` §3.6).
//!
//! **Phase-1 status:** only the cell-geometry vocabulary exists — integer
//! columns and rows, never pixels. Constraint solving, the measure/arrange
//! pass, overflow policy, and invalidation rules are RFC detail. Layout never
//! measures text; it consumes already-measured sizes and produces regions.

/// An integer rectangle in terminal cell geometry.
///
/// `(x, y)` is the top-left cell; `width` and `height` are cell counts.
/// Fractional remainders and constraint solving are future work — this type
/// only names *where* a laid-out region is, honestly and without clamping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left column, in cells from the terminal's left edge.
    pub x: u16,
    /// Top row, in cells from the terminal's top edge.
    pub y: u16,
    /// Width in cells (columns).
    pub width: u16,
    /// Height in cells (rows).
    pub height: u16,
}

impl Rect {
    /// Creates a rectangle from a top-left origin and a cell size.
    ///
    /// Values are stored as given; validation of over-constrained or
    /// unsatisfiable layouts is RFC detail and never panics here.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Rect;
    ///
    /// let region = Rect::new(0, 0, 80, 24);
    /// assert_eq!(region.area(), 1920);
    /// ```
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the area in cells.
    ///
    /// Computed in `u32` so a full `u16 x u16` region cannot overflow.
    #[must_use]
    pub const fn area(self) -> u32 {
        self.width as u32 * self.height as u32
    }

    /// Returns `true` when the region covers no cells.
    ///
    /// A zero width *or* a zero height is empty; both are required to be
    /// non-zero before any paint work is meaningful.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_counts_cells_without_overflow() {
        assert_eq!(Rect::new(0, 0, 80, 24).area(), 1920);
        assert_eq!(Rect::new(0, 0, u16::MAX, u16::MAX).area(), 4_294_836_225);
    }

    #[test]
    fn emptiness_requires_both_dimensions() {
        assert!(Rect::new(0, 0, 0, 24).is_empty());
        assert!(Rect::new(0, 0, 80, 0).is_empty());
        assert!(!Rect::new(1, 2, 3, 4).is_empty());
    }
}
