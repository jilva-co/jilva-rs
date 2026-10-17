//! Diffing and damage tracking: buffers in, instruction stream out
//! (RFC-0004 §5–§6).
//!
//! [`diff`] is a pure function of `(previous buffer, next buffer, damage,
//! capability profile, options)`. It touches no terminal and reads no ambient
//! state, so golden tests run headless.
//!
//! # Stream contract
//!
//! Every stream returned here:
//!
//! 1. **starts** assuming default attributes and an unknown cursor position,
//!    so it never depends on what an earlier stream left behind;
//! 2. **ends** with default attributes (a trailing
//!    [`OutputCommand::ResetAttributes`] when anything styled was drawn);
//! 3. contains only commands that validate for the next buffer's size;
//! 4. is empty when nothing visible changed.
//!
//! # Equality is *effective* equality
//!
//! Two cells are the same when their symbol matches and their attributes are
//! equal *after* downgrade to the capability profile. A colour change that
//! the terminal cannot show (two RGB values that collapse to one ANSI colour,
//! or any colour under `NO_COLOR`) emits nothing.

use super::buffer::Buffer;
use super::cell::Cell;
use crate::capabilities::TerminalCapabilities;
use crate::layout::Rect;
use crate::output::{OutputCommand, ResolvedAttributes};

/// Which parts of the screen may differ from the previous frame.
///
/// Damage is a *promise from the caller*: cells outside the damaged area are
/// guaranteed identical in both buffers and are never compared. Under-reporting
/// leaves stale cells on screen; over-reporting only costs comparison time.
/// When unsure, use [`Damage::Full`] — the default.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Damage, Rect};
///
/// let mut damage = Damage::none();
/// damage.add(Rect::new(0, 0, 10, 1));
/// damage.add(Rect::new(5, 0, 10, 3)); // overlap is fine
/// assert!(!damage.is_empty());
/// assert!(Damage::none().is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Damage {
    /// Every cell may have changed.
    #[default]
    Full,
    /// Only cells inside these rectangles may have changed. Rectangles may
    /// overlap, touch, or extend past the buffer (they are clipped).
    Regions(Vec<Rect>),
}

impl Damage {
    /// Returns damage covering the whole screen.
    #[must_use]
    pub const fn full() -> Self {
        Self::Full
    }

    /// Returns damage covering nothing: the previous frame is still correct.
    #[must_use]
    pub const fn none() -> Self {
        Self::Regions(Vec::new())
    }

    /// Builds damage from rectangles.
    #[must_use]
    pub fn from_rects(rects: impl IntoIterator<Item = Rect>) -> Self {
        Self::Regions(rects.into_iter().filter(|rect| !rect.is_empty()).collect())
    }

    /// Adds a damaged rectangle. A no-op for [`Damage::Full`] and for empty
    /// rectangles.
    pub fn add(&mut self, rect: Rect) {
        if let Self::Regions(rects) = self
            && !rect.is_empty()
        {
            rects.push(rect);
        }
    }

    /// Returns `true` when no cell is damaged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Regions(rects) if rects.is_empty())
    }
}

/// Tunables for [`diff`]. Defaults favour fewer bytes on a trusting terminal.
///
/// # Example
///
/// ```rust
/// use jilva_core::DiffOptions;
///
/// let careful = DiffOptions::default().with_resync_after_wide(true);
/// assert!(careful.resync_after_wide());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiffOptions {
    bridge_gap: u16,
    resync_after_wide: bool,
}

impl DiffOptions {
    /// Default maximum gap, in columns, that the diff bridges by re-printing
    /// unchanged cells instead of moving the cursor. A cursor move costs
    /// roughly six to ten bytes on the wire, so short gaps are cheaper to
    /// print through.
    pub const DEFAULT_BRIDGE_GAP: u16 = 4;

    /// Returns the maximum bridged gap in columns (`0` disables bridging).
    #[must_use]
    pub const fn bridge_gap(&self) -> u16 {
        self.bridge_gap
    }

