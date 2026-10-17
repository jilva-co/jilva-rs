//! The grapheme-capable terminal cell (RFC-0004 §3).

use super::width::{cluster_width, graphemes};
use crate::output::ResolvedAttributes;
use std::fmt;

/// Bytes stored inline before a symbol spills to the heap. Covers every
/// single-scalar symbol, base-plus-combining pairs, flags, and common emoji
/// with a variation selector or ZWJ pair without allocating.
const INLINE_CAP: usize = 15;

/// Owned text of one cell: inline when short, boxed when long.
///
/// Canonical form (inline iff it fits) keeps equality a plain string
/// comparison; the type is private so no caller can construct a
/// non-canonical value.
#[derive(Clone)]
enum Symbol {
    Inline { len: u8, bytes: [u8; INLINE_CAP] },
    Heap(Box<str>),
}

impl Symbol {
    const fn space() -> Self {
        let mut bytes = [0u8; INLINE_CAP];
        bytes[0] = b' ';
        Self::Inline { len: 1, bytes }
    }

    const fn empty() -> Self {
        Self::Inline {
            len: 0,
            bytes: [0u8; INLINE_CAP],
        }
    }

    fn new(text: &str) -> Self {
        if text.len() <= INLINE_CAP {
            let mut bytes = [0u8; INLINE_CAP];
            bytes[..text.len()].copy_from_slice(text.as_bytes());
            // `len <= INLINE_CAP == 15`, so the cast cannot truncate.
            Self::Inline {
                len: text.len() as u8,
                bytes,
            }
        } else {
            Self::Heap(text.into())
        }
    }

    fn as_str(&self) -> &str {
        match self {
            // Only `Symbol::new` writes inline bytes, always copied from a
            // `&str`, so the slice is valid UTF-8 and this never takes the
            // fallback arm.
            Self::Inline { len, bytes } => {
                std::str::from_utf8(&bytes[..usize::from(*len)]).unwrap_or("\u{fffd}")
            }
            Self::Heap(text) => text,
        }
    }
}

impl PartialEq for Symbol {
    /// Compares representations directly. The canonical form (inline iff it
    /// fits) makes an inline/heap pair unequal by construction, and avoids
    /// re-validating UTF-8 on the diff's hot path.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Inline {
                    len: a_len,
                    bytes: a,
                },
                Self::Inline {
                    len: b_len,
                    bytes: b,
                },
            ) => a_len == b_len && a[..usize::from(*a_len)] == b[..usize::from(*b_len)],
            (Self::Heap(a), Self::Heap(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Symbol {}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

/// One terminal cell: a grapheme cluster, its resolved attributes, and the
/// number of columns it occupies.
///
/// # Width and continuation cells
///
/// A cell is one of three kinds, told apart by [`Cell::width`]:
///
/// | width | kind | symbol |
/// |---|---|---|
/// | `1` | narrow cell | one grapheme cluster (`"a"`, `"e\u{301}"`) |
/// | `2` | wide head | one double-width cluster (`"日"`, `"👩‍💻"`) |
/// | `0` | continuation | empty; the right half of the wide head on its left |
///
/// A [`crate::renderer::Buffer`] maintains the pairing invariant (every wide
/// head is followed by exactly one continuation, and every continuation
/// follows a wide head), so a half-overwritten wide glyph is unrepresentable.
///
/// # Empty versus styled blank
///
/// [`Cell::blank`] — a space with plain attributes — is the *ground state*: it
/// is what a freshly cleared terminal shows, so diffing never needs to emit
/// it against an equally blank previous frame. A space with a background
/// colour ([`Cell::styled_blank`]) is a different cell and *is* emitted.
///
/// # Example
///
/// ```rust
/// use jilva_core::{Cell, ResolvedAttributes};
///
/// let cell = Cell::try_new("日", ResolvedAttributes::plain()).unwrap();
/// assert_eq!(cell.symbol(), "日");
/// assert_eq!(cell.width(), 2);
/// assert!(!cell.is_blank());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    symbol: Symbol,
    attrs: ResolvedAttributes,
    width: u8,
}

impl Cell {
    /// Returns the ground-state cell: a space with plain attributes.
    #[must_use]
    pub const fn blank() -> Self {
        Self::styled_blank(ResolvedAttributes::plain())
    }

    /// Returns a space carrying `attrs` (for example a background fill).
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Cell, ResolvedAttributes, ResolvedColor};
    ///
    /// let fill = Cell::styled_blank(ResolvedAttributes::new(
    ///     ResolvedColor::Default,
    ///     ResolvedColor::Palette(24),
    /// ));
    /// assert!(fill.is_blank());
    /// assert!(!fill.is_ground_state());
    /// ```
    #[must_use]
    pub const fn styled_blank(attrs: ResolvedAttributes) -> Self {
        Self {
            symbol: Symbol::space(),
            attrs,
            width: 1,
        }
    }

    /// Builds a cell from exactly one renderable grapheme cluster.
    ///
    /// Returns `None` when `cluster` is empty, is more than one grapheme
    /// cluster, contains a control character, or has zero width. Nothing is
    /// repaired or truncated: a rejected input is the caller's bug to see.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{Cell, ResolvedAttributes};
    ///
    /// let plain = ResolvedAttributes::plain();
    /// assert!(Cell::try_new("x", plain).is_some());
    /// assert!(Cell::try_new("xy", plain).is_none()); // two clusters
    /// assert!(Cell::try_new("\u{1b}", plain).is_none()); // control
    /// ```
    #[must_use]
    pub fn try_new(cluster: &str, attrs: ResolvedAttributes) -> Option<Self> {
        let mut parts = graphemes(cluster);
        let first = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        match cluster_width(first) {
            0 => None,
            width => Some(Self::from_cluster(first, width, attrs)),
        }
    }

    /// Builds a cell from a single `char`, or `None` when it is not
    /// renderable on its own (a control or zero-width scalar).
    #[must_use]
    pub fn from_char(symbol: char, attrs: ResolvedAttributes) -> Option<Self> {
        let mut buffer = [0u8; 4];
        Self::try_new(symbol.encode_utf8(&mut buffer), attrs)
    }

    /// Internal constructor; `width` must be `1` or `2` and `cluster` one
    /// renderable cluster of that width.
    pub(super) fn from_cluster(cluster: &str, width: u8, attrs: ResolvedAttributes) -> Self {
        debug_assert!(width == 1 || width == 2);
        Self {
            symbol: Symbol::new(cluster),
            attrs,
            width,
        }
    }

    /// Internal constructor for the right half of a wide head.
    pub(super) const fn continuation(attrs: ResolvedAttributes) -> Self {
        Self {
            symbol: Symbol::empty(),
            attrs,
            width: 0,
        }
    }

    /// Replaces the attributes in place (buffer-internal restyling).
    pub(super) const fn restyle(&mut self, attrs: ResolvedAttributes) {
        self.attrs = attrs;
    }

    /// Returns the cluster text (empty for a continuation cell).
    #[must_use]
    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }

    /// Returns the resolved visual attributes of this cell.
    #[must_use]
    pub const fn attrs(&self) -> &ResolvedAttributes {
        &self.attrs
    }

    /// Returns `0` (continuation), `1` (narrow) or `2` (wide head).
    #[must_use]
    pub const fn width(&self) -> u8 {
        self.width
    }

    /// Returns `true` for the right half of a double-width cluster.
    #[must_use]
    pub const fn is_continuation(&self) -> bool {
        self.width == 0
    }

    /// Returns `true` when the symbol is a single space, whatever its
    /// attributes.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.width == 1 && self.symbol.as_str() == " "
    }

    /// Returns `true` for the ground state: a space with plain attributes,
    /// indistinguishable from a freshly cleared terminal cell.
    #[must_use]
    pub fn is_ground_state(&self) -> bool {
        self.is_blank() && self.attrs == ResolvedAttributes::plain()
    }

    /// Returns this cell with `attrs` replacing its attributes.
    #[must_use]
    pub const fn with_attrs(mut self, attrs: ResolvedAttributes) -> Self {
        self.attrs = attrs;
        self
    }
}

