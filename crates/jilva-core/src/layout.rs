//! Integer cell geometry ([`Rect`]) and a one-dimensional flex solver
//! ([`Layout`]) — `docs/ARCHITECTURE.md` §3.6, `docs/RFC/0005-layout-solver.md`.
//!
//! **Status:** row/column splitting with a closed constraint vocabulary, gap,
//! alignment, a deterministic remainder rule, and a never-panic overflow
//! policy. Grid layout, the measure/arrange pass, the nesting protocol, and
//! invalidation rules are later slices. Layout never measures text; it
//! consumes already-measured sizes and produces regions, in whole cells.

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

    /// Returns the column just past the right edge (exclusive), in `u32` so
    /// it cannot wrap.
    #[must_use]
    pub const fn right(self) -> u32 {
        self.x as u32 + self.width as u32
    }

    /// Returns the row just past the bottom edge (exclusive), in `u32` so it
    /// cannot wrap.
    #[must_use]
    pub const fn bottom(self) -> u32 {
        self.y as u32 + self.height as u32
    }

    /// Returns the overlap of two rectangles, or an empty rectangle at
    /// `self`'s origin when they do not overlap.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Rect;
    ///
    /// let a = Rect::new(0, 0, 10, 10);
    /// assert_eq!(a.intersection(Rect::new(5, 5, 10, 10)), Rect::new(5, 5, 5, 5));
    /// assert!(a.intersection(Rect::new(20, 0, 3, 3)).is_empty());
    /// ```
    #[must_use]
    pub fn intersection(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if u32::from(x) >= right || u32::from(y) >= bottom {
            return Self::new(self.x, self.y, 0, 0);
        }
        // `right - x` and `bottom - y` are at most one operand's width/height.
        Self::new(
            x,
            y,
            (right - u32::from(x)) as u16,
            (bottom - u32::from(y)) as u16,
        )
    }

    /// Returns the rectangle shrunk by `margin` cells on every side.
    ///
    /// A margin that consumes the whole rectangle yields an empty rectangle
    /// at its centre; it never underflows.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Rect;
    ///
    /// assert_eq!(Rect::new(0, 0, 10, 4).inner(1), Rect::new(1, 1, 8, 2));
    /// assert!(Rect::new(0, 0, 3, 3).inner(2).is_empty());
    /// ```
    #[must_use]
    pub const fn inner(self, margin: u16) -> Self {
        let doubled = margin.saturating_mul(2);
        if doubled >= self.width || doubled >= self.height {
            return Self::new(
                self.x.saturating_add(self.width / 2),
                self.y.saturating_add(self.height / 2),
                0,
                0,
            );
        }
        Self::new(
            self.x.saturating_add(margin),
            self.y.saturating_add(margin),
            self.width - doubled,
            self.height - doubled,
        )
    }
}

/// The axis a [`Layout`] splits along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Direction {
    /// Items sit side by side, left to right; widths are constrained.
    #[default]
    Row,
    /// Items are stacked, top to bottom; heights are constrained.
    Column,
}

/// How one item is sized along the layout axis. A closed vocabulary: every
/// variant is expressible in whole cells with no float or pixel unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Constraint {
    /// Exactly this many cells (shrinks only under overflow).
    Length(u16),
    /// This percentage of the space left after gaps, rounded down; values
    /// above 100 count as 100.
    Percentage(u16),
    /// At least this many cells, then shares leftover space like `Fill(1)`.
    Min(u16),
    /// At most this many cells; shares leftover space like `Fill(1)` up to
    /// the cap.
    Max(u16),
    /// Shares leftover space in proportion to this weight. Weight `0` takes
    /// no space.
    Fill(u16),
}

/// Where unused space goes when no item can grow to absorb it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Align {
    /// Items start at the leading edge; free space trails.
    #[default]
    Start,
    /// Free space is split before and after (the odd cell goes after).
    Center,
    /// Items end at the trailing edge; free space leads.
    End,
}

