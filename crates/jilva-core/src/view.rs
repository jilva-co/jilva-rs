//! The owned frame description and its paint pass
//! (`docs/RFC/0006-view-description.md`).
//!
//! A [`View`] is what `describe` returns in the render pipeline of
//! `docs/research/RENDERING_MODEL_RESOLUTION.md` §5: an *owned* value tree
//! with no lifetimes, no coordinates, and no terminal. The framework — never
//! user code — runs layout over it and paints it into a [`Buffer`]:
//!
//! ```text
//! View (owned) ──► View::paint(area, &mut Buffer) ──► Renderer::present ──► OutputCommand stream
//!                   │ layout: Layout::split per container
//!                   └ paint: clipped, infallible, deterministic
//! ```
//!
//! Painting is infallible by construction: every write clips to its region,
//! so an over-constrained or tiny area degrades (empty regions, truncated
//! text) instead of failing. There is no partially painted state to recover
//! from, because failure cannot occur after description.
//!
//! This is the *static* half of the component model. Identity, state slots,
//! events and the `Component` trait build on top of it and are not here.

use crate::layout::{Align, Constraint, Layout, Rect};
use crate::output::ResolvedAttributes;
use crate::renderer::{Buffer, text_width};

/// A run of text drawn with one set of resolved attributes.
///
/// # Example
///
/// ```rust
/// use jilva_core::{ResolvedAttributes, Span};
///
/// let span = Span::new("hello", ResolvedAttributes::plain().with_bold());
/// assert_eq!(span.text(), "hello");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    text: String,
    attrs: ResolvedAttributes,
}

impl Span {
    /// Creates a span. Control characters in `text` are skipped when painted.
    #[must_use]
    pub fn new(text: impl Into<String>, attrs: ResolvedAttributes) -> Self {
        Self {
            text: text.into(),
            attrs,
        }
    }

    /// Creates a span with plain attributes.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, ResolvedAttributes::plain())
    }

    /// Returns the span text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the span attributes.
    #[must_use]
    pub const fn attrs(&self) -> &ResolvedAttributes {
        &self.attrs
    }
}

/// The glyph set of a [`View::bordered`] frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BorderStyle {
    /// `┌─┐│└┘`
    #[default]
    Plain,
    /// `╭─╮│╰╯`
    Rounded,
    /// `╔═╗║╚╝`
    Double,
    /// `┏━┓┃┗┛`
    Heavy,
}