impl Default for Cell {
    /// Returns the ground-state cell ([`Cell::blank`]).
    fn default() -> Self {
        Self::blank()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::ResolvedColor;

    fn plain() -> ResolvedAttributes {
        ResolvedAttributes::plain()
    }

    #[test]
    fn blank_is_the_default_ground_state() {
        assert_eq!(Cell::default(), Cell::blank());
        assert!(Cell::blank().is_blank());
        assert!(Cell::blank().is_ground_state());
        assert_eq!(Cell::blank().width(), 1);
        assert_eq!(Cell::blank().symbol(), " ");
    }

    #[test]
    fn styled_blank_is_blank_but_not_ground_state() {
        let attrs = ResolvedAttributes::new(ResolvedColor::Default, ResolvedColor::Palette(24));
        let fill = Cell::styled_blank(attrs);
        assert!(fill.is_blank());
        assert!(!fill.is_ground_state());
        assert_ne!(fill, Cell::blank());
    }

    #[test]
    fn try_new_accepts_exactly_one_renderable_cluster() {
        for (text, width) in [("x", 1), ("e\u{301}", 1), ("日", 2), ("👩‍💻", 2), ("🇺🇿", 2)]
        {
            let cell = Cell::try_new(text, plain()).unwrap_or_else(|| panic!("{text:?}"));
            assert_eq!(cell.symbol(), text);
            assert_eq!(cell.width(), width);
            assert!(!cell.is_continuation());
        }
        for text in ["", "ab", "\n", "a\u{1b}", "\u{200b}", "\u{301}", "日本"] {
            assert!(Cell::try_new(text, plain()).is_none(), "{text:?}");
        }
        assert!(Cell::from_char('x', plain()).is_some());
        assert!(Cell::from_char('\t', plain()).is_none());
    }

    #[test]
    fn long_clusters_spill_to_the_heap_without_changing_equality() {
        // A family ZWJ sequence is far longer than the inline capacity.
        let family = "👨‍👩‍👧‍👦";
        assert!(family.len() > INLINE_CAP);
        let heap = Cell::try_new(family, plain()).unwrap();
        assert_eq!(heap.symbol(), family);
        assert_eq!(heap, heap.clone());
        let short = Cell::try_new("a", plain()).unwrap();
        assert_ne!(heap, short);
        // Round trip at the inline boundary.
        let edge = "e\u{301}\u{302}\u{303}\u{304}\u{305}\u{306}";
        assert_eq!(Cell::try_new(edge, plain()).unwrap().symbol(), edge);
    }

    #[test]
    fn continuation_has_no_symbol_and_zero_width() {
        let cont = Cell::continuation(plain());
        assert!(cont.is_continuation());
        assert_eq!(cont.symbol(), "");
        assert_eq!(cont.width(), 0);
        assert!(!cont.is_blank());
        assert_ne!(cont, Cell::blank());
    }

    #[test]
    fn attributes_participate_in_equality() {
        let bold = plain().with_bold();
        let a = Cell::try_new("a", plain()).unwrap();
        assert_ne!(a, a.clone().with_attrs(bold));
        assert_eq!(a.clone().with_attrs(bold).attrs(), &bold);
    }
}