/// A one-dimensional flex split: divide a [`Rect`] into one rectangle per
/// constraint along a [`Direction`].
///
/// # Rules (deterministic, integer-only)
///
/// 1. **Gaps** come off the axis first. If the gaps alone do not fit they
///    shrink uniformly (`axis / (items - 1)` each) rather than overflow.
/// 2. **Fixed sizes** are `Length(n)` and `Percentage(p)` (of the space after
///    gaps, rounded down); `Min(n)` starts at `n`, `Max` and `Fill` at `0`.
/// 3. **Leftover** is shared among growable items (`Min`, `Max`, `Fill`) by
///    weight (`Fill(w)` has weight `w`; `Min` and `Max` weight `1`). An item
///    that reaches its `Max` cap is frozen and its excess re-shared. The
///    division remainder goes one cell at a time to the largest fractional
///    shares; ties go to the **earlier** item.
/// 4. **No growable item** (or all capped): the free space is placed by
///    [`Align`].
/// 5. **Overflow** — the fixed sizes and minimums exceed the axis: later
///    items shrink first (each down to zero) until everything fits. Nothing
///    overlaps, nothing panics, and the result always has one rectangle per
///    constraint.
/// 6. The **cross axis** is the full extent of the area.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Constraint, Layout, Rect};
///
/// // "row, gap 1, sidebar 30%": a 30% sidebar and a content area.
/// let areas = Layout::row()
///     .constraints([Constraint::Percentage(30), Constraint::Fill(1)])
///     .gap(1)
///     .split(Rect::new(0, 0, 80, 24));
/// assert_eq!(areas, [Rect::new(0, 0, 23, 24), Rect::new(24, 0, 56, 24)]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    direction: Direction,
    constraints: Vec<Constraint>,
    gap: u16,
    align: Align,
}

impl Layout {
    /// Starts a horizontal layout (widths are constrained).
    #[must_use]
    pub fn row() -> Self {
        Self::default()
    }

    /// Starts a vertical layout (heights are constrained).
    #[must_use]
    pub fn column() -> Self {
        Self {
            direction: Direction::Column,
            ..Self::default()
        }
    }

    /// Replaces the constraints, one per output rectangle.
    #[must_use]
    pub fn constraints(mut self, constraints: impl IntoIterator<Item = Constraint>) -> Self {
        self.constraints = constraints.into_iter().collect();
        self
    }

    /// Sets the empty cells between neighbouring items.
    #[must_use]
    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// Sets where free space goes when no item can absorb it.
    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Returns the layout direction.
    #[must_use]
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Splits `area` into one rectangle per constraint, in order.
    ///
    /// See the type documentation for the rules. Always returns exactly one
    /// rectangle per constraint, all inside `area`, non-overlapping and
    /// ordered along the axis.
    #[must_use]
    pub fn split(&self, area: Rect) -> Vec<Rect> {
        if self.constraints.is_empty() {
            return Vec::new();
        }
        let axis = match self.direction {
            Direction::Row => area.width,
            Direction::Column => area.height,
        };
        let solved = solve(&self.constraints, axis, self.gap, self.align);
        let mut cursor = u32::from(solved.lead);
        let mut out = Vec::with_capacity(solved.sizes.len());
        for size in solved.sizes {
            out.push(match self.direction {
                Direction::Row => Rect::new(
                    clamp_u16(u32::from(area.x) + cursor),
                    area.y,
                    size,
                    area.height,
                ),
                Direction::Column => Rect::new(
                    area.x,
                    clamp_u16(u32::from(area.y) + cursor),
                    area.width,
                    size,
                ),
            });
            cursor += u32::from(size) + u32::from(solved.gap);
        }
        out
    }
}

fn clamp_u16(value: u32) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

/// Per-item sizing state for [`solve`].
struct Item {
    size: u32,
    max: u32,
    weight: u32,
}