impl BorderStyle {
    /// `[top-left, top-right, bottom-left, bottom-right, horizontal, vertical]`.
    const fn glyphs(self) -> [&'static str; 6] {
        match self {
            Self::Plain => ["┌", "┐", "└", "┘", "─", "│"],
            Self::Rounded => ["╭", "╮", "╰", "╯", "─", "│"],
            Self::Double => ["╔", "╗", "╚", "╝", "═", "║"],
            Self::Heavy => ["┏", "┓", "┗", "┛", "━", "┃"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Empty,
    Fill(ResolvedAttributes),
    Text {
        lines: Vec<Vec<Span>>,
        align: Align,
    },
    Stack {
        layout: Layout,
        children: Vec<View>,
    },
    Overlay(Vec<View>),
    Padding {
        margin: u16,
        child: Box<View>,
    },
    Border {
        style: BorderStyle,
        attrs: ResolvedAttributes,
        title: Option<String>,
        child: Box<View>,
    },
}

/// An owned, geometry-free description of what to draw.
///
/// Build views with the constructors and combinators; paint them with
/// [`View::paint`]. A `View` is plain data: it can be cloned, compared, and
/// asserted on in tests without a terminal.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Buffer, Constraint, Rect, View};
///
/// let screen = View::column([
///     (Constraint::Length(1), View::text("title")),
///     (Constraint::Fill(1), View::text("body").padding(1)),
/// ]);
/// let mut buffer = Buffer::new(8, 4);
/// screen.paint(Rect::new(0, 0, 8, 4), &mut buffer);
/// assert_eq!(buffer.text_rows(), ["title   ", "        ", " body   ", "        "]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View(Node);

impl Default for View {
    /// Returns [`View::empty`].
    fn default() -> Self {
        Self::empty()
    }
}

impl View {
    /// A view that draws nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self(Node::Empty)
    }

    /// Fills the whole region with styled blanks (a background).
    #[must_use]
    pub const fn fill(attrs: ResolvedAttributes) -> Self {
        Self(Node::Fill(attrs))
    }

    /// Plain text. `'\n'` starts a new line; other controls are skipped when
    /// painted. Text is clipped to its region, never wrapped.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self::styled_text(text, ResolvedAttributes::plain())
    }

    /// Text drawn with one set of attributes (see [`View::text`]).
    #[must_use]
    pub fn styled_text(text: &str, attrs: ResolvedAttributes) -> Self {
        Self(Node::Text {
            lines: text
                .split('\n')
                .map(|line| vec![Span::new(line, attrs)])
                .collect(),
            align: Align::Start,
        })
    }

    /// One line made of differently styled spans.
    #[must_use]
    pub fn line(spans: impl IntoIterator<Item = Span>) -> Self {
        Self(Node::Text {
            lines: vec![spans.into_iter().collect()],
            align: Align::Start,
        })
    }

    /// Sets how each line of a text view sits inside its region. No effect on
    /// other views.
    #[must_use]
    pub fn text_align(mut self, align: Align) -> Self {
        if let Node::Text { align: current, .. } = &mut self.0 {
            *current = align;
        }
        self
    }

    /// Stacks children left to right, each sized by its constraint.
    #[must_use]
    pub fn row(items: impl IntoIterator<Item = (Constraint, Self)>) -> Self {
        Self::stack(Layout::row(), items)
    }

    /// Stacks children top to bottom, each sized by its constraint.
    #[must_use]
    pub fn column(items: impl IntoIterator<Item = (Constraint, Self)>) -> Self {
        Self::stack(Layout::column(), items)
    }

    fn stack(layout: Layout, items: impl IntoIterator<Item = (Constraint, Self)>) -> Self {
        let (constraints, children): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        Self(Node::Stack {
            layout: layout.constraints(constraints),
            children,
        })
    }

    /// Sets the gap between the children of a row or column. No effect on
    /// other views.
    #[must_use]
    pub fn gap(mut self, gap: u16) -> Self {
        if let Node::Stack { layout, .. } = &mut self.0 {
            *layout = std::mem::take(layout).gap(gap);
        }
        self
    }

    /// Sets where free space goes in a row or column (see [`Layout`]). No
    /// effect on other views.
    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        if let Node::Stack { layout, .. } = &mut self.0 {
            *layout = std::mem::take(layout).align(align);
        }
        self
    }

    /// Draws views on top of each other in the same region, first to last
    /// (later views cover earlier ones). The usual background-plus-content
    /// pattern.
    #[must_use]
    pub fn overlay(layers: impl IntoIterator<Item = Self>) -> Self {
        Self(Node::Overlay(layers.into_iter().collect()))
    }

    /// Insets this view by `margin` cells on every side.
    #[must_use]
    pub fn padding(self, margin: u16) -> Self {
        Self(Node::Padding {
            margin,
            child: Box::new(self),
        })
    }

    /// Frames this view with a one-cell border; the view sits inside it.
    /// A region smaller than 2×2 draws no border.
    #[must_use]
    pub fn bordered(self, style: BorderStyle, attrs: ResolvedAttributes) -> Self {
        Self(Node::Border {
            style,
            attrs,
            title: None,
            child: Box::new(self),
        })
    }

    /// Sets the title of a bordered view, drawn in the top edge. No effect on
    /// other views.
    #[must_use]
    pub fn title(mut self, title: &str) -> Self {
        if let Node::Border { title: slot, .. } = &mut self.0 {
            *slot = Some(title.to_owned());
        }
        self
    }