    /// Returns whether the cursor is re-positioned after every wide glyph.
    #[must_use]
    pub const fn resync_after_wide(&self) -> bool {
        self.resync_after_wide
    }

    /// Returns these options with the bridged-gap limit replaced.
    #[must_use]
    pub const fn with_bridge_gap(mut self, columns: u16) -> Self {
        self.bridge_gap = columns;
        self
    }

    /// Returns these options with wide-glyph resynchronisation set.
    ///
    /// When `true`, an explicit cursor move follows every double-width glyph.
    /// This costs bytes but stops a terminal whose width table disagrees with
    /// `unicode-width` from shifting the rest of the row: the disagreement
    /// damages one glyph, not the line.
    #[must_use]
    pub const fn with_resync_after_wide(mut self, enabled: bool) -> Self {
        self.resync_after_wide = enabled;
        self
    }
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            bridge_gap: Self::DEFAULT_BRIDGE_GAP,
            resync_after_wide: false,
        }
    }
}

/// Computes the instruction stream that turns what `previous` shows into what
/// `next` shows.
///
/// Only cells inside `damage` are compared. If the two buffers differ in
/// size, `previous` says nothing about the screen and the whole of `next` is
/// redrawn after a clear. Output follows the stream contract in the module
/// documentation.
///
/// # Example
///
/// ```rust
/// use jilva_core::{
///     Buffer, Damage, DiffOptions, OutputCommand, ResolvedAttributes, TerminalCapabilities, diff,
/// };
///
/// let before = Buffer::new(8, 1);
/// let mut after = before.clone();
/// after.set_string(2, 0, "hi", ResolvedAttributes::plain());
///
/// let caps = TerminalCapabilities::conservative(1);
/// let stream = diff(&before, &after, &Damage::Full, &caps, &DiffOptions::default());
/// assert_eq!(
///     stream,
///     [OutputCommand::move_to(2, 0), OutputCommand::print_segment("hi")]
/// );
/// ```
#[must_use]
pub fn diff(
    previous: &Buffer,
    next: &Buffer,
    damage: &Damage,
    caps: &TerminalCapabilities,
    options: &DiffOptions,
) -> Vec<OutputCommand> {
    if next.width() == 0 || next.height() == 0 {
        return Vec::new();
    }
    if (previous.width(), previous.height()) != (next.width(), next.height()) {
        return redraw(next, caps, options);
    }
    let mut emitter = Emitter::new(next.width());
    let mut spans = Vec::new();
    let (first_row, last_row) = row_bounds(damage, next.height());
    for y in first_row..last_row {
        let (Some(old_row), Some(new_row)) = (previous.row(y), next.row(y)) else {
            continue;
        };
        damage_spans(damage, y, old_row, new_row, &mut spans);
        let row = Row {
            y,
            old: old_row,
            new: new_row,
            caps,
            options,
        };
        for &(start, end) in &spans {
            diff_span(&mut emitter, &row, start, end);
        }
    }
    emitter.finish()
}

/// Returns the instruction stream that draws `next` onto an unknown screen:
/// reset, clear, then every non-ground cell.
///
/// This is what [`diff`] falls back to on a size change and what the
/// [`crate::renderer::Renderer`] uses after an invalidation.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Buffer, DiffOptions, OutputCommand, Rect, TerminalCapabilities, redraw};
///
/// let screen = Buffer::new(4, 2);
/// let caps = TerminalCapabilities::conservative(1);
/// let stream = redraw(&screen, &caps, &DiffOptions::default());
/// assert_eq!(
///     stream,
///     [OutputCommand::ResetAttributes, OutputCommand::clear_region(Rect::new(0, 0, 4, 2))]
/// );
/// ```
#[must_use]
pub fn redraw(
    next: &Buffer,
    caps: &TerminalCapabilities,
    options: &DiffOptions,
) -> Vec<OutputCommand> {
    if next.width() == 0 || next.height() == 0 {
        return Vec::new();
    }
    let ground = Buffer::new(next.width(), next.height());
    let mut stream = vec![
        OutputCommand::ResetAttributes,
        OutputCommand::clear_region(next.area()),
    ];
    stream.extend(diff(&ground, next, &Damage::Full, caps, options));
    stream
}

