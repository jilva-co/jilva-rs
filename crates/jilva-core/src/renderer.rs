//! The rendering engine: cell buffer, diff, and damage tracking
//! (`docs/README.md#rendering-and-layout`).
//!
//! ```text
//!  paint ──► back Buffer ──► diff(front, back, damage, caps) ──► Vec<OutputCommand>
//!                                │                                     │
//!                          front Buffer ◄── synced after present       ▼
//!                      (what the terminal shows)                  Backend::submit
//! ```
//!
//! Rendering is a pure function of explicit inputs: the previous buffer, the
//! next buffer, the damage the caller reports, and a capability profile.
//! There are no hidden globals and no terminal I/O here; the output is plain
//! [`crate::OutputCommand`] data a backend executes.
//!
//! | Piece | Role |
//! |---|---|
//! | [`Cell`] | one grapheme cluster + attributes + column width |
//! | [`Buffer`] | owned grid of cells; keeps wide glyphs whole |
//! | [`Damage`] | caller's promise of which cells may have changed |
//! | [`diff`] / [`redraw`] | pure buffer comparison → instruction stream |
//! | [`Renderer`] | double buffer + invalidation around [`diff`] |
//! | [`cluster_width`], [`graphemes`], [`text_width`], [`truncate_to_width`] | the width/segmentation authorities |
//!
//! Layout, components, and theming are *not* here: this module consumes
//! already-resolved attributes and already-placed text.

mod buffer;
mod cell;
mod diff;
mod width;

pub use buffer::Buffer;
pub use cell::Cell;
pub use diff::{Damage, DiffOptions, diff, redraw};
pub use width::{cluster_width, graphemes, text_width, truncate_to_width};

use crate::capabilities::{ColorProfile, TerminalCapabilities};
use crate::output::OutputCommand;
use crate::view::View;

/// A double-buffered renderer: paint into the back buffer, then
/// [`Renderer::present`] to get the instruction stream that updates the
/// terminal.
///
/// The *front* buffer is the renderer's belief about what the terminal shows.
/// The *back* buffer is where the next frame is painted. After
/// [`Renderer::present`] the back buffer keeps its content, so a retained-mode
/// caller may repaint only damaged regions; an immediate-mode caller calls
/// [`Buffer::clear`] on it first.
///
/// # Belief versus reality
///
/// The front buffer is only as accurate as the caller's delivery of the
/// streams. When a submitted stream may not have reached the terminal — a
/// failed `submit` or `flush`, a suspend/resume, an external program writing
/// to the screen — call [`Renderer::invalidate`]; the next present clears the
/// screen and redraws everything. A resize ([`Renderer::resize`]) and a change
/// of colour capability do this automatically.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Damage, OutputCommand, Renderer, ResolvedAttributes, TerminalCapabilities};
///
/// let caps = TerminalCapabilities::conservative(1);
/// let mut renderer = Renderer::new(10, 2);
///
/// // The first present draws onto an unknown screen: clear, then content.
/// renderer.back_buffer_mut().set_string(0, 0, "hello", ResolvedAttributes::plain());
/// let first = renderer.present(&Damage::Full, &caps);
/// assert_eq!(first[0], OutputCommand::ResetAttributes);
///
/// // Change one letter: only that cell crosses the seam.
/// renderer.back_buffer_mut().set_string(1, 0, "a", ResolvedAttributes::plain());
/// let second = renderer.present(&Damage::Full, &caps);
/// assert_eq!(second, [OutputCommand::move_to(1, 0), OutputCommand::print_segment("a")]);
///
/// // Nothing changed: nothing is emitted.
/// assert!(renderer.present(&Damage::Full, &caps).is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct Renderer {
    front: Buffer,
    back: Buffer,
    options: DiffOptions,
    color: Option<ColorProfile>,
    needs_redraw: bool,
    spans: Vec<(u16, u16)>,
}