    /// Paints this view into `area` of `buffer`.
    ///
    /// Infallible and deterministic: everything clips to `area` and to the
    /// buffer, an empty area draws nothing, and painting twice gives the same
    /// cells. Only cells inside `area` change, apart from the one-cell
    /// repair of a wide glyph cut by the area's edge (RFC-0004 §3).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        match &self.0 {
            Node::Empty => {}
            Node::Fill(attrs) => buffer.fill(area, *attrs),
            Node::Text { lines, align } => paint_text(lines, *align, area, buffer),
            Node::Stack { layout, children } => {
                for (child, region) in children.iter().zip(layout.split(area)) {
                    child.paint(region, buffer);
                }
            }
            Node::Overlay(layers) => {
                for layer in layers {
                    layer.paint(area, buffer);
                }
            }
            Node::Padding { margin, child } => child.paint(area.inner(*margin), buffer),
            Node::Border {
                style,
                attrs,
                title,
                child,
            } => {
                paint_border(*style, *attrs, title.as_deref(), area, buffer);
                child.paint(area.inner(1), buffer);
            }
        }
    }
}

fn paint_text(lines: &[Vec<Span>], align: Align, area: Rect, buffer: &mut Buffer) {
    for (index, line) in lines.iter().enumerate() {
        let row = u32::from(area.y) + index as u32;
        if row >= area.bottom() || row > u32::from(u16::MAX) {
            break;
        }
        let used: usize = line.iter().map(|span| text_width(span.text())).sum();
        let free = usize::from(area.width).saturating_sub(used);
        let offset = match align {
            Align::Start => 0,
            Align::Center => free / 2,
            Align::End => free,
        };
        // `offset <= area.width`, and `x` never leaves `area` thanks to the clip.
        let mut x = area.x.saturating_add(offset as u16);
        for span in line {
            x = buffer.set_string_clipped(x, row as u16, span.text(), *span.attrs(), area);
        }
    }
}