/// Returns the half-open row range that `damage` can touch.
pub(super) fn row_bounds(damage: &Damage, height: u16) -> (u16, u16) {
    match damage {
        Damage::Full => (0, height),
        Damage::Regions(rects) => {
            let mut first = u32::from(height);
            let mut last = 0u32;
            for rect in rects.iter().filter(|rect| !rect.is_empty()) {
                first = first.min(u32::from(rect.y));
                last = last.max(u32::from(rect.y) + u32::from(rect.height));
            }
            let last = last.min(u32::from(height));
            if first >= last {
                (0, 0)
            } else {
                (first as u16, last as u16)
            }
        }
    }
}

/// Collects the disjoint, sorted column spans of row `y` that `damage`
/// covers, widened so no span begins or ends in the middle of a wide glyph in
/// either buffer.
pub(super) fn damage_spans(
    damage: &Damage,
    y: u16,
    old_row: &[Cell],
    new_row: &[Cell],
    out: &mut Vec<(u16, u16)>,
) {
    out.clear();
    let width = new_row.len().min(old_row.len());
    let Ok(width) = u16::try_from(width) else {
        return;
    };
    match damage {
        Damage::Full => out.push((0, width)),
        Damage::Regions(rects) => {
            for rect in rects {
                let top = u32::from(rect.y);
                if rect.is_empty()
                    || u32::from(y) < top
                    || u32::from(y) >= top + u32::from(rect.height)
                {
                    continue;
                }
                let start = rect.x.min(width);
                let end = (u32::from(rect.x) + u32::from(rect.width)).min(u32::from(width)) as u16;
                if start >= end {
                    continue;
                }
                let start = if start > 0
                    && (new_row[usize::from(start)].is_continuation()
                        || old_row[usize::from(start)].is_continuation())
                {
                    start - 1
                } else {
                    start
                };
                let end = if end < width
                    && (new_row[usize::from(end) - 1].width() == 2
                        || old_row[usize::from(end) - 1].width() == 2)
                {
                    end + 1
                } else {
                    end
                };
                out.push((start, end));
            }
            out.sort_unstable();
            let mut kept = 0;
            for index in 0..out.len() {
                let (start, end) = out[index];
                if kept > 0 && start <= out[kept - 1].1 {
                    out[kept - 1].1 = out[kept - 1].1.max(end);
                } else {
                    out[kept] = (start, end);
                    kept += 1;
                }
            }
            out.truncate(kept);
        }
    }
}

/// Everything [`diff_span`] needs to know about one row.
struct Row<'a> {
    y: u16,
    old: &'a [Cell],
    new: &'a [Cell],
    caps: &'a TerminalCapabilities,
    options: &'a DiffOptions,
}

/// Diffs columns `start..end` of one row. `start` and `end` are aligned to
/// wide-glyph boundaries by [`damage_spans`].
fn diff_span(emitter: &mut Emitter, row: &Row<'_>, start: u16, end: u16) {
    let changed =
        |x: u16| cell_changed(&row.old[usize::from(x)], &row.new[usize::from(x)], row.caps);
    let mut x = start;
    while x < end {
        if !changed(x) {
            x += 1;
            continue;
        }
        // A run: print `x`, then keep printing while the next change is
        // adjacent or close enough to bridge.
        let mut cursor = x;
        loop {
            let cell = &row.new[usize::from(cursor)];
            emitter.put(cursor, row, cell);
            cursor += u16::from(cell.width().max(1));
            let mut next_change = cursor;
            while next_change < end && !changed(next_change) {
                next_change += 1;
            }
            if next_change >= end {
                x = end;
                break;
            }
            let gap = next_change - cursor;
            if gap == 0 {
                continue;
            }
            let bridgeable = gap <= row.options.bridge_gap()
                && !row.options.resync_after_wide()
                && emitter.can_bridge(
                    &row.new[usize::from(cursor)..usize::from(next_change)],
                    &row.new[usize::from(next_change)],
                    row.caps,
                );
            if !bridgeable {
                x = next_change;
                break;
            }
            while cursor < next_change {
                let cell = &row.new[usize::from(cursor)];
                emitter.put(cursor, row, cell);
                cursor += u16::from(cell.width().max(1));
            }
        }
    }
}

