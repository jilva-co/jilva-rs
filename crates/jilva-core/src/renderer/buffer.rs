//! The cell buffer: an owned, bounds-disciplined grid of [`Cell`]s (RFC-0004 §3).

use super::cell::Cell;
use super::width::{cluster_width, graphemes};
use crate::layout::Rect;
use crate::output::ResolvedAttributes;

/// A rectangular grid of terminal cells, origin at the top-left `(0, 0)`.
///
/// # Ownership and bounds
///
/// A buffer owns its cells outright; there are no views and no borrowed
/// lifetimes. Every writing method clips to the buffer (and to an optional
/// clip rectangle) and never panics: writes outside the grid are dropped,
/// reads outside the grid return `None`. A zero-sized buffer is valid and
/// inert.
///
/// # Wide-cell invariant
///
/// Every double-width head is followed by exactly one continuation cell, and
/// every continuation follows a head. Writes maintain this by *repairing*
/// whatever they overwrite: replacing half of a wide glyph blanks the other
/// half (keeping its attributes), so a torn glyph never exists in the buffer
/// and never reaches the diff.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Buffer, ResolvedAttributes};
///
/// let mut buffer = Buffer::new(8, 2);
/// let end = buffer.set_string(1, 0, "a日b", ResolvedAttributes::plain());
/// assert_eq!(end, 5); // 1 + 1 + 2 + 1
/// assert_eq!(buffer.text_rows(), [" a日b   ", "        "]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Buffer {
    /// Creates a buffer of ground-state cells.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::blank(); usize::from(width) * usize::from(height)],
        }
    }

    /// Returns the width in columns.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Returns the height in rows.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Returns the buffer's full extent as a rectangle at the origin.
    #[must_use]
    pub const fn area(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// Returns the cell at `(x, y)`, or `None` outside the buffer.
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|index| &self.cells[index])
    }

    /// Returns row `y` as a slice, or `None` outside the buffer.
    #[must_use]
    pub fn row(&self, y: u16) -> Option<&[Cell]> {
        if y >= self.height {
            return None;
        }
        let start = usize::from(y) * usize::from(self.width);
        Some(&self.cells[start..start + usize::from(self.width)])
    }

    /// Resets every cell to the ground state.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::blank());
    }

    /// Replaces the grid with a new ground-state grid of the given size.
    /// Existing content is discarded — a resize invalidates layout anyway.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.cells.clear();
        self.cells
            .resize(usize::from(width) * usize::from(height), Cell::blank());
    }

    /// Fills `rect` (clipped to the buffer) with styled blanks.
    ///
    /// Wide glyphs cut by the rectangle edge are repaired, never torn.
    pub fn fill(&mut self, rect: Rect, attrs: ResolvedAttributes) {
        let Some((x0, y0, x1, y1)) = self.clip(rect) else {
            return;
        };
        for y in y0..y1 {
            for x in x0..x1 {
                self.put_blank(x, y, attrs);
            }
        }
    }

    /// Replaces the attributes of every cell in `rect` (clipped to the
    /// buffer) and leaves symbols alone.
    ///
    /// A wide glyph only partly inside the rectangle takes the new attributes
    /// as a whole, so the head and continuation never disagree.
    pub fn set_attrs(&mut self, rect: Rect, attrs: ResolvedAttributes) {
        self.map_attrs(rect, |_| attrs);
    }

    /// Replaces the attributes of every cell in `rect` (clipped to the
    /// buffer) with `f(old attributes)` and leaves symbols alone.
    ///
    /// The building block for effects that depend on what is already painted:
    /// tinting, dimming, shadows. `f` must be a pure function of its input; a
    /// wide glyph only partly inside the rectangle is mapped as a whole, and a
    /// pure `f` gives its head and continuation the same result, so they never
    /// disagree.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Buffer, Rect, ResolvedAttributes};
    ///
    /// let mut buffer = Buffer::new(4, 1);
    /// buffer.set_string(0, 0, "ab", ResolvedAttributes::plain());
    /// buffer.map_attrs(Rect::new(0, 0, 2, 1), |attrs| attrs.with_bold());
    /// assert!(buffer.get(1, 0).unwrap().attrs().bold);
    /// assert!(!buffer.get(2, 0).unwrap().attrs().bold);
    /// ```
    pub fn map_attrs(&mut self, rect: Rect, f: impl Fn(&ResolvedAttributes) -> ResolvedAttributes) {
        let Some((left, top, right, bottom)) = self.clip(rect) else {
            return;
        };
        for y in top..bottom {
            let row_start = usize::from(y) * usize::from(self.width);
            let mut x0 = left;
            let mut x1 = right;
            if x0 > 0 && self.cells[row_start + usize::from(x0)].is_continuation() {
                x0 -= 1;
            }
            if x1 < self.width && self.cells[row_start + usize::from(x1) - 1].width() == 2 {
                x1 += 1;
            }
            for cell in &mut self.cells[row_start + usize::from(x0)..row_start + usize::from(x1)] {
                let mapped = f(cell.attrs());
                cell.restyle(mapped);
            }
        }
    }

    /// Writes `text` starting at `(x, y)` and returns the column just past
    /// the last cell drawn (clamped to the right edge).
    ///
    /// Text is segmented into grapheme clusters; each cluster is drawn as one
    /// cell (or one wide head plus continuation). Control-bearing and
    /// zero-width clusters are skipped, never drawn. Text that runs past the
    /// right edge is truncated on a cluster boundary; a wide glyph that would
    /// straddle the edge is replaced by one styled blank so no half glyph is
    /// drawn. Text is never wrapped.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Buffer, ResolvedAttributes};
    ///
    /// let mut buffer = Buffer::new(4, 1);
    /// // The wide glyph needs two columns but only one remains: blank pad.
    /// let end = buffer.set_string(0, 0, "abc日", ResolvedAttributes::plain());
    /// assert_eq!(end, 4);
    /// assert_eq!(buffer.text_rows(), ["abc "]);
    /// ```
    pub fn set_string(&mut self, x: u16, y: u16, text: &str, attrs: ResolvedAttributes) -> u16 {
        self.set_string_clipped(x, y, text, attrs, self.area())
    }

    /// Like [`Buffer::set_string`], but draws only inside `clip`.
    ///
    /// `clip` is intersected with the buffer. Clusters left of the clip are
    /// skipped (a wide glyph straddling the left clip edge leaves one styled
    /// blank at the edge); clusters past the right clip edge follow the
    /// truncation rule of [`Buffer::set_string`]. A row outside `clip` draws
    /// nothing. Returns the column where following text would start (clamped
    /// to the right clip edge), or `x` when nothing was drawn.
    pub fn set_string_clipped(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        attrs: ResolvedAttributes,
        clip: Rect,
    ) -> u16 {
        let Some((left, top, right, bottom)) = self.clip(clip) else {
            return x;
        };
        if y < top || y >= bottom {
            return x;
        }
        let (left, right) = (u32::from(left), u32::from(right));
        let mut cursor = u32::from(x);
        for cluster in graphemes(text) {
            let width = cluster_width(cluster);
            if width == 0 {
                continue;
            }
            if cursor >= right {
                break;
            }
            let end = cursor + u32::from(width);
            if cursor < left {
                if end > left {
                    // Wide glyph straddling the left clip edge.
                    self.put_blank(left as u16, y, attrs);
                }
                cursor = end;
                continue;
            }
            if end > right {
                // Wide glyph with one column of room: pad, then stop.
                self.put_blank(cursor as u16, y, attrs);
                cursor = right;
                break;
            }
            self.put_cluster(cursor as u16, y, cluster, width, attrs);
            cursor = end;
        }
        // `right <= self.width <= u16::MAX`, so the cast cannot truncate.
        cursor.min(right) as u16
    }

    /// Returns each row as text, one string per row.
    ///
    /// Continuation cells contribute nothing, so a wide glyph appears once.
    /// Attributes are not represented. Intended for golden tests and
    /// debugging, not for output to a terminal.
    #[must_use]
    pub fn text_rows(&self) -> Vec<String> {
        (0..self.height)
            .map(|y| {
                self.row(y)
                    .unwrap_or_default()
                    .iter()
                    .map(Cell::symbol)
                    .collect()
            })
            .collect()
    }

    /// Copies columns `x0..x1` of row `y` from `source`. Callers pass spans
    /// aligned to wide-cell boundaries in both buffers and equal dimensions.
    pub(super) fn copy_span_from(&mut self, source: &Self, y: u16, x0: u16, x1: u16) {
        debug_assert_eq!((self.width, self.height), (source.width, source.height));
        if y >= self.height || x0 >= x1 || x1 > self.width {
            return;
        }
        let start = usize::from(y) * usize::from(self.width);
        let range = start + usize::from(x0)..start + usize::from(x1);
        self.cells[range.clone()].clone_from_slice(&source.cells[range]);
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    /// Intersects `rect` with the buffer, returning half-open bounds
    /// `(x0, y0, x1, y1)`, or `None` when the intersection is empty.
    fn clip(&self, rect: Rect) -> Option<(u16, u16, u16, u16)> {
        if rect.is_empty() {
            return None;
        }
        let x1 = (u32::from(rect.x) + u32::from(rect.width)).min(u32::from(self.width));
        let y1 = (u32::from(rect.y) + u32::from(rect.height)).min(u32::from(self.height));
        (u32::from(rect.x) < x1 && u32::from(rect.y) < y1)
            .then_some((rect.x, rect.y, x1 as u16, y1 as u16))
    }

    /// Blanks the other half of any wide pair that the cell at `index`
    /// belongs to, keeping the orphaned half's attributes. The cell at
    /// `index` itself is left for the caller to overwrite.
    fn detach_pair(&mut self, index: usize) {
        match self.cells[index].width() {
            0 if index > 0 => {
                let attrs = *self.cells[index - 1].attrs();
                self.cells[index - 1] = Cell::styled_blank(attrs);
            }
            2 if index + 1 < self.cells.len() => {
                let attrs = *self.cells[index + 1].attrs();
                self.cells[index + 1] = Cell::styled_blank(attrs);
            }
            _ => {}
        }
    }

    fn put_blank(&mut self, x: u16, y: u16, attrs: ResolvedAttributes) {
        if let Some(index) = self.index(x, y) {
            self.detach_pair(index);
            self.cells[index] = Cell::styled_blank(attrs);
        }
    }

    /// Writes one renderable cluster of `width` 1 or 2 that is known to fit
    /// inside the buffer row.
    fn put_cluster(&mut self, x: u16, y: u16, cluster: &str, width: u8, attrs: ResolvedAttributes) {
        let Some(index) = self.index(x, y) else {
            return;
        };
        self.detach_pair(index);
        self.cells[index] = Cell::from_cluster(cluster, width, attrs);
        if width == 2 {
            // Fit was checked by the caller, so `index + 1` is on this row.
            self.detach_pair(index + 1);
            self.cells[index + 1] = Cell::continuation(attrs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::ResolvedColor;

    fn plain() -> ResolvedAttributes {
        ResolvedAttributes::plain()
    }

    fn red() -> ResolvedAttributes {
        ResolvedAttributes::new(ResolvedColor::Rgb(255, 0, 0), ResolvedColor::Default)
    }

    /// The pairing invariant the whole renderer rests on.
    fn assert_consistent(buffer: &Buffer) {
        for y in 0..buffer.height() {
            let row = buffer.row(y).unwrap();
            for (x, cell) in row.iter().enumerate() {
                match cell.width() {
                    0 => {
                        assert!(x > 0, "continuation at column 0");
                        assert_eq!(row[x - 1].width(), 2, "orphan continuation at {x},{y}");
                        assert_eq!(row[x - 1].attrs(), cell.attrs());
                        assert_eq!(cell.symbol(), "");
                    }
                    2 => {
                        assert!(x + 1 < row.len(), "wide head in last column");
                        assert!(row[x + 1].is_continuation(), "headless wide at {x},{y}");
                    }
                    1 => assert!(!cell.symbol().is_empty()),
                    other => panic!("impossible width {other}"),
                }
            }
        }
    }

    #[test]
    fn new_buffer_is_all_ground_state() {
        let buffer = Buffer::new(3, 2);
        assert_eq!((buffer.width(), buffer.height()), (3, 2));
        assert_eq!(buffer.area(), Rect::new(0, 0, 3, 2));
        assert!(buffer.row(0).unwrap().iter().all(Cell::is_ground_state));
        assert_eq!(buffer.text_rows(), ["   ", "   "]);
    }

    #[test]
    fn reads_outside_the_grid_are_none_never_panics() {
        let buffer = Buffer::new(3, 2);
        assert!(buffer.get(3, 0).is_none());
        assert!(buffer.get(0, 2).is_none());
        assert!(buffer.get(u16::MAX, u16::MAX).is_none());
        assert!(buffer.row(2).is_none());
        let empty = Buffer::new(0, 0);
        assert!(empty.get(0, 0).is_none());
        assert_eq!(empty.text_rows(), Vec::<String>::new());
    }

    #[test]
    fn set_string_draws_clusters_and_reports_the_end_column() {
        let mut buffer = Buffer::new(10, 1);
        let end = buffer.set_string(1, 0, "a日e\u{301}", plain());
        assert_eq!(end, 5);
        assert_eq!(buffer.text_rows(), [" a日e\u{301}     "]);
        assert_eq!(buffer.get(2, 0).unwrap().width(), 2);
        assert!(buffer.get(3, 0).unwrap().is_continuation());
        assert_eq!(buffer.get(4, 0).unwrap().symbol(), "e\u{301}");
        assert_consistent(&buffer);
    }

    #[test]
    fn controls_and_zero_width_clusters_are_never_drawn() {
        let mut buffer = Buffer::new(8, 1);
        buffer.set_string(0, 0, "a\u{1b}[2Jb\tc\u{200b}d", plain());
        assert_eq!(buffer.text_rows(), ["a[2Jbcd "]);
        for cell in buffer.row(0).unwrap() {
            assert!(!cell.symbol().chars().any(char::is_control));
        }
    }

    #[test]
    fn truncation_pads_a_wide_glyph_with_one_column_of_room() {
        let mut buffer = Buffer::new(4, 1);
        assert_eq!(buffer.set_string(0, 0, "abc日", red()), 4);
        assert_eq!(buffer.text_rows(), ["abc "]);
        // The pad column carries the text's attributes.
        assert_eq!(buffer.get(3, 0).unwrap().attrs(), &red());
        assert_consistent(&buffer);

        let mut exact = Buffer::new(4, 1);
        assert_eq!(exact.set_string(0, 0, "ab日", plain()), 4);
        assert_eq!(exact.text_rows(), ["ab日"]);
        assert_consistent(&exact);
    }

    #[test]
    fn out_of_bounds_writes_are_clipped_not_panics() {
        let mut buffer = Buffer::new(4, 2);
        assert_eq!(buffer.set_string(9, 0, "x", plain()), 4);
        assert_eq!(buffer.set_string(0, 9, "x", plain()), 0);
        assert_eq!(
            buffer.set_string(u16::MAX, u16::MAX, "x", plain()),
            u16::MAX
        );
        assert_eq!(buffer.text_rows(), ["    ", "    "]);
        let mut empty = Buffer::new(0, 0);
        assert_eq!(empty.set_string(0, 0, "x", plain()), 0);
        empty.fill(Rect::new(0, 0, 5, 5), red());
    }

    #[test]
    fn overwriting_half_a_wide_glyph_repairs_the_other_half() {
        // Overwrite the continuation: the head must become a blank.
        let mut buffer = Buffer::new(6, 1);
        buffer.set_string(1, 0, "日", red());
        buffer.set_string(2, 0, "x", plain());
        assert_eq!(buffer.text_rows(), ["  x   "]);
        assert_eq!(buffer.get(1, 0).unwrap().attrs(), &red());
        assert_consistent(&buffer);

        // Overwrite the head with a narrow glyph: the continuation blanks.
        let mut buffer = Buffer::new(6, 1);
        buffer.set_string(1, 0, "日", red());
        buffer.set_string(1, 0, "x", plain());
        assert_eq!(buffer.text_rows(), [" x    "]);
        assert!(buffer.get(2, 0).unwrap().is_blank());
        assert_consistent(&buffer);

        // A wide glyph landing on the middle of another wide glyph.
        let mut buffer = Buffer::new(6, 1);
        buffer.set_string(0, 0, "日本", plain());
        buffer.set_string(1, 0, "語", plain());
        assert_eq!(buffer.text_rows(), [" 語   "]);
        assert_consistent(&buffer);
    }

    #[test]
    fn clipping_skips_left_and_truncates_right_on_cluster_boundaries() {
        let clip = Rect::new(2, 0, 4, 2);
        let mut buffer = Buffer::new(10, 3);
        // "ab" fall left of the clip and are skipped; "cdef" fill it.
        assert_eq!(
            buffer.set_string_clipped(0, 0, "abcdefgh", plain(), clip),
            6
        );
        assert_eq!(buffer.text_rows()[0], "  cdef    ");
        // Rows outside the clip draw nothing and report the input column.
        assert_eq!(buffer.set_string_clipped(0, 2, "zzz", plain(), clip), 0);
        assert_eq!(buffer.text_rows()[2], "          ");
        // A wide glyph straddling the left clip edge leaves one blank.
        let mut buffer = Buffer::new(10, 1);
        buffer.set_string_clipped(1, 0, "日x", red(), Rect::new(2, 0, 4, 1));
        assert_eq!(buffer.text_rows(), ["   x      "]);
        assert_eq!(buffer.get(2, 0).unwrap().attrs(), &red());
        assert_consistent(&buffer);
        // Empty and fully-outside clips are no-ops.
        let mut buffer = Buffer::new(4, 1);
        assert_eq!(
            buffer.set_string_clipped(0, 0, "x", plain(), Rect::new(0, 0, 0, 1)),
            0
        );
        assert_eq!(
            buffer.set_string_clipped(0, 0, "x", plain(), Rect::new(9, 9, 2, 2)),
            0
        );
        assert_eq!(buffer.text_rows(), ["    "]);
    }

    #[test]
    fn fill_and_set_attrs_never_tear_wide_glyphs() {
        let mut buffer = Buffer::new(6, 1);
        buffer.set_string(0, 0, "日本語", plain());
        // Fill covering only the right half of the first glyph.
        buffer.fill(Rect::new(1, 0, 2, 1), red());
        assert_eq!(buffer.text_rows(), ["    語"]);
        assert_consistent(&buffer);

        let mut buffer = Buffer::new(6, 1);
        buffer.set_string(0, 0, "日本語", plain());
        // Restyling one column of a wide glyph restyles the whole glyph.
        buffer.set_attrs(Rect::new(1, 0, 1, 1), red());
        assert_eq!(buffer.get(0, 0).unwrap().attrs(), &red());
        assert_eq!(buffer.get(1, 0).unwrap().attrs(), &red());
        assert_eq!(buffer.get(2, 0).unwrap().attrs(), &plain());
        assert_eq!(buffer.text_rows(), ["日本語"]);
        assert_consistent(&buffer);
    }

    #[test]
    fn resize_discards_content_and_clear_restores_ground_state() {
        let mut buffer = Buffer::new(4, 1);
        buffer.set_string(0, 0, "abcd", red());
        buffer.clear();
        assert!(buffer.row(0).unwrap().iter().all(Cell::is_ground_state));
        buffer.set_string(0, 0, "abcd", red());
        buffer.resize(2, 2);
        assert_eq!((buffer.width(), buffer.height()), (2, 2));
        assert_eq!(buffer.text_rows(), ["  ", "  "]);
    }

    #[test]
    fn random_writes_never_break_the_pairing_invariant() {
        let alphabet = ["a", "日", "é", "👩‍💻", "x", "한", " ", "\u{301}", "🇺🇿", "\n"];
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut buffer = Buffer::new(13, 4);
        for _ in 0..4000 {
            let mut text = String::new();
            for _ in 0..(next() % 5) {
                text.push_str(alphabet[(next() % alphabet.len() as u64) as usize]);
            }
            let (x, y) = ((next() % 16) as u16, (next() % 5) as u16);
            match next() % 4 {
                0 => {
                    buffer.fill(
                        Rect::new(
                            (next() % 14) as u16,
                            y,
                            (next() % 6) as u16,
                            (next() % 3) as u16,
                        ),
                        red(),
                    );
                }
                1 => buffer.set_attrs(
                    Rect::new(
                        (next() % 14) as u16,
                        y,
                        (next() % 6) as u16,
                        (next() % 3) as u16,
                    ),
                    red(),
                ),
                2 => {
                    buffer.set_string_clipped(x, y, &text, red(), Rect::new(2, 1, 9, 2));
                }
                _ => {
                    buffer.set_string(x, y, &text, plain());
                }
            }
            assert_consistent(&buffer);
        }
    }
}