impl Renderer {
    /// Creates a renderer for a `width` × `height` screen whose content is
    /// unknown: the first present clears and redraws.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            front: Buffer::new(width, height),
            back: Buffer::new(width, height),
            options: DiffOptions::default(),
            color: None,
            needs_redraw: true,
            spans: Vec::new(),
        }
    }

    /// Returns this renderer with `options` replacing the diff tunables.
    #[must_use]
    pub const fn with_options(mut self, options: DiffOptions) -> Self {
        self.options = options;
        self
    }

    /// Returns the screen size in `(columns, rows)`.
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        (self.back.width(), self.back.height())
    }

    /// Returns the buffer the next frame is painted into.
    #[must_use]
    pub const fn back_buffer(&self) -> &Buffer {
        &self.back
    }

    /// Returns the buffer the next frame is painted into, mutably.
    pub const fn back_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.back
    }

    /// Returns the renderer's belief about what the terminal currently shows.
    #[must_use]
    pub const fn front_buffer(&self) -> &Buffer {
        &self.front
    }

    /// Resizes both buffers to the new screen size, discarding their content,
    /// and schedules a clear-and-redraw. Does nothing when the size is
    /// unchanged.
    pub fn resize(&mut self, width: u16, height: u16) {
        if self.size() == (width, height) {
            return;
        }
        self.front.resize(width, height);
        self.back.resize(width, height);
        self.needs_redraw = true;
    }

    /// Declares the terminal content unknown; the next present clears the
    /// screen and redraws the whole back buffer.
    pub const fn invalidate(&mut self) {
        self.needs_redraw = true;
    }

    /// Returns `true` when the next present will clear and redraw.
    #[must_use]
    pub const fn is_invalidated(&self) -> bool {
        self.needs_redraw
    }

    /// Produces the instruction stream that makes the terminal show the back
    /// buffer, then records the back buffer as the new front.
    ///
    /// `damage` bounds the comparison: cells outside it must be identical in
    /// both buffers (see [`Damage`]). It is ignored when a full redraw is due
    /// — after [`Renderer::new`], [`Renderer::resize`],
    /// [`Renderer::invalidate`], or a change of `caps.color` since the last
    /// present, because colour downgrade changes what every cell looks like.
    ///
    /// The caller must submit the returned stream before the next present, or
    /// call [`Renderer::invalidate`].
    #[must_use]
    pub fn present(&mut self, damage: &Damage, caps: &TerminalCapabilities) -> Vec<OutputCommand> {
        let full = self.needs_redraw || self.color != Some(caps.color);
        let stream = if full {
            redraw(&self.back, caps, &self.options)
        } else {
            diff::diff_and_sync(
                &mut self.front,
                &self.back,
                damage,
                caps,
                &self.options,
                &mut self.spans,
            )
        };
        if full {
            self.front.clone_from(&self.back);
        }
        self.needs_redraw = false;
        self.color = Some(caps.color);
        stream
    }

    /// Immediate-mode convenience: clears the back buffer, paints `view` over
    /// the whole screen, and presents with [`Damage::Full`].
    ///
    /// The describe → paint → diff pipeline in one call. Only changed cells
    /// cross the seam (the diff still compares against the front buffer), but
    /// every cell is repainted and compared; retained-mode callers that know
    /// what changed should paint into [`Renderer::back_buffer_mut`] and pass
    /// a [`Damage`] to [`Renderer::present`] instead.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Renderer, TerminalCapabilities, View};
    ///
    /// let caps = TerminalCapabilities::conservative(1);
    /// let mut renderer = Renderer::new(8, 1);
    /// let first = renderer.render(&View::text("hi"), &caps);
    /// assert!(!first.is_empty());
    /// // Same view again: nothing to send.
    /// assert!(renderer.render(&View::text("hi"), &caps).is_empty());
    /// ```
    #[must_use]
    pub fn render(&mut self, view: &View, caps: &TerminalCapabilities) -> Vec<OutputCommand> {
        self.back.clear();
        view.paint(self.back.area(), &mut self.back);
        self.present(&Damage::Full, caps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{AttrSupport, BackgroundHint, ColorTier};
    use crate::layout::Rect;
    use crate::output::{Ansi16Color, ResolvedAttributes, ResolvedColor};
    use std::io;

    // ---- shared helpers ---------------------------------------------------

    fn caps_for(tier: ColorTier) -> TerminalCapabilities {
        let mut caps = TerminalCapabilities::conservative(1);
        caps.color = ColorProfile::new(tier, AttrSupport::for_tier(tier), BackgroundHint::Unknown);
        caps
    }

    fn truecolor() -> TerminalCapabilities {
        caps_for(ColorTier::Truecolor)
    }

    fn plain() -> ResolvedAttributes {
        ResolvedAttributes::plain()
    }

    fn fg(r: u8, g: u8, b: u8) -> ResolvedAttributes {
        ResolvedAttributes::new(ResolvedColor::Rgb(r, g, b), ResolvedColor::Default)
    }

    fn bg(r: u8, g: u8, b: u8) -> ResolvedAttributes {
        ResolvedAttributes::new(ResolvedColor::Default, ResolvedColor::Rgb(r, g, b))
    }

    fn run(previous: &Buffer, next: &Buffer, damage: &Damage) -> Vec<OutputCommand> {
        diff(
            previous,
            next,
            damage,
            &truecolor(),
            &DiffOptions::default(),
        )
    }

    /// A virtual terminal: interprets an instruction stream the way the real
    /// backend does, so tests can prove "stream applied to what was on screen
    /// yields what was asked for".
    struct VirtualScreen {
        screen: Buffer,
        cursor: (u16, u16),
        attrs: ResolvedAttributes,
    }

    impl VirtualScreen {
        fn showing(buffer: &Buffer, caps: &TerminalCapabilities) -> Self {
            Self {
                screen: effective_copy(buffer, caps),
                cursor: (0, 0),
                attrs: plain(),
            }
        }

        fn apply(&mut self, stream: &[OutputCommand]) -> io::Result<()> {
            let (cols, rows) = (self.screen.width(), self.screen.height());
            for command in stream {
                command.validate_for_size(cols, rows)?;
                match command {
                    OutputCommand::MoveTo { x, y } => self.cursor = (*x, *y),
                    OutputCommand::PrintSegment { text } => {
                        let (x, y) = self.cursor;
                        let end = self.screen.set_string(x, y, text, self.attrs);
                        self.cursor = (end, y);
                    }
                    OutputCommand::SetAttributes(attrs) => self.attrs = *attrs,
                    OutputCommand::ResetAttributes => self.attrs = plain(),
                    OutputCommand::ClearRegion { rect } => self.screen.fill(*rect, plain()),
                    _ => {}
                }
            }
            Ok(())
        }
    }

    /// `buffer` with every cell's attributes replaced by what `caps` can show.
    fn effective_copy(buffer: &Buffer, caps: &TerminalCapabilities) -> Buffer {
        let mut out = buffer.clone();
        for y in 0..buffer.height() {
            for x in 0..buffer.width() {
                let cell = buffer.get(x, y).unwrap();
                out.set_attrs(Rect::new(x, y, 1, 1), diff::effective(cell.attrs(), caps));
            }
        }
        out
    }

    /// Core invariant: applying `diff(prev, next)` to a screen showing `prev`
    /// makes it show `next`, and the stream honours its contract.
    fn assert_round_trip(
        previous: &Buffer,
        next: &Buffer,
        damage: &Damage,
        caps: &TerminalCapabilities,
        options: &DiffOptions,
    ) -> Vec<OutputCommand> {
        let stream = diff(previous, next, damage, caps, options);
        let mut screen = VirtualScreen::showing(previous, caps);
        screen
            .apply(&stream)
            .unwrap_or_else(|error| panic!("invalid stream {stream:?}: {error}"));
        assert_eq!(
            screen.screen,
            effective_copy(next, caps),
            "screen != next after {stream:?}"
        );
        assert_eq!(screen.attrs, plain(), "stream must end with default attrs");
        stream
    }

    // ---- golden streams ---------------------------------------------------

    #[test]
    fn identical_buffers_emit_nothing() {
        let mut buffer = Buffer::new(20, 3);
        buffer.set_string(2, 1, "static ❤️ 日本", fg(1, 2, 3));
        assert!(run(&buffer, &buffer.clone(), &Damage::Full).is_empty());
    }

    #[test]
    fn one_changed_cell_is_one_move_and_one_print() {
        let before = Buffer::new(10, 2);
        let mut after = before.clone();
        after.set_string(4, 1, "x", plain());
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(4, 1),
                OutputCommand::print_segment("x")
            ]
        );
    }

    #[test]
    fn adjacent_cells_with_equal_attributes_coalesce_into_one_segment() {
        let before = Buffer::new(12, 1);
        let mut after = before.clone();
        after.set_string(2, 0, "hello", plain());
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(2, 0),
                OutputCommand::print_segment("hello")
            ]
        );
    }

    #[test]
    fn attribute_changes_split_segments_and_the_stream_resets_at_the_end() {
        let before = Buffer::new(8, 1);
        let mut after = before.clone();
        after.set_string(0, 0, "ab", fg(255, 0, 0));
        after.set_string(2, 0, "cd", plain());
        after.set_string(4, 0, "ef", fg(255, 0, 0));
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::SetAttributes(fg(255, 0, 0)),
                OutputCommand::print_segment("ab"),
                OutputCommand::ResetAttributes,
                OutputCommand::print_segment("cd"),
                OutputCommand::SetAttributes(fg(255, 0, 0)),
                OutputCommand::print_segment("ef"),
                OutputCommand::ResetAttributes,
            ]
        );
    }

    #[test]
    fn close_changes_bridge_through_unchanged_cells_instead_of_moving() {
        let mut before = Buffer::new(12, 1);
        before.set_string(0, 0, "a...b", plain());
        let mut after = before.clone();
        after.set_string(0, 0, "X...Y", plain());
        // Gap of three equal-attribute cells: print through, one MoveTo.
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("X...Y")
            ]
        );
        // With bridging disabled the same change costs a second move.
        let no_bridge = DiffOptions::default().with_bridge_gap(0);
        assert_eq!(
            diff(&before, &after, &Damage::Full, &truecolor(), &no_bridge),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("X"),
                OutputCommand::move_to(4, 0),
                OutputCommand::print_segment("Y"),
            ]
        );
    }

    #[test]
    fn wide_gaps_and_attribute_mismatches_are_not_bridged() {
        let mut before = Buffer::new(30, 1);
        before.set_string(0, 0, "a", plain());
        before.set_string(20, 0, "b", plain());
        let mut after = before.clone();
        after.set_string(0, 0, "X", plain());
        after.set_string(20, 0, "Y", plain());
        assert_eq!(run(&before, &after, &Damage::Full).len(), 4); // 2 moves + 2 prints

        // A gap cell with different attributes would force a SetAttributes
        // pair, so the diff moves instead.
        let mut before = Buffer::new(10, 1);
        before.set_string(1, 0, ".", fg(0, 255, 0));
        let mut after = before.clone();
        after.set_string(0, 0, "X", plain());
        after.set_string(2, 0, "Y", plain());
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("X"),
                OutputCommand::move_to(2, 0),
                OutputCommand::print_segment("Y"),
            ]
        );
    }

    #[test]
    fn changes_on_separate_rows_each_get_a_move() {
        let before = Buffer::new(6, 3);
        let mut after = before.clone();
        after.set_string(5, 0, "a", plain());
        after.set_string(0, 1, "b", plain());
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(5, 0),
                OutputCommand::print_segment("a"),
                OutputCommand::move_to(0, 1),
                OutputCommand::print_segment("b"),
            ]
        );
    }

    #[test]
    fn styled_blank_is_drawn_but_ground_state_blank_is_not() {
        let before = Buffer::new(6, 1);
        let mut after = before.clone();
        after.fill(Rect::new(1, 0, 3, 1), bg(0, 0, 90));
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(1, 0),
                OutputCommand::SetAttributes(bg(0, 0, 90)),
                OutputCommand::print_segment("   "),
                OutputCommand::ResetAttributes,
            ]
        );
        // And back: erasing the fill to ground state must paint spaces.
        assert_eq!(
            run(&after, &before, &Damage::Full),
            [
                OutputCommand::move_to(1, 0),
                OutputCommand::print_segment("   "),
            ]
        );
    }

    #[test]
    fn wide_glyphs_print_whole_and_replace_cleanly() {
        let mut before = Buffer::new(8, 1);
        before.set_string(0, 0, "ab", plain());
        let mut after = before.clone();
        after.set_string(0, 0, "日", plain());
        // The wide glyph covers the old "ab"; one print, no continuation text.
        assert_eq!(
            run(&before, &after, &Damage::Full),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("日")
            ]
        );
        // Replacing the wide glyph with two narrow cells prints both.
        assert_eq!(
            run(&after, &before, &Damage::Full),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("ab")
            ]
        );
    }

    #[test]
    fn writing_in_the_last_column_forgets_the_cursor() {
        let before = Buffer::new(4, 2);
        let mut after = before.clone();
        after.set_string(3, 0, "a", plain());
        after.set_string(0, 1, "b", plain());
        let stream = run(&before, &after, &Damage::Full);
        // Row 1 needs its own move even though row 0 ended at the margin.
        assert_eq!(
            stream,
            [
                OutputCommand::move_to(3, 0),
                OutputCommand::print_segment("a"),
                OutputCommand::move_to(0, 1),
                OutputCommand::print_segment("b"),
            ]
        );
    }

    #[test]
    fn resync_after_wide_inserts_a_move_after_each_wide_glyph() {
        let before = Buffer::new(10, 1);
        let mut after = before.clone();
        after.set_string(0, 0, "日本x", plain());
        let careful = DiffOptions::default().with_resync_after_wide(true);
        assert_eq!(
            diff(&before, &after, &Damage::Full, &truecolor(), &careful),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("日"),
                OutputCommand::move_to(2, 0),
                OutputCommand::print_segment("本"),
                OutputCommand::move_to(4, 0),
                OutputCommand::print_segment("x"),
            ]
        );
    }

    // ---- capability handling ---------------------------------------------

    #[test]
    fn effective_equality_ignores_changes_the_terminal_cannot_show() {
        let mut before = Buffer::new(4, 1);
        before.set_string(0, 0, "ab", fg(250, 10, 10));
        let mut after = before.clone();
        after.set_string(0, 0, "ab", fg(255, 0, 0));
        let ansi = caps_for(ColorTier::Ansi16);
        // Distinct RGB, same nearest ANSI colour: no output at 16 colours...
        assert!(
            diff(
                &before,
                &after,
                &Damage::Full,
                &ansi,
                &DiffOptions::default()
            )
            .is_empty()
        );
        // ...but a visible change at truecolor.
        assert!(!run(&before, &after, &Damage::Full).is_empty());
        // Mono shows no colour at all.
        let mono = caps_for(ColorTier::Mono);
        assert!(
            diff(
                &before,
                &after,
                &Damage::Full,
                &mono,
                &DiffOptions::default()
            )
            .is_empty()
        );
    }

    #[test]
    fn emitted_attributes_are_already_downgraded_to_the_tier() {
        let before = Buffer::new(4, 1);
        let mut after = before.clone();
        after.set_string(0, 0, "a", fg(255, 0, 0));
        let stream = diff(
            &before,
            &after,
            &Damage::Full,
            &caps_for(ColorTier::Ansi16),
            &DiffOptions::default(),
        );
        assert_eq!(
            stream[1],
            OutputCommand::SetAttributes(ResolvedAttributes::new(
                ResolvedColor::Ansi16(Ansi16Color::Red),
                ResolvedColor::Default
            ))
        );
    }

    #[test]
    fn unsupported_decorations_are_dropped_before_comparison_and_emission() {
        let before = Buffer::new(4, 1);
        let mut after = before.clone();
        let mut italic = plain();
        italic.italic = true;
        after.set_string(0, 0, "a", italic);
        let stream = diff(
            &before,
            &after,
            &Damage::Full,
            &caps_for(ColorTier::Ansi16), // no italic at 16 colours
            &DiffOptions::default(),
        );
        // Italic collapses to plain, so the cell prints with no attribute command.
        assert_eq!(
            stream,
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("a")
            ]
        );
    }

    // ---- damage -----------------------------------------------------------

    #[test]
    fn damage_limits_comparison_to_reported_regions() {
        let before = Buffer::new(10, 3);
        let mut after = before.clone();
        after.set_string(0, 0, "in", plain());
        after.set_string(0, 2, "out", plain());
        let only_row_zero = Damage::from_rects([Rect::new(0, 0, 10, 1)]);
        // "out" changed but was not reported: by contract it is not emitted.
        assert_eq!(
            run(&before, &after, &only_row_zero),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("in")
            ]
        );
        assert!(run(&before, &after, &Damage::none()).is_empty());
    }

    #[test]
    fn damage_regions_clip_overlap_and_may_exceed_the_buffer() {
        let before = Buffer::new(8, 2);
        let mut after = before.clone();
        after.set_string(0, 0, "abcdefgh", plain());
        let damage = Damage::from_rects([
            Rect::new(0, 0, 3, 1),
            Rect::new(2, 0, 3, 1),   // overlaps the first
            Rect::new(5, 0, 99, 99), // far past the buffer
            Rect::new(0, 0, 0, 4),   // empty: ignored
        ]);
        assert_eq!(
            run(&before, &after, &damage),
            [
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("abcdefgh")
            ]
        );
    }

    #[test]
    fn damage_cutting_a_wide_glyph_widens_to_the_whole_glyph() {
        let mut before = Buffer::new(8, 1);
        before.set_string(0, 0, "日本", plain());
        let mut after = before.clone();
        after.set_string(0, 0, "日語", plain());
        // The region touches only the continuation column of the changed glyph.
        let damage = Damage::from_rects([Rect::new(3, 0, 1, 1)]);
        assert_eq!(
            run(&before, &after, &damage),
            [
                OutputCommand::move_to(2, 0),
                OutputCommand::print_segment("語")
            ]
        );
    }

    #[test]
    fn small_change_in_a_large_screen_emits_a_small_stream() {
        let mut before = Buffer::new(200, 60);
        for y in 0..60 {
            before.set_string(0, y, &"x".repeat(200), plain());
        }
        let mut after = before.clone();
        after.set_string(100, 30, "Y", plain());
        let damage = Damage::from_rects([Rect::new(100, 30, 1, 1)]);
        let stream = run(&before, &after, &damage);
        assert_eq!(stream.len(), 2);
        assert_eq!(stream, run(&before, &after, &Damage::Full));
    }

    #[test]
    fn full_screen_churn_is_bounded_by_cells_and_rows() {
        let (cols, rows) = (120u16, 40u16);
        let mut before = Buffer::new(cols, rows);
        let mut after = Buffer::new(cols, rows);
        for y in 0..rows {
            before.set_string(0, y, &"a".repeat(usize::from(cols)), plain());
            after.set_string(0, y, &"b".repeat(usize::from(cols)), plain());
        }
        let stream = run(&before, &after, &Damage::Full);
        // One move and one segment per row — no per-cell commands.
        assert_eq!(stream.len(), usize::from(rows) * 2);
        let printed: usize = stream
            .iter()
            .map(|command| match command {
                OutputCommand::PrintSegment { text } => text.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(printed, usize::from(cols) * usize::from(rows));
    }

    // ---- redraw and size changes -----------------------------------------

    #[test]
    fn redraw_clears_then_paints_only_non_ground_cells() {
        let mut screen = Buffer::new(6, 2);
        screen.set_string(1, 1, "ok", plain());
        let stream = redraw(&screen, &truecolor(), &DiffOptions::default());
        assert_eq!(
            stream,
            [
                OutputCommand::ResetAttributes,
                OutputCommand::clear_region(Rect::new(0, 0, 6, 2)),
                OutputCommand::move_to(1, 1),
                OutputCommand::print_segment("ok"),
            ]
        );
        assert!(redraw(&Buffer::new(0, 0), &truecolor(), &DiffOptions::default()).is_empty());
    }

    #[test]
    fn size_mismatch_falls_back_to_a_full_redraw() {
        let mut next = Buffer::new(6, 2);
        next.set_string(0, 0, "hi", plain());
        let stream = run(&Buffer::new(3, 3), &next, &Damage::none());
        assert_eq!(stream[0], OutputCommand::ResetAttributes);
        assert!(matches!(stream[1], OutputCommand::ClearRegion { .. }));
        assert!(stream.contains(&OutputCommand::print_segment("hi")));
    }

    // ---- round-trip property ---------------------------------------------

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }
    }

    fn random_text(rng: &mut Rng) -> String {
        const ATOMS: [&str; 14] = [
            "a",
            "b",
            "z",
            " ",
            ".",
            "日",
            "本",
            "한",
            "é",
            "e\u{301}",
            "👩‍💻",
            "🇺🇿",
            "❤\u{fe0f}",
            "~",
        ];
        (0..rng.below(7))
            .map(|_| ATOMS[rng.below(ATOMS.len() as u64) as usize])
            .collect()
    }

    fn random_attrs(rng: &mut Rng) -> ResolvedAttributes {
        match rng.below(6) {
            0 => plain(),
            1 => fg(255, 0, 0),
            2 => fg(250, 10, 10),
            3 => bg(0, 0, 200),
            4 => plain().with_bold(),
            _ => ResolvedAttributes::new(
                ResolvedColor::Palette(rng.below(256) as u8),
                ResolvedColor::Default,
            ),
        }
    }

    fn random_rect(rng: &mut Rng, cols: u16, rows: u16) -> Rect {
        Rect::new(
            rng.below(u64::from(cols) + 2) as u16,
            rng.below(u64::from(rows) + 1) as u16,
            rng.below(u64::from(cols) + 2) as u16,
            rng.below(u64::from(rows) + 1) as u16,
        )
    }

    /// Mutates `buffer` strictly inside `rect` (the damage promise).
    fn scribble(rng: &mut Rng, buffer: &mut Buffer, rect: Rect) {
        for _ in 0..rng.below(4) + 1 {
            let attrs = random_attrs(rng);
            match rng.below(5) {
                0 => buffer.fill(rect, attrs),
                1 => buffer.set_attrs(rect, attrs),
                _ => {
                    let text = random_text(rng);
                    let x = rect
                        .x
                        .saturating_add(rng.below(u64::from(rect.width) + 1) as u16);
                    let y = rect
                        .y
                        .saturating_add(rng.below(u64::from(rect.height) + 1) as u16);
                    buffer.set_string_clipped(x, y, &text, attrs, rect);
                }
            }
        }
    }

    #[test]
    fn diff_applied_to_the_previous_screen_always_yields_the_next() {
        let tiers = [
            ColorTier::Truecolor,
            ColorTier::Palette256,
            ColorTier::Ansi16,
            ColorTier::Mono,
        ];
        let option_sets = [
            DiffOptions::default(),
            DiffOptions::default().with_bridge_gap(0),
            DiffOptions::default().with_bridge_gap(12),
            DiffOptions::default().with_resync_after_wide(true),
        ];
        let mut rng = Rng(0xdead_beef_cafe_f00d);
        for round in 0..1500 {
            let (cols, rows) = (1 + rng.below(23) as u16, 1 + rng.below(6) as u16);
            let mut previous = Buffer::new(cols, rows);
            let full = Rect::new(0, 0, cols, rows);
            scribble(&mut rng, &mut previous, full);
            scribble(&mut rng, &mut previous, full);

            let mut next = previous.clone();
            let damage = if rng.below(4) == 0 {
                scribble(&mut rng, &mut next, full);
                Damage::Full
            } else {
                let mut damage = Damage::none();
                for _ in 0..=rng.below(3) {
                    let rect = random_rect(&mut rng, cols, rows);
                    scribble(&mut rng, &mut next, rect);
                    damage.add(rect);
                }
                damage
            };
            let caps = caps_for(tiers[rng.below(4) as usize]);
            let options = option_sets[rng.below(4) as usize];
            let stream = assert_round_trip(&previous, &next, &damage, &caps, &options);
            // An unchanged screen must be silent.
            if previous == next {
                assert!(stream.is_empty(), "round {round}: {stream:?}");
            }
        }
    }

    #[test]
    fn redraw_onto_garbage_yields_the_buffer_exactly() {
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for _ in 0..300 {
            let (cols, rows) = (1 + rng.below(20) as u16, 1 + rng.below(5) as u16);
            let mut garbage = Buffer::new(cols, rows);
            let mut wanted = Buffer::new(cols, rows);
            let full = Rect::new(0, 0, cols, rows);
            scribble(&mut rng, &mut garbage, full);
            scribble(&mut rng, &mut wanted, full);
            let caps = caps_for(ColorTier::Truecolor);
            let stream = redraw(&wanted, &caps, &DiffOptions::default());
            let mut screen = VirtualScreen::showing(&garbage, &caps);
            screen.apply(&stream).unwrap();
            assert_eq!(screen.screen, effective_copy(&wanted, &caps));
        }
    }

    // ---- Renderer ---------------------------------------------------------

    #[test]
    fn renderer_first_present_redraws_then_goes_quiet() {
        let caps = truecolor();
        let mut renderer = Renderer::new(10, 2);
        assert!(renderer.is_invalidated());
        renderer
            .back_buffer_mut()
            .set_string(0, 0, "hello", plain());
        let first = renderer.present(&Damage::Full, &caps);
        assert_eq!(first[0], OutputCommand::ResetAttributes);
        assert!(matches!(first[1], OutputCommand::ClearRegion { .. }));
        assert!(!renderer.is_invalidated());
        assert_eq!(renderer.front_buffer(), renderer.back_buffer());
        assert!(renderer.present(&Damage::Full, &caps).is_empty());
    }

    #[test]
    fn renderer_tracks_the_screen_across_partial_frames() {
        let caps = truecolor();
        let mut renderer = Renderer::new(16, 4);
        let mut screen = VirtualScreen::showing(&Buffer::new(16, 4), &caps);
        let mut rng = Rng(0x0f0f_1234_5678_9999);
        screen
            .apply(&renderer.present(&Damage::Full, &caps))
            .unwrap();
        for _ in 0..400 {
            let mut damage = Damage::none();
            for _ in 0..=rng.below(2) {
                let rect = random_rect(&mut rng, 16, 4);
                scribble(&mut rng, renderer.back_buffer_mut(), rect);
                damage.add(rect);
            }
            let stream = renderer.present(&damage, &caps);
            screen.apply(&stream).unwrap();
            assert_eq!(screen.screen, effective_copy(renderer.back_buffer(), &caps));
            assert_eq!(renderer.front_buffer(), renderer.back_buffer());
        }
    }

    #[test]
    fn renderer_records_raw_changes_even_when_the_terminal_cannot_show_them() {
        for damage in [Damage::Full, Damage::from_rects([Rect::new(1, 0, 1, 1)])] {
            let mono = caps_for(ColorTier::Mono);
            let mut renderer = Renderer::new(8, 1);
            renderer
                .back_buffer_mut()
                .set_string(0, 0, "日本", fg(255, 0, 0));
            let _ = renderer.present(&Damage::Full, &mono);
            // Only the continuation is damaged. Both halves must be synced,
            // even though monochrome downgrades both reds to the same ink.
            renderer
                .back_buffer_mut()
                .set_attrs(Rect::new(1, 0, 1, 1), fg(200, 10, 10));
            assert!(renderer.present(&damage, &mono).is_empty());
            assert_eq!(renderer.front_buffer(), renderer.back_buffer());
            let stream = renderer.present(&Damage::none(), &truecolor());
            let mut screen = VirtualScreen::showing(&Buffer::new(8, 1), &truecolor());
            screen.apply(&stream).unwrap();
            assert_eq!(
                screen.screen,
                effective_copy(renderer.back_buffer(), &truecolor())
            );
        }
    }

    #[test]
    fn renderer_stream_matches_standalone_diff_for_disjoint_wide_and_heap_edits() {
        let caps = truecolor();
        let mut renderer = Renderer::new(30, 2);
        renderer
            .back_buffer_mut()
            .set_string(0, 0, "日 abc 語", plain());
        renderer
            .back_buffer_mut()
            .set_string(1, 1, "👨‍👩‍👧‍👦 z", fg(1, 2, 3));
        let _ = renderer.present(&Damage::Full, &caps);
        let before = renderer.front_buffer().clone();
        renderer.back_buffer_mut().set_string(1, 0, "x", plain());
        renderer.back_buffer_mut().set_string(8, 0, "本", plain());
        renderer
            .back_buffer_mut()
            .set_string(1, 1, "👩‍👩‍👦‍👦", fg(3, 2, 1));
        let damage = Damage::from_rects([
            Rect::new(1, 0, 1, 1),
            Rect::new(8, 0, 2, 1),
            Rect::new(2, 1, 1, 1),
        ]);
        let expected = diff(
            &before,
            renderer.back_buffer(),
            &damage,
            &caps,
            &DiffOptions::default(),
        );
        let actual = renderer.present(&damage, &caps);
        assert_eq!(actual, expected);
        assert_eq!(renderer.front_buffer(), renderer.back_buffer());
        let mut screen = VirtualScreen::showing(&before, &caps);
        screen.apply(&actual).unwrap();
        assert_eq!(screen.screen, effective_copy(renderer.back_buffer(), &caps));
        assert!(renderer.present(&damage, &caps).is_empty());
    }

    #[test]
    fn renderer_resize_invalidate_and_colour_change_force_a_redraw() {
        let caps = truecolor();
        let mut renderer = Renderer::new(8, 2);
        renderer.back_buffer_mut().set_string(0, 0, "abc", plain());
        let _ = renderer.present(&Damage::Full, &caps);
        assert!(renderer.present(&Damage::Full, &caps).is_empty());

        renderer.invalidate();
        assert!(renderer.is_invalidated());
        let after_invalidate = renderer.present(&Damage::none(), &caps);
        assert!(matches!(
            after_invalidate[1],
            OutputCommand::ClearRegion { .. }
        ));
        assert!(after_invalidate.contains(&OutputCommand::print_segment("abc")));

        // Same size is a no-op; a new size clears both buffers.
        renderer.resize(8, 2);
        assert!(!renderer.is_invalidated());
        renderer.resize(5, 1);
        assert_eq!(renderer.size(), (5, 1));
        assert!(renderer.is_invalidated());
        assert_eq!(renderer.back_buffer().text_rows(), ["     "]);

        // A colour-capability change redraws even with `Damage::none()`.
        let _ = renderer.present(&Damage::Full, &caps);
        assert!(renderer.present(&Damage::none(), &caps).is_empty());
        let ansi = caps_for(ColorTier::Ansi16);
        let redrawn = renderer.present(&Damage::none(), &ansi);
        assert!(matches!(redrawn[1], OutputCommand::ClearRegion { .. }));
    }

    #[test]
    fn renderer_with_a_degenerate_size_is_inert() {
        let mut renderer = Renderer::new(0, 0);
        assert!(renderer.present(&Damage::Full, &truecolor()).is_empty());
        renderer.back_buffer_mut().set_string(0, 0, "x", plain());
        assert!(renderer.present(&Damage::Full, &truecolor()).is_empty());
    }
}