/// Returns `true` when `new` must be sent to make the screen show it.
///
/// Continuation cells are never "changed" on their own: the wide head to their
/// left carries them, and the buffer invariant guarantees a head change is
/// seen first.
fn cell_changed(old: &Cell, new: &Cell, caps: &TerminalCapabilities) -> bool {
    if new.is_continuation() || old == new {
        return false;
    }
    old.symbol() != new.symbol() || effective(old.attrs(), caps) != effective(new.attrs(), caps)
}

/// Resolves attributes against what the terminal can show: colours downgrade
/// to the tier, unsupported decorations drop.
pub(super) fn effective(
    attrs: &ResolvedAttributes,
    caps: &TerminalCapabilities,
) -> ResolvedAttributes {
    let support = caps.color.attrs;
    let mut out = attrs.downgraded_to(caps.color.color);
    out.bold &= support.bold;
    out.dim &= support.dim;
    out.italic &= support.italic;
    out.underline &= support.underline;
    out.reverse &= support.reverse;
    out.strikethrough &= support.strikethrough;
    out
}

/// Accumulates commands, tracking the cursor and the attributes the terminal
/// will hold after the commands so far.
struct Emitter {
    commands: Vec<OutputCommand>,
    text: String,
    width: u16,
    cursor: Option<(u16, u16)>,
    attrs: ResolvedAttributes,
}

impl Emitter {
    fn new(width: u16) -> Self {
        Self {
            commands: Vec::new(),
            text: String::new(),
            width,
            cursor: None,
            attrs: ResolvedAttributes::plain(),
        }
    }

    fn flush_text(&mut self) {
        if !self.text.is_empty() {
            self.commands
                .push(OutputCommand::print_segment(std::mem::take(&mut self.text)));
        }
    }

    /// Prints one cell at `(x, y)`, moving the cursor and switching
    /// attributes only when needed.
    fn put(&mut self, x: u16, row: &Row<'_>, cell: &Cell) {
        let y = row.y;
        if self.cursor != Some((x, y)) {
            self.flush_text();
            self.commands.push(OutputCommand::move_to(x, y));
        }
        let attrs = effective(cell.attrs(), row.caps);
        if attrs != self.attrs {
            self.flush_text();
            self.commands.push(if attrs == ResolvedAttributes::plain() {
                OutputCommand::ResetAttributes
            } else {
                OutputCommand::SetAttributes(attrs)
            });
            self.attrs = attrs;
        }
        self.text.push_str(cell.symbol());
        let after = u32::from(x) + u32::from(cell.width());
        // Past the last column the terminal's pending-wrap state is not
        // portable; forget the cursor so the next print repositions.
        self.cursor = if after >= u32::from(self.width)
            || (cell.width() == 2 && row.options.resync_after_wide())
        {
            None
        } else {
            Some((after as u16, y))
        };
    }

    /// A gap can be printed through when that adds text and no attribute
    /// switch the stream would not need anyway: every unchanged cell in it
    /// carries either the attributes currently set or the attributes of the
    /// next changed cell (which the stream must switch to regardless).
    fn can_bridge(&self, gap: &[Cell], target: &Cell, caps: &TerminalCapabilities) -> bool {
        let target = effective(target.attrs(), caps);
        let mut cells = gap.iter().filter(|cell| !cell.is_continuation());
        cells.all(|cell| effective(cell.attrs(), caps) == self.attrs)
            || gap
                .iter()
                .filter(|cell| !cell.is_continuation())
                .all(|cell| effective(cell.attrs(), caps) == target)
    }

    fn finish(mut self) -> Vec<OutputCommand> {
        self.flush_text();
        if self.attrs != ResolvedAttributes::plain() {
            self.commands.push(OutputCommand::ResetAttributes);
        }
        self.commands
    }
}