impl Item {
    fn growable(&self) -> bool {
        self.weight > 0 && self.size < self.max
    }
}

struct Solved {
    sizes: Vec<u16>,
    gap: u16,
    lead: u16,
}

/// Solves `constraints` over `axis` cells. `constraints` must be non-empty.
fn solve(constraints: &[Constraint], axis: u16, gap: u16, align: Align) -> Solved {
    let gaps = constraints.len() - 1;
    let gap = match u16::try_from(gaps) {
        Ok(0) => 0,
        Ok(n) => gap.min(axis / n),
        Err(_) => 0,
    };
    let gap_total = u32::from(gap) * gaps as u32;
    let available = u32::from(axis).saturating_sub(gap_total);

    let mut items: Vec<Item> = constraints
        .iter()
        .map(|constraint| match *constraint {
            Constraint::Length(n) => Item {
                size: u32::from(n),
                max: u32::from(n),
                weight: 0,
            },
            Constraint::Percentage(p) => {
                let size = available * u32::from(p.min(100)) / 100;
                Item {
                    size,
                    max: size,
                    weight: 0,
                }
            }
            Constraint::Min(n) => Item {
                size: u32::from(n),
                max: u32::MAX,
                weight: 1,
            },
            Constraint::Max(n) => Item {
                size: 0,
                max: u32::from(n),
                weight: 1,
            },
            Constraint::Fill(w) => Item {
                size: 0,
                max: u32::MAX,
                weight: u32::from(w),
            },
        })
        .collect();

    let used: u32 = items.iter().map(|item| item.size).sum();
    if used > available {
        // Overflow: later items yield first.
        let mut deficit = used - available;
        for item in items.iter_mut().rev() {
            let cut = item.size.min(deficit);
            item.size -= cut;
            deficit -= cut;
            if deficit == 0 {
                break;
            }
        }
    } else {
        share(&mut items, available - used);
    }

    let used = items.iter().map(|item| item.size).sum::<u32>() + gap_total;
    let free = u32::from(axis).saturating_sub(used);
    let lead = if items.iter().any(Item::growable) {
        0
    } else {
        match align {
            Align::Start => 0,
            Align::Center => free / 2,
            Align::End => free,
        }
    };
    Solved {
        sizes: items.iter().map(|item| clamp_u16(item.size)).collect(),
        gap,
        lead: clamp_u16(lead),
    }
}