fn paint_border(
    style: BorderStyle,
    attrs: ResolvedAttributes,
    title: Option<&str>,
    area: Rect,
    buffer: &mut Buffer,
) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        horizontal,
        vertical,
    ] = style.glyphs();
    let inner = usize::from(area.width) - 2;
    let top = format!("{top_left}{}{top_right}", horizontal.repeat(inner));
    let bottom = format!("{bottom_left}{}{bottom_right}", horizontal.repeat(inner));
    let (Ok(last_row), Ok(last_col)) = (
        u16::try_from(area.bottom() - 1),
        u16::try_from(area.right() - 1),
    ) else {
        return;
    };
    buffer.set_string_clipped(area.x, area.y, &top, attrs, area);
    buffer.set_string_clipped(area.x, last_row, &bottom, attrs, area);
    for y in area.y.saturating_add(1)..last_row {
        buffer.set_string_clipped(area.x, y, vertical, attrs, area);
        buffer.set_string_clipped(last_col, y, vertical, attrs, area);
    }
    if let Some(title) = title {
        // " title " inset by two cells, never over the corners.
        let room = Rect::new(
            area.x.saturating_add(2),
            area.y,
            area.width.saturating_sub(4),
            1,
        );
        if !room.is_empty() {
            buffer.set_string_clipped(room.x, room.y, &format!(" {title} "), attrs, room);
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

    fn render(view: &View, cols: u16, rows: u16) -> Vec<String> {
        let mut buffer = Buffer::new(cols, rows);
        view.paint(Rect::new(0, 0, cols, rows), &mut buffer);
        buffer.text_rows()
    }

    #[test]
    fn text_splits_lines_clips_and_skips_controls() {
        assert_eq!(
            render(&View::text("ab\ncd"), 4, 3),
            ["ab  ", "cd  ", "    "]
        );
        assert_eq!(render(&View::text("abcdef"), 4, 1), ["abcd"]);
        assert_eq!(render(&View::text("a\ncd\ne"), 3, 2), ["a  ", "cd "]);
        assert_eq!(render(&View::text("a\u{1b}[2Jb"), 8, 1), ["a[2Jb   "]);
    }

    #[test]
    fn text_alignment_uses_cell_width() {
        let row = |align| render(&View::text("日本").text_align(align), 8, 1);
        assert_eq!(row(Align::Start), ["日本    "]);
        assert_eq!(row(Align::Center), ["  日本  "]);
        assert_eq!(row(Align::End), ["    日本"]);
        // Too wide to align: starts at the edge and clips on a cluster boundary.
        assert_eq!(
            render(&View::text("日本語").text_align(Align::End), 5, 1),
            ["日本 "]
        );
    }

    #[test]
    fn spans_keep_their_own_attributes() {
        let view = View::line([Span::new("ab", red()), Span::plain("cd")]);
        let mut buffer = Buffer::new(6, 1);
        view.paint(Rect::new(0, 0, 6, 1), &mut buffer);
        assert_eq!(buffer.get(1, 0).unwrap().attrs(), &red());
        assert_eq!(buffer.get(2, 0).unwrap().attrs(), &plain());
        assert_eq!(buffer.text_rows(), ["abcd  "]);
    }

    #[test]
    fn column_and_row_place_children_with_layout_rules() {
        let view = View::column([
            (Constraint::Length(1), View::text("head")),
            (
                Constraint::Fill(1),
                View::row([
                    (Constraint::Length(3), View::text("nav")),
                    (Constraint::Fill(1), View::text("main")),
                ])
                .gap(1),
            ),
            (Constraint::Length(1), View::text("foot")),
        ]);
        assert_eq!(
            render(&view, 10, 4),
            ["head      ", "nav main  ", "          ", "foot      "].map(String::from)
        );
    }

    #[test]
    fn gap_and_align_apply_only_to_stacks() {
        let text = View::text("x")
            .gap(5)
            .align(Align::End)
            .title("t")
            .text_align(Align::End);
        assert_eq!(render(&text, 3, 1), ["  x"]);
        let stack = View::row([
            (Constraint::Length(1), View::text("a")),
            (Constraint::Length(1), View::text("b")),
        ])
        .gap(2)
        .align(Align::End);
        assert_eq!(render(&stack, 8, 1), ["    a  b"]);
    }

    #[test]
    fn overlay_paints_later_layers_over_earlier_ones() {
        let background = ResolvedAttributes::new(ResolvedColor::Default, ResolvedColor::Palette(4));
        let view = View::overlay([View::fill(background), View::text("hi")]);
        let mut buffer = Buffer::new(4, 1);
        view.paint(Rect::new(0, 0, 4, 1), &mut buffer);
        assert_eq!(buffer.text_rows(), ["hi  "]);
        // A later layer replaces the cells it writes, attributes included;
        // cells it does not write keep the earlier layer.
        assert_eq!(buffer.get(0, 0).unwrap().attrs(), &plain());
        assert_eq!(buffer.get(3, 0).unwrap().attrs(), &background);
    }

    #[test]
    fn padding_insets_and_collapses_gracefully() {
        assert_eq!(
            render(&View::text("ab").padding(1), 4, 3),
            ["    ", " ab ", "    "]
        );
        // Too much padding: nothing drawn, nothing panics.
        assert_eq!(
            render(&View::text("ab").padding(9), 4, 3),
            ["    ", "    ", "    "]
        );
    }

    #[test]
    fn borders_frame_the_child_and_carry_a_title() {
        let view = View::text("hi")
            .bordered(BorderStyle::Rounded, plain())
            .title("T");
        assert_eq!(
            render(&view, 8, 4),
            ["╭─ T ──╮", "│hi    │", "│      │", "╰──────╯"].map(String::from)
        );
        let double = View::empty().bordered(BorderStyle::Double, plain());
        assert_eq!(render(&double, 4, 3), ["╔══╗", "║  ║", "╚══╝"]);
        // Too small for a border: skipped, child gets nothing.
        assert_eq!(
            render(&View::text("x").bordered(BorderStyle::Plain, plain()), 1, 3),
            [" ", " ", " "]
        );
        assert_eq!(
            render(&View::text("x").bordered(BorderStyle::Plain, plain()), 2, 2),
            ["┌┐", "└┘"]
        );
    }

    #[test]
    fn painting_stays_inside_the_area() {
        let mut buffer = Buffer::new(10, 5);
        for y in 0..5 {
            buffer.set_string(0, y, "##########", plain());
        }
        let area = Rect::new(2, 1, 5, 3);
        View::overlay([
            View::fill(plain()),
            View::text("abcdefghij\nsecond\nthird\nfourth").bordered(BorderStyle::Plain, plain()),
        ])
        .paint(area, &mut buffer);
        for y in 0..5u16 {
            for x in 0..10u16 {
                let inside = (2..7).contains(&x) && (1..4).contains(&y);
                if !inside {
                    assert_eq!(
                        buffer.get(x, y).unwrap().symbol(),
                        "#",
                        "({x},{y}) was touched"
                    );
                }
            }
        }
    }

    #[test]
    fn empty_and_default_views_draw_nothing() {
        assert_eq!(View::default(), View::empty());
        assert_eq!(render(&View::empty(), 3, 1), ["   "]);
        let mut buffer = Buffer::new(3, 1);
        View::text("x").paint(Rect::new(0, 0, 0, 5), &mut buffer);
        assert_eq!(buffer.text_rows(), ["   "]);
    }

    #[test]
    fn painting_twice_is_identical() {
        let view = View::column([
            (
                Constraint::Length(2),
                View::text("日本語\nx").bordered(BorderStyle::Heavy, red()),
            ),
            (Constraint::Fill(1), View::fill(red())),
        ]);
        let mut once = Buffer::new(7, 6);
        view.paint(Rect::new(0, 0, 7, 6), &mut once);
        let mut twice = once.clone();
        view.paint(Rect::new(0, 0, 7, 6), &mut twice);
        assert_eq!(once, twice);
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

    fn random_view(rng: &mut Rng, depth: u32) -> View {
        const TEXTS: [&str; 6] = [
            "hello",
            "日本語",
            "a\nb\ncd",
            "👩‍💻 x",
            "e\u{301}e\u{301}",
            "",
        ];
        match if depth == 0 {
            rng.below(3)
        } else {
            rng.below(8)
        } {
            0 => View::text(TEXTS[rng.below(6) as usize]),
            1 => View::fill(red()),
            2 => View::empty(),
            3 | 4 => {
                let items = (0..rng.below(4)).map(|_| {
                    let n = rng.below(12) as u16;
                    let constraint = match rng.below(5) {
                        0 => Constraint::Length(n),
                        1 => Constraint::Percentage(n * 8),
                        2 => Constraint::Min(n),
                        3 => Constraint::Max(n),
                        _ => Constraint::Fill(rng.below(3) as u16),
                    };
                    (constraint, random_view(rng, depth - 1))
                });
                let stack: Vec<_> = items.collect();
                if rng.below(2) == 0 {
                    View::row(stack)
                } else {
                    View::column(stack)
                }
                .gap(rng.below(3) as u16)
            }
            5 => random_view(rng, depth - 1).padding(rng.below(3) as u16),
            6 => random_view(rng, depth - 1)
                .bordered(BorderStyle::Rounded, red())
                .title(TEXTS[rng.below(6) as usize]),
            _ => View::overlay([random_view(rng, depth - 1), random_view(rng, depth - 1)]),
        }
    }

    #[test]
    fn random_trees_in_random_areas_never_panic_and_never_tear_glyphs() {
        let mut rng = Rng(0x5eed_1234_abcd_ef01);
        for _ in 0..3000 {
            let view = random_view(&mut rng, 3);
            let (cols, rows) = (rng.below(24) as u16, rng.below(8) as u16);
            let mut buffer = Buffer::new(cols, rows);
            let area = Rect::new(
                rng.below(6) as u16,
                rng.below(3) as u16,
                rng.below(30) as u16,
                rng.below(10) as u16,
            );
            view.paint(area, &mut buffer);
            for y in 0..rows {
                let row = buffer.row(y).unwrap();
                for (x, cell) in row.iter().enumerate() {
                    match cell.width() {
                        0 => assert!(x > 0 && row[x - 1].width() == 2),
                        2 => assert!(row.get(x + 1).is_some_and(|c| c.is_continuation())),
                        _ => {}
                    }
                }
            }
        }
    }
}
