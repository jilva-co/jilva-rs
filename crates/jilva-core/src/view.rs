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
//! This is the *static* half of the component model. A view may contain
//! [`View::component`] nodes; the [`App`](crate::App) runtime expands them
//! into their own views (with their state slots) before painting. Painting a
//! view directly draws unexpanded components as nothing.

use std::collections::HashSet;

use crate::component::{Component, ComponentNode, Key, Path, Segment};
use crate::error::Result;
use crate::layout::{Align, Constraint, Direction, Layout, Rect};
use crate::motion::Lerp;
use crate::output::{ResolvedAttributes, ResolvedColor};
use crate::renderer::{Buffer, graphemes, text_width};

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

/// Assumed terminal default colours, used when an effect must blend with a
/// cell whose colour is "whatever the terminal default is" (unknowable from
/// inside the process). Pass the real theme's colours when you have them.
///
/// # Example
///
/// ```rust
/// use jilva_core::Backdrop;
///
/// let dark = Backdrop::dark();
/// assert!(dark.bg.0 < dark.fg.0);
/// let custom = Backdrop::new((255, 255, 255), (0, 0, 40));
/// assert_eq!(custom.bg, (0, 0, 40));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Backdrop {
    /// Assumed foreground (text) colour of the terminal default.
    pub fg: (u8, u8, u8),
    /// Assumed background colour of the terminal default.
    pub bg: (u8, u8, u8),
}