/// Shares `leftover` cells among growable items by weight with capped
/// water-filling and a largest-remainder rule (ties to the earlier item).
fn share(items: &mut [Item], mut leftover: u32) {
    loop {
        let active: Vec<usize> = (0..items.len()).filter(|&i| items[i].growable()).collect();
        if active.is_empty() || leftover == 0 {
            return;
        }
        let total: u64 = active.iter().map(|&i| u64::from(items[i].weight)).sum();
        let left = u64::from(leftover);

        // Freeze every item whose proportional share would reach its cap.
        let capped: Vec<usize> = active
            .iter()
            .copied()
            .filter(|&i| {
                let room = u64::from(items[i].max - items[i].size);
                left * u64::from(items[i].weight) >= room * total
            })
            .collect();
        if !capped.is_empty() {
            for i in capped {
                let room = items[i].max - items[i].size;
                items[i].size = items[i].max;
                leftover -= room;
            }
            continue;
        }

        // No cap binds: floor shares, then the largest remainders.
        let mut remainders: Vec<(u64, usize)> = Vec::with_capacity(active.len());
        let mut granted = 0u32;
        for &i in &active {
            let numerator = left * u64::from(items[i].weight);
            let whole = (numerator / total) as u32;
            items[i].size += whole;
            granted += whole;
            remainders.push((numerator % total, i));
        }
        let mut extra = leftover - granted;
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for &(_, i) in &remainders {
            if extra == 0 {
                break;
            }
            items[i].size += 1;
            extra -= 1;
        }
        return;
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

    #[test]
    fn edges_and_intersection_cannot_wrap() {
        let far = Rect::new(u16::MAX, u16::MAX, u16::MAX, u16::MAX);
        assert_eq!(far.right(), 2 * u32::from(u16::MAX));
        assert_eq!(far.bottom(), 2 * u32::from(u16::MAX));
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(a.intersection(a), a);
        assert_eq!(a.intersection(Rect::new(9, 9, 5, 5)), Rect::new(9, 9, 1, 1));
        // Touching edges do not overlap.
        assert!(a.intersection(Rect::new(10, 0, 5, 5)).is_empty());
        assert!(a.intersection(far).is_empty());
        assert!(a.intersection(Rect::new(0, 0, 0, 5)).is_empty());
    }

    #[test]
    fn inner_never_underflows() {
        assert_eq!(Rect::new(2, 3, 10, 6).inner(0), Rect::new(2, 3, 10, 6));
        assert_eq!(Rect::new(2, 3, 10, 6).inner(2), Rect::new(4, 5, 6, 2));
        for margin in [3, 4, 100, u16::MAX] {
            let shrunk = Rect::new(0, 0, 10, 6).inner(margin);
            assert!(shrunk.is_empty());
            assert!(shrunk.x <= 10 && shrunk.y <= 6);
        }
        assert!(Rect::new(0, 0, 0, 0).inner(1).is_empty());
        assert!(Rect::new(u16::MAX, u16::MAX, 3, 3).inner(1).x == u16::MAX);
    }

    fn widths(layout: &Layout, cols: u16) -> Vec<u16> {
        layout
            .split(Rect::new(0, 0, cols, 5))
            .iter()
            .map(|rect| rect.width)
            .collect()
    }

    fn row(constraints: &[Constraint]) -> Layout {
        Layout::row().constraints(constraints.iter().copied())
    }

    #[test]
    fn no_constraints_means_no_rectangles() {
        assert!(Layout::row().split(Rect::new(0, 0, 80, 24)).is_empty());
    }

    #[test]
    fn fixed_and_fill_constraints_resolve_to_whole_cells() {
        let layout = row(&[
            Constraint::Length(10),
            Constraint::Fill(1),
            Constraint::Length(5),
        ]);
        assert_eq!(widths(&layout, 40), [10, 25, 5]);
        let areas = layout.split(Rect::new(3, 4, 40, 7));
        assert_eq!(areas[0], Rect::new(3, 4, 10, 7));
        assert_eq!(areas[1], Rect::new(13, 4, 25, 7));
        assert_eq!(areas[2], Rect::new(38, 4, 5, 7));
    }

    #[test]
    fn column_splits_heights_and_keeps_the_full_width() {
        let areas = Layout::column()
            .constraints([
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .split(Rect::new(5, 2, 30, 10));
        assert_eq!(
            areas,
            [
                Rect::new(5, 2, 30, 1),
                Rect::new(5, 3, 30, 8),
                Rect::new(5, 11, 30, 1),
            ]
        );
    }

    #[test]
    fn percentages_round_down_of_the_space_after_gaps() {
        let layout = row(&[Constraint::Percentage(50), Constraint::Percentage(50)]).gap(1);
        // 21 - 1 gap = 20 -> 10 + 10.
        assert_eq!(widths(&layout, 21), [10, 10]);
        // 22 - 1 gap = 21 -> 10 + 10 with one free cell (Align::Start trails).
        assert_eq!(widths(&layout, 22), [10, 10]);
        // Above 100 counts as 100.
        assert_eq!(widths(&row(&[Constraint::Percentage(900)]), 7), [7]);
        assert_eq!(widths(&row(&[Constraint::Percentage(0)]), 7), [0]);
    }

    #[test]
    fn the_documented_sidebar_example_holds() {
        let areas = Layout::row()
            .constraints([Constraint::Percentage(30), Constraint::Fill(1)])
            .gap(1)
            .split(Rect::new(0, 0, 80, 24));
        assert_eq!(areas, [Rect::new(0, 0, 23, 24), Rect::new(24, 0, 56, 24)]);
    }

    #[test]
    fn remainder_goes_to_the_earliest_items_on_equal_weight() {
        // 10 cells over three equal fills: 4, 3, 3 (not 3, 3, 4).
        let layout = row(&[Constraint::Fill(1); 3]);
        assert_eq!(widths(&layout, 10), [4, 3, 3]);
        assert_eq!(widths(&layout, 11), [4, 4, 3]);
        assert_eq!(widths(&layout, 12), [4, 4, 4]);
        // Same input, same output.
        assert_eq!(widths(&layout, 10), widths(&layout, 10));
    }

    #[test]
    fn remainder_follows_the_largest_fraction_not_the_index() {
        // 7 cells over weights 1:2 -> exact 2.33 / 4.67 -> 2 + 4 floors,
        // the spare cell goes to the larger fraction (second item).
        let layout = row(&[Constraint::Fill(1), Constraint::Fill(2)]);
        assert_eq!(widths(&layout, 7), [2, 5]);
        // Weight 0 takes nothing.
        let layout = row(&[Constraint::Fill(0), Constraint::Fill(1)]);
        assert_eq!(widths(&layout, 9), [0, 9]);
    }

    #[test]
    fn min_and_max_bound_their_share() {
        let layout = row(&[Constraint::Min(10), Constraint::Fill(1)]);
        // Min takes 10 and then shares: (30-10)=20 split evenly -> 20 + 10.
        assert_eq!(widths(&layout, 30), [20, 10]);
        // Max caps; the excess goes to the other growable item.
        let layout = row(&[Constraint::Max(4), Constraint::Fill(1)]);
        assert_eq!(widths(&layout, 30), [4, 26]);
        // All capped: free space is placed by alignment.
        let layout = row(&[Constraint::Max(4), Constraint::Max(6)]);
        assert_eq!(widths(&layout, 30), [4, 6]);
    }

    #[test]
    fn alignment_places_free_space_only_when_nothing_can_grow() {
        let fixed = [Constraint::Length(4), Constraint::Length(4)];
        let at = |align| {
            Layout::row()
                .constraints(fixed)
                .gap(1)
                .align(align)
                .split(Rect::new(10, 0, 20, 1))
                .iter()
                .map(|rect| rect.x)
                .collect::<Vec<_>>()
        };
        // 4 + 1 + 4 = 9 used, 11 free.
        assert_eq!(at(Align::Start), [10, 15]);
        assert_eq!(at(Align::Center), [15, 20]); // 11 / 2 = 5 before
        assert_eq!(at(Align::End), [21, 26]);
        // A growable item absorbs the space, so alignment is irrelevant.
        let grow = Layout::row()
            .constraints([Constraint::Length(4), Constraint::Fill(1)])
            .align(Align::End)
            .split(Rect::new(0, 0, 20, 1));
        assert_eq!(grow[0].x, 0);
        assert_eq!(grow[1].right(), 20);
    }

    #[test]
    fn gaps_larger_than_the_axis_shrink_instead_of_overflowing() {
        let layout = row(&[Constraint::Fill(1); 3]).gap(50);
        let areas = layout.split(Rect::new(0, 0, 10, 1));
        assert_eq!(areas.len(), 3);
        assert!(areas.iter().all(|rect| rect.right() <= 10));
        // Zero-width axis and zero gap handling.
        assert!(
            row(&[Constraint::Fill(1)])
                .gap(5)
                .split(Rect::new(0, 0, 0, 3))[0]
                .is_empty()
        );
    }

    #[test]
    fn overflow_makes_later_items_yield_first() {
        let layout = row(&[
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ]);
        assert_eq!(widths(&layout, 20), [8, 8, 4]);
        assert_eq!(widths(&layout, 10), [8, 2, 0]);
        assert_eq!(widths(&layout, 0), [0, 0, 0]);
        // Minimums are floors for growth, not protection from overflow.
        let layout = row(&[Constraint::Min(15), Constraint::Min(15)]);
        assert_eq!(widths(&layout, 20), [15, 5]);
    }

    struct Rng(u64);

    impl Rng {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % bound
        }
    }

    #[test]
    fn split_always_yields_ordered_disjoint_rectangles_inside_the_area() {
        let mut rng = Rng(0x00c0_ffee_1234_5678);
        for _ in 0..20_000 {
            let count = rng.below(8) as usize;
            let constraints: Vec<Constraint> = (0..count)
                .map(|_| {
                    let n = rng.below(40) as u16;
                    match rng.below(5) {
                        0 => Constraint::Length(n),
                        1 => Constraint::Percentage(n * 3),
                        2 => Constraint::Min(n),
                        3 => Constraint::Max(n),
                        _ => Constraint::Fill(rng.below(5) as u16),
                    }
                })
                .collect();
            let layout = if rng.below(2) == 0 {
                Layout::row()
            } else {
                Layout::column()
            }
            .constraints(constraints.clone())
            .gap(rng.below(6) as u16)
            .align([Align::Start, Align::Center, Align::End][rng.below(3) as usize]);
            let area = Rect::new(
                rng.below(100) as u16,
                rng.below(100) as u16,
                rng.below(120) as u16,
                rng.below(120) as u16,
            );
            let areas = layout.split(area);
            assert_eq!(areas.len(), constraints.len());
            assert_eq!(areas, layout.split(area), "deterministic");
            let (start, end, mut cursor) = match layout.direction() {
                Direction::Row => (area.x, area.right(), u32::from(area.x)),
                Direction::Column => (area.y, area.bottom(), u32::from(area.y)),
            };
            for (rect, constraint) in areas.iter().zip(&constraints) {
                let (lo, hi, cross_lo, cross_len, size) = match layout.direction() {
                    Direction::Row => (rect.x, rect.right(), rect.y, rect.height, rect.width),
                    Direction::Column => (rect.y, rect.bottom(), rect.x, rect.width, rect.height),
                };
                assert!(u32::from(lo) >= cursor, "overlap or disorder: {areas:?}");
                assert!(
                    u32::from(lo) >= u32::from(start) && hi <= end,
                    "outside area"
                );
                cursor = hi;
                let (want_lo, want_len) = match layout.direction() {
                    Direction::Row => (area.y, area.height),
                    Direction::Column => (area.x, area.width),
                };
                assert_eq!((cross_lo, cross_len), (want_lo, want_len));
                if let Constraint::Max(cap) = constraint {
                    assert!(size <= *cap);
                }
                if let Constraint::Length(n) = constraint {
                    assert!(size <= *n);
                }
            }
            // With room to spare, fixed lengths are honoured exactly.
            let fixed: u32 = constraints
                .iter()
                .map(|c| match c {
                    Constraint::Length(n) | Constraint::Min(n) => u32::from(*n),
                    _ => 0,
                })
                .sum();
            let axis = match layout.direction() {
                Direction::Row => area.width,
                Direction::Column => area.height,
            };
            let no_percent = constraints
                .iter()
                .all(|c| !matches!(c, Constraint::Percentage(_)));
            if no_percent && fixed + 6 * constraints.len() as u32 <= u32::from(axis) {
                for (rect, constraint) in areas.iter().zip(&constraints) {
                    if let Constraint::Length(n) = constraint {
                        let size = match layout.direction() {
                            Direction::Row => rect.width,
                            Direction::Column => rect.height,
                        };
                        assert_eq!(size, *n);
                    }
                }
            }
        }
    }
}