impl Backdrop {
    /// Creates a backdrop from explicit foreground and background colours.
    #[must_use]
    pub const fn new(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> Self {
        Self { fg, bg }
    }

    /// A typical dark terminal: light grey on near-black.
    #[must_use]
    pub const fn dark() -> Self {
        Self::new((204, 204, 204), (24, 24, 24))
    }

    /// A typical light terminal: near-black on off-white.
    #[must_use]
    pub const fn light() -> Self {
        Self::new((36, 36, 36), (248, 248, 248))
    }
}

impl Default for Backdrop {
    /// Returns [`Backdrop::dark`].
    fn default() -> Self {
        Self::dark()
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
    Gradient {
        from: (u8, u8, u8),
        to: (u8, u8, u8),
        direction: Direction,
    },
    Tint {
        color: (u8, u8, u8),
        alpha: u8,
        backdrop: Backdrop,
    },
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
    Component(ComponentNode),
    Keyed {
        key: Key,
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

    /// A background that blends smoothly from `from` to `to` across the region:
    /// left to right for [`Direction::Row`], top to bottom for
    /// [`Direction::Column`]. Truecolor values; the renderer downgrades them
    /// to 256 or 16 colours automatically on weaker terminals.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Buffer, Direction, Rect, ResolvedColor, View};
    ///
    /// let mut buffer = Buffer::new(3, 1);
    /// View::gradient((0, 0, 0), (200, 100, 50), Direction::Row)
    ///     .paint(Rect::new(0, 0, 3, 1), &mut buffer);
    /// assert_eq!(buffer.get(0, 0).unwrap().attrs().bg, ResolvedColor::Rgb(0, 0, 0));
    /// assert_eq!(buffer.get(1, 0).unwrap().attrs().bg, ResolvedColor::Rgb(100, 50, 25));
    /// assert_eq!(buffer.get(2, 0).unwrap().attrs().bg, ResolvedColor::Rgb(200, 100, 50));
    /// ```
    #[must_use]
    pub const fn gradient(from: (u8, u8, u8), to: (u8, u8, u8), direction: Direction) -> Self {
        Self(Node::Gradient {
            from,
            to,
            direction,
        })
    }

    /// Blends `color` over everything already painted in the region, with
    /// 8-bit `alpha` (`0` changes nothing, `255` replaces colours). Affects
    /// foreground and background colours; symbols and decorations stay.
    ///
    /// Use inside [`View::overlay`], after the content it covers: a dim
    /// scrim behind a dialog is `tint((0, 0, 0), 140, Backdrop::dark())`.
    /// Cells whose colour is the terminal default blend against `backdrop`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Backdrop, Buffer, Rect, ResolvedAttributes, ResolvedColor, View};
    ///
    /// let white = ResolvedAttributes::new(ResolvedColor::Rgb(255, 255, 255), ResolvedColor::Default);
    /// let scene = View::overlay([
    ///     View::styled_text("hi", white),
    ///     View::tint((0, 0, 0), 128, Backdrop::dark()),
    /// ]);
    /// let mut buffer = Buffer::new(2, 1);
    /// scene.paint(Rect::new(0, 0, 2, 1), &mut buffer);
    /// assert_eq!(buffer.text_rows(), ["hi"]); // text survives, colours dim
    /// assert_eq!(buffer.get(0, 0).unwrap().attrs().fg, ResolvedColor::Rgb(127, 127, 127));
    /// ```
    #[must_use]
    pub const fn tint(color: (u8, u8, u8), alpha: u8, backdrop: Backdrop) -> Self {
        Self(Node::Tint {
            color,
            alpha,
            backdrop,
        })
    }

    /// One line per `'\n'`, with the foreground colour blending from `from`
    /// to `to` across each line, one grapheme at a time. Wide glyphs get one
    /// colour; the blend is by grapheme index.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Buffer, Rect, ResolvedColor, View};
    ///
    /// let mut buffer = Buffer::new(3, 1);
    /// View::gradient_text("abc", (0, 0, 0), (200, 0, 0)).paint(Rect::new(0, 0, 3, 1), &mut buffer);
    /// assert_eq!(buffer.get(1, 0).unwrap().attrs().fg, ResolvedColor::Rgb(100, 0, 0));
    /// ```
    #[must_use]
    pub fn gradient_text(text: &str, from: (u8, u8, u8), to: (u8, u8, u8)) -> Self {
        let lines = text
            .split('\n')
            .map(|line| {
                let clusters: Vec<&str> = graphemes(line).collect();
                let last = clusters.len().saturating_sub(1).max(1) as f32;
                clusters
                    .iter()
                    .enumerate()
                    .map(|(index, cluster)| {
                        let (r, g, b) = from.lerp(&to, index as f32 / last);
                        Span::new(
                            *cluster,
                            ResolvedAttributes::new(
                                ResolvedColor::Rgb(r, g, b),
                                ResolvedColor::Default,
                            ),
                        )
                    })
                    .collect()
            })
            .collect();
        Self(Node::Text {
            lines,
            align: Align::Start,
        })
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

    /// Places a [`Component`] here. The runtime gives it a state slot keyed
    /// by its position (or [`View::key`]), calls its `view`, and paints the
    /// result in this region.
    ///
    /// Views compare components by type and configuration, so an unchanged
    /// screen stays equal. Painting a view yourself, outside the runtime,
    /// draws a component as nothing: it has no state there.
    #[must_use]
    pub fn component<C: Component>(component: C) -> Self {
        Self(Node::Component(ComponentNode::new(component)))
    }

    /// Gives this view an explicit identity among its siblings (RFC-2 §2).
    ///
    /// Key the *outermost* view of each item in a dynamic list: component
    /// state inside it then follows the key when items reorder. Siblings
    /// should be all keyed or all unkeyed; duplicate keys among siblings get
    /// fresh state every frame. Both misuses assert in debug builds.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Constraint, View};
    ///
    /// let items = ["a", "b"].map(|id| (Constraint::Length(1), View::text(id).key(id)));
    /// let list = View::column(items);
    /// assert_ne!(list, View::column([(Constraint::Length(1), View::text("a"))]));
    /// ```
    #[must_use]
    pub fn key(self, key: impl Into<Key>) -> Self {
        Self(Node::Keyed {
            key: key.into(),
            child: Box::new(self),
        })
    }

    /// Replaces every component node with what `mount` returns for it, in
    /// depth-first paint order, tracking each node's identity path.
    pub(crate) fn expand(
        self,
        path: &mut Path,
        mount: &mut dyn FnMut(&mut Path, ComponentNode) -> Result<Self>,
    ) -> Result<Self> {
        match self.0 {
            Node::Component(node) => mount(path, node),
            Node::Keyed { key, child } => {
                // The key replaces the edge from the parent (or names the root).
                let saved = path.pop();
                path.push(match saved {
                    Some(Segment::Stateless(index)) => Segment::Stateless(index),
                    _ => Segment::Key(key.clone()),
                });
                let child = child.expand(path, mount);
                path.pop();
                if let Some(segment) = saved {
                    path.push(segment);
                }
                Ok(Self(Node::Keyed {
                    key,
                    child: Box::new(child?),
                }))
            }
            Node::Stack { layout, children } => Ok(Self(Node::Stack {
                layout,
                children: expand_children(children, path, mount)?,
            })),
            Node::Overlay(layers) => Ok(Self(Node::Overlay(expand_children(layers, path, mount)?))),
            Node::Padding { margin, child } => Ok(Self(Node::Padding {
                margin,
                child: Box::new(expand_only_child(*child, path, mount)?),
            })),
            Node::Border {
                style,
                attrs,
                title,
                child,
            } => Ok(Self(Node::Border {
                style,
                attrs,
                title,
                child: Box::new(expand_only_child(*child, path, mount)?),
            })),
            other @ (Node::Empty
            | Node::Fill(_)
            | Node::Gradient { .. }
            | Node::Tint { .. }
            | Node::Text { .. }) => Ok(Self(other)),
        }
    }

    /// The key of a keyed view, if this is one.
    fn own_key(&self) -> Option<&Key> {
        match &self.0 {
            Node::Keyed { key, .. } => Some(key),
            _ => None,
        }
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
            Node::Gradient {
                from,
                to,
                direction,
            } => paint_gradient(*from, *to, *direction, area, buffer),
            Node::Tint { alpha: 0, .. } => {}
            Node::Tint {
                color,
                alpha,
                backdrop,
            } => buffer.map_attrs(area, |attrs| ResolvedAttributes {
                fg: attrs.fg.blend(*color, *alpha, backdrop.fg),
                bg: attrs.bg.blend(*color, *alpha, backdrop.bg),
                ..*attrs
            }),
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
            Node::Keyed { child, .. } => child.paint(area, buffer),
            Node::Component(_) => {}
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

fn expand_only_child(
    child: View,
    path: &mut Path,
    mount: &mut dyn FnMut(&mut Path, ComponentNode) -> Result<View>,
) -> Result<View> {
    path.push(Segment::Child(0));
    let child = child.expand(path, mount);
    path.pop();
    child
}

/// Expands siblings, enforcing the RFC-2 §2 key rules: keyed and unkeyed
/// siblings must not mix (positional fallback), and a duplicate key gets
/// fresh state every frame. Both assert in debug builds.
fn expand_children(
    children: Vec<View>,
    path: &mut Path,
    mount: &mut dyn FnMut(&mut Path, ComponentNode) -> Result<View>,
) -> Result<Vec<View>> {
    let keyed = children
        .iter()
        .filter(|child| child.own_key().is_some())
        .count();
    let mixed = keyed != 0 && keyed != children.len();
    debug_assert!(
        !mixed,
        "keyed and unkeyed views are mixed among siblings; key all of them or none          (identity falls back to positions)"
    );
    let mut seen = HashSet::new();
    let mut expanded = Vec::with_capacity(children.len());
    for (index, child) in children.into_iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let duplicate = child.own_key().is_some_and(|key| !seen.insert(key.clone()));
        debug_assert!(
            !duplicate,
            "duplicate key {:?} among siblings; the extra views get fresh state every frame",
            child.own_key()
        );
        path.push(if duplicate {
            Segment::Stateless(index)
        } else {
            Segment::Child(index)
        });
        // Mixed siblings ignore their keys: positions decide identity.
        let child = if mixed { child.unkeyed() } else { child };
        let result = child.expand(path, mount);
        path.pop();
        expanded.push(result?);
    }
    Ok(expanded)
}

impl View {
    /// Drops an outer key, keeping what it wrapped.
    fn unkeyed(self) -> Self {
        match self.0 {
            Node::Keyed { child, .. } => *child,
            other => Self(other),
        }
    }
}

fn paint_gradient(
    from: (u8, u8, u8),
    to: (u8, u8, u8),
    direction: Direction,
    area: Rect,
    buffer: &mut Buffer,
) {
    let steps = match direction {
        Direction::Row => area.width,
        Direction::Column => area.height,
    };
    let last = f32::from(steps.saturating_sub(1).max(1));
    for step in 0..steps {
        let (r, g, b) = from.lerp(&to, f32::from(step) / last);
        let attrs = ResolvedAttributes::new(ResolvedColor::Default, ResolvedColor::Rgb(r, g, b));
        let slice = match direction {
            Direction::Row => Rect::new(area.x.saturating_add(step), area.y, 1, area.height),
            Direction::Column => Rect::new(area.x, area.y.saturating_add(step), area.width, 1),
        };
        buffer.fill(slice, attrs);
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

    #[derive(Debug, Clone, PartialEq)]
    struct Badge(&'static str);

    impl Component for Badge {
        type State = ();
        type Message = ();

        fn init(&self) -> Result<()> {
            Ok(())
        }

        fn update(
            &self,
            _state: &mut (),
            _event: crate::Event,
            _effects: &mut crate::Effects,
        ) -> Result<crate::Outcome> {
            Ok(crate::Outcome::Ignored)
        }

        fn view(&self, _state: &(), _cx: &crate::Context) -> Result<View> {
            Ok(View::text(self.0))
        }
    }

    #[test]
    fn component_views_compare_by_configuration_and_key() {
        assert_eq!(View::component(Badge("a")), View::component(Badge("a")));
        assert_ne!(View::component(Badge("a")), View::component(Badge("b")));
        assert_ne!(
            View::component(Badge("a")).key(1u64),
            View::component(Badge("a"))
        );
        assert_eq!(View::text("x").key("k"), View::text("x").key("k"));
    }

    #[test]
    fn unexpanded_components_paint_nothing_and_keys_paint_their_child() {
        let mut buffer = Buffer::new(3, 1);
        View::row([
            (Constraint::Length(1), View::component(Badge("a"))),
            (Constraint::Length(2), View::text("ok").key("k")),
        ])
        .paint(Rect::new(0, 0, 3, 1), &mut buffer);
        assert_eq!(buffer.text_rows(), [" ok"]);
    }
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

    fn bg_at(buffer: &Buffer, x: u16, y: u16) -> ResolvedColor {
        buffer.get(x, y).unwrap().attrs().bg
    }

    #[test]
    fn gradient_runs_from_end_to_end_in_both_directions() {
        let mut buffer = Buffer::new(5, 3);
        View::gradient((0, 0, 0), (200, 100, 40), Direction::Row)
            .paint(Rect::new(0, 0, 5, 3), &mut buffer);
        assert_eq!(bg_at(&buffer, 0, 0), ResolvedColor::Rgb(0, 0, 0));
        assert_eq!(bg_at(&buffer, 2, 1), ResolvedColor::Rgb(100, 50, 20));
        assert_eq!(bg_at(&buffer, 4, 2), ResolvedColor::Rgb(200, 100, 40));
        // Every row of a horizontal gradient is identical.
        assert_eq!(bg_at(&buffer, 3, 0), bg_at(&buffer, 3, 2));

        let mut buffer = Buffer::new(2, 3);
        View::gradient((0, 0, 0), (100, 100, 100), Direction::Column)
            .paint(Rect::new(0, 0, 2, 3), &mut buffer);
        assert_eq!(bg_at(&buffer, 0, 0), ResolvedColor::Rgb(0, 0, 0));
        assert_eq!(bg_at(&buffer, 1, 1), ResolvedColor::Rgb(50, 50, 50));
        assert_eq!(bg_at(&buffer, 0, 2), ResolvedColor::Rgb(100, 100, 100));
        assert_eq!(bg_at(&buffer, 0, 1), bg_at(&buffer, 1, 1));
    }

    #[test]
    fn gradient_degenerate_sizes_do_not_divide_by_zero() {
        let mut buffer = Buffer::new(4, 4);
        View::gradient((10, 20, 30), (200, 200, 200), Direction::Row)
            .paint(Rect::new(1, 1, 1, 2), &mut buffer);
        assert_eq!(bg_at(&buffer, 1, 1), ResolvedColor::Rgb(10, 20, 30));
        View::gradient((1, 1, 1), (2, 2, 2), Direction::Column)
            .paint(Rect::new(0, 0, 3, 0), &mut buffer);
        View::gradient((1, 1, 1), (2, 2, 2), Direction::Column)
            .paint(Rect::new(0, 0, 0, 3), &mut buffer);
    }

    #[test]
    fn tint_blends_foreground_and_background_and_keeps_symbols() {
        let attrs = ResolvedAttributes::new(
            ResolvedColor::Rgb(200, 200, 200),
            ResolvedColor::Rgb(0, 0, 100),
        )
        .with_bold();
        let scene = View::overlay([
            View::styled_text("ab", attrs),
            View::tint((0, 0, 0), 128, Backdrop::dark()),
        ]);
        let mut buffer = Buffer::new(3, 1);
        scene.paint(Rect::new(0, 0, 3, 1), &mut buffer);
        assert_eq!(buffer.text_rows(), ["ab "]);
        let cell = buffer.get(0, 0).unwrap().attrs();
        assert_eq!(cell.fg, ResolvedColor::Rgb(100, 100, 100));
        assert_eq!(cell.bg, ResolvedColor::Rgb(0, 0, 50));
        assert!(cell.bold, "decorations survive a tint");
        // An untouched cell has terminal-default colours: blends against the backdrop.
        let blank = buffer.get(2, 0).unwrap().attrs();
        assert_eq!(blank.bg, ResolvedColor::Rgb(12, 12, 12));
        assert_eq!(blank.fg, ResolvedColor::Rgb(102, 102, 102));
    }

    #[test]
    fn tint_alpha_zero_changes_nothing_visible_and_is_contained() {
        let mut buffer = Buffer::new(6, 1);
        buffer.set_string(0, 0, "日本語", red());
        let before = buffer.clone();
        View::tint((255, 255, 255), 0, Backdrop::dark()).paint(Rect::new(0, 0, 6, 1), &mut buffer);
        assert_eq!(buffer, before, "alpha 0 is a true no-op");
        // A tint over half a wide glyph tints the whole glyph, never half.
        let mut buffer = Buffer::new(4, 1);
        buffer.set_string(0, 0, "日本", plain());
        View::tint((255, 0, 0), 255, Backdrop::dark()).paint(Rect::new(1, 0, 1, 1), &mut buffer);
        assert_eq!(bg_at(&buffer, 0, 0), ResolvedColor::Rgb(255, 0, 0));
        assert_eq!(bg_at(&buffer, 1, 0), ResolvedColor::Rgb(255, 0, 0));
        assert_eq!(bg_at(&buffer, 2, 0), ResolvedColor::Default);
    }

    #[test]
    fn gradient_text_colours_each_grapheme_and_keeps_lines() {
        let mut buffer = Buffer::new(6, 2);
        View::gradient_text("abc\n日本", (0, 0, 0), (200, 0, 0))
            .paint(Rect::new(0, 0, 6, 2), &mut buffer);
        assert_eq!(buffer.text_rows(), ["abc   ", "日本  "]);
        let fg = |x, y| buffer.get(x, y).unwrap().attrs().fg;
        assert_eq!(fg(0, 0), ResolvedColor::Rgb(0, 0, 0));
        assert_eq!(fg(2, 0), ResolvedColor::Rgb(200, 0, 0));
        // Each line runs its own gradient; a wide glyph is one step.
        assert_eq!(fg(0, 1), ResolvedColor::Rgb(0, 0, 0));
        assert_eq!(fg(2, 1), ResolvedColor::Rgb(200, 0, 0));
        // A single grapheme takes the start colour (no division by zero).
        let mut one = Buffer::new(2, 1);
        View::gradient_text("x", (9, 9, 9), (99, 99, 99)).paint(Rect::new(0, 0, 2, 1), &mut one);
        assert_eq!(
            one.get(0, 0).unwrap().attrs().fg,
            ResolvedColor::Rgb(9, 9, 9)
        );
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
            rng.below(10)
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
            7 => View::overlay([random_view(rng, depth - 1), random_view(rng, depth - 1)]),
            8 => View::gradient(
                (0, 0, 0),
                (255, 128, 64),
                if rng.below(2) == 0 {
                    Direction::Row
                } else {
                    Direction::Column
                },
            ),
            _ => View::overlay([
                random_view(rng, depth - 1),
                View::tint((10, 20, 30), rng.below(256) as u8, Backdrop::dark()),
            ]),
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
