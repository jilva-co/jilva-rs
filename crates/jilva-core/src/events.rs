//! State and event propagation taxonomy (`docs/ARCHITECTURE.md` §3.4).
//!
//! **Phase-1 status:** a minimal event vocabulary proving the shape of the
//! taxonomy (timer, resize, app-defined message). The full ladder — keyboard
//! encodings, mouse modes, tick sources, focus and dispatch order — is RFC
//! detail. Flow stays unidirectional: events flow in, updates reconcile, and
//! components never touch the terminal.
//!
//! **Phase-2 addition:** [`InputEvent`] is the backend-decoded subset of the
//! taxonomy crossing the terminal seam (`docs/PHASE2_BACKEND_CONTRACT.md`
//! §2): key, mouse, resize, terminal focus, and opt-in paste. It carries no
//! application message, timer tick, or synthesised widget focus event. The
//! future application loop promotes input into its dispatch taxonomy.

use crate::capabilities::KeyEncoding;

/// An event delivered to the application for one transactional update.
///
/// Exactly one update follows one event; intermediate states are never
/// painted. Marked `#[non_exhaustive]` so keyboard, mouse, and future variants
/// can be added without breaking downstream exhaustive matches before `1.0`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// Timer tick, for animation and polling.
    Tick,
    /// Terminal resized; layout must be recomputed for the new dimensions.
    Resized {
        /// New terminal width in cells (columns).
        cols: u16,
        /// New terminal height in cells (rows).
        rows: u16,
    },
    /// Application-defined message through the user-message channel.
    Custom(String),
}

impl Event {
    /// Returns `true` when handling this event must invalidate layout.
    ///
    /// Only [`Event::Resized`] forces re-layout today; ticks and custom
    /// messages are paint-only until an RFC says otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Event;
    ///
    /// assert!(Event::Resized { cols: 80, rows: 24 }.affects_layout());
    /// assert!(!Event::Tick.affects_layout());
    /// ```
    #[must_use]
    pub fn affects_layout(&self) -> bool {
        match self {
            Self::Resized { .. } => true,
            Self::Tick | Self::Custom(_) => false,
        }
    }
}

/// Backend-decoded input event crossing the terminal seam.
///
/// This vocabulary contains terminal observations, not application state
/// dispatch. Terminal focus is distinct from synthesised widget focus.
/// Unsupported reports may be skipped by a backend; no event is fabricated
/// to fill in missing modifier, repeat, release, or focus information.
///
/// Text payloads are owned strings; no API assigns one Unicode scalar to
/// one terminal cell or asserts that an input string is a single grapheme.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputEvent {
    /// A decoded key event, with kind and encoding source recorded.
    Key(KeyEvent),
    /// A decoded mouse report. Only produced when the capability snapshot
    /// offers mouse support; absence of support yields no mouse events,
    /// never synthetic ones.
    Mouse(MouseEvent),
    /// Terminal focus gained (`true`) or lost (`false`). Only produced
    /// when focus reporting was explicitly enabled.
    Focus(bool),
    /// Untrusted bracketed-paste text, only when that mode is enabled.
    ///
    /// Paste can contain newlines and terminal control characters. It is
    /// input data and must pass output validation before being displayed.
    Paste(String),
    /// Terminal resized; layout must be recomputed for the new dimensions.
    /// Backends discard zero dimensions, including directly scripted enum
    /// values (see [`InputEvent::resize`]).
    Resize {
        /// New terminal width in cells (columns); backend reports are non-zero.
        cols: u16,
        /// New terminal height in cells (rows); backend reports are non-zero.
        rows: u16,
    },
}

impl InputEvent {
    /// Builds a resize event, rejecting degenerate sizes.
    ///
    /// Mirrors the `Backend::size` non-degenerate rule: a zero width or
    /// height is never a valid size, so it is never a valid resize event
    /// either. Returns `None` when `cols == 0 || rows == 0`; the backend
    /// drops such reports (counted, debug-visible) instead of emitting them.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::InputEvent;
    ///
    /// assert!(InputEvent::resize(80, 24).is_some());
    /// assert_eq!(InputEvent::resize(0, 24), None);
    /// ```
    #[must_use]
    pub const fn resize(cols: u16, rows: u16) -> Option<Self> {
        if cols == 0 || rows == 0 {
            return None;
        }
        Some(Self::Resize { cols, rows })
    }

    /// Returns `true` when handling this event must invalidate layout.
    ///
    /// Phase-2 invalidation floor: `Resize` invalidates layout; key and
    /// mouse events do not by backend assertion alone (widgets report their
    /// own change class).
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::InputEvent;
    ///
    /// assert!(InputEvent::Resize { cols: 80, rows: 24 }.affects_layout());
    /// ```
    #[must_use]
    pub const fn affects_layout(&self) -> bool {
        match self {
            Self::Resize { .. } => true,
            Self::Key(_) | Self::Mouse(_) | Self::Focus(_) | Self::Paste(_) => false,
        }
    }
}

/// A decoded key: identity, modifiers, kind, and encoding source.
///
/// The `encoding` field records HOW the key arrived ([`KeyEncoding`]) so the
/// loop can apply the documented degradation ladder:
///
/// - `Legacy`: byte/escape-sequence encodings. Cannot distinguish `Ctrl+I`
///   from `Tab`, `Ctrl+M` from `Enter`, or `Ctrl+H` from `Backspace`;
///   shifted printable characters arrive pre-shifted with no reliable shift
///   flag. A legacy byte stream normally reports only presses, although a
///   platform input API may supply additional event-kind information
///   without enhanced key identity. Consumers MUST NOT assume a
///   one-to-one mapping between physical keys and legacy events.
/// - `Enhanced` (e.g. a negotiated Kitty keyboard protocol): richer key
///   identity. Exact available features depend on the negotiated flags.
///   A repeat or release alone is not evidence of enhanced encoding.
///
/// No crossterm (or any terminal-library) type leaks here: backends convert
/// from library types into these plain-data structs inside `jilva-terminal`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct KeyEvent {
    /// Which key was pressed.
    pub code: KeyCode,
    /// Modifiers held with the press (as far as the encoding reports them).
    pub modifiers: KeyModifiers,
    /// Reported press, autorepeat, or release. Constructors default to
    /// press; decoders preserve additional information with `with_kind`.
    pub kind: KeyEventKind,
    /// The encoding source: legacy vs enhanced.
    pub encoding: KeyEncoding,
}

impl KeyEvent {
    /// Builds a printable-text key event; rejects empty or control text.
    ///
    /// Unicode strings (including emoji ZWJ sequences, flags, and combining
    /// marks) travel intact. This does not perform grapheme segmentation.
    #[must_use]
    pub fn text(
        text: impl Into<String>,
        modifiers: KeyModifiers,
        encoding: KeyEncoding,
    ) -> Option<Self> {
        let text = text.into();
        if text.is_empty() || text.chars().any(char::is_control) {
            return None;
        }
        Some(Self {
            code: KeyCode::Text(text),
            modifiers,
            kind: KeyEventKind::Press,
            encoding,
        })
    }

    /// Builds a special-key event.
    #[must_use]
    pub const fn special(key: SpecialKey, modifiers: KeyModifiers, encoding: KeyEncoding) -> Self {
        Self {
            code: KeyCode::Special(key),
            modifiers,
            kind: KeyEventKind::Press,
            encoding,
        }
    }

    /// Builds a function-key event (`F1`–`F24`); returns `None` for numbers
    /// outside `1..=24`.
    #[must_use]
    pub const fn function(
        number: u8,
        modifiers: KeyModifiers,
        encoding: KeyEncoding,
    ) -> Option<Self> {
        if number == 0 || number > 24 {
            return None;
        }
        Some(Self {
            code: KeyCode::Function(number),
            modifiers,
            kind: KeyEventKind::Press,
            encoding,
        })
    }

    /// Records a reported kind without inferring a different key encoding.
    #[must_use]
    pub const fn with_kind(mut self, kind: KeyEventKind) -> Self {
        self.kind = kind;
        self
    }

    /// Returns `true` for printable-text keys.
    #[must_use]
    pub const fn is_printable(&self) -> bool {
        matches!(self.code, KeyCode::Text(_))
    }

    /// Decodes one legacy byte (C0 control, DEL, or printable ASCII) into a
    /// key event with [`KeyEncoding::Legacy`].
    ///
    /// Total over the decodable range; returns `None` for bytes with no
    /// legacy meaning (`NUL`, remaining C0 controls, C1 controls), which the
    /// backend skips (counted, debug-visible) and continues past — one
    /// garbage byte MUST NOT poison the stream. Multi-byte UTF-8 leaders
    /// are NOT handled here; they go through [`KeyEvent::decode_text_cluster`]
    /// once the full cluster is buffered.
    ///
    /// Degradation, documented: `0x01..=0x1A` report `Ctrl+letter` with the
    /// letter identity only (`0x09`/`0x0D` are claimed by Tab/Enter first, so
    /// `Ctrl+I`/`Ctrl+M` are indistinguishable from them by construction).
    #[must_use]
    pub fn decode_legacy_byte(byte: u8) -> Option<Self> {
        let no_mod = KeyModifiers::NONE;
        match byte {
            0x08 | 0x7F => Some(Self::special(
                SpecialKey::Backspace,
                no_mod,
                KeyEncoding::Legacy,
            )),
            0x09 => Some(Self::special(SpecialKey::Tab, no_mod, KeyEncoding::Legacy)),
            0x0D => Some(Self::special(
                SpecialKey::Enter,
                no_mod,
                KeyEncoding::Legacy,
            )),
            0x1B => Some(Self::special(
                SpecialKey::Escape,
                no_mod,
                KeyEncoding::Legacy,
            )),
            0x01..=0x07 | 0x0A..=0x0C | 0x0E..=0x1A => {
                let letter = (byte + b'a' - 1) as char;
                let mut text = String::new();
                text.push(letter);
                Some(Self {
                    code: KeyCode::Text(text),
                    modifiers: KeyModifiers::CTRL,
                    kind: KeyEventKind::Press,
                    encoding: KeyEncoding::Legacy,
                })
            }
            0x20..=0x7E => {
                let mut text = String::new();
                text.push(byte as char);
                Some(Self {
                    code: KeyCode::Text(text),
                    modifiers: no_mod,
                    kind: KeyEventKind::Press,
                    encoding: KeyEncoding::Legacy,
                })
            }
            _ => None,
        }
    }

    /// Decodes a buffered UTF-8 grapheme cluster into a printable key event.
    ///
    /// Returns `None` for empty input or input containing Unicode control
    /// characters (those belong to [`KeyEvent::decode_legacy_byte`]). The
    /// cluster is stored whole — segmentation authority sits with the width
    /// layer, never with this decoder. Encoding is recorded as
    /// [`KeyEncoding::Legacy`] with no modifiers: a bare pasted/typed
    /// string carries no modifier evidence, and claiming any would be a
    /// guess.
    #[must_use]
    pub fn decode_text_cluster(text: &str) -> Option<Self> {
        Self::text(text, KeyModifiers::NONE, KeyEncoding::Legacy)
    }
}

/// A key's reported transition. Legacy byte streams usually expose presses
/// only; repeat/release must never be synthesised from timing guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyEventKind {
    /// Initial press, or a press when no finer distinction is available.
    Press,
    /// A reported autorepeat of a held key.
    Repeat,
    /// A reported key release.
    Release,
}

/// Key identity: printable cluster text, function keys, or named specials.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyCode {
    /// Printable input text. Constructors reject empty and control text;
    /// directly constructed enum values are ordinary unvalidated data.
    Text(String),
    /// Function key number (`1` = `F1`, up to `24`).
    Function(u8),
    /// Named non-printable key.
    Special(SpecialKey),
}

/// Named non-printable keys decodable from terminal input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpecialKey {
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Shift+Tab (back-tab).
    BackTab,
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Insert.
    Insert,
    /// Delete (forward delete).
    Delete,
}

/// Modifier state held with a key press, as far as the encoding reports it.
///
/// Legacy encodings report modifiers unreliably (see [`KeyEvent`]); absent
/// evidence is reported as not-held, never synthesised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct KeyModifiers {
    /// Shift held.
    pub shift: bool,
    /// Control held.
    pub ctrl: bool,
    /// Alt (meta) held.
    pub alt: bool,
    /// Super (logo/command) held. Rarely reported by legacy encodings.
    pub super_key: bool,
    /// Hyper modifier held, when explicitly reported.
    pub hyper: bool,
    /// Meta modifier held, when explicitly reported separately from Alt.
    pub meta: bool,
}

impl KeyModifiers {
    /// Builds a modifier set from its four flags.
    ///
    /// Decoders use this to report exactly what the encoding evidenced;
    /// absent evidence is reported as not-held, never synthesised.
    #[must_use]
    pub const fn new(shift: bool, ctrl: bool, alt: bool, super_key: bool) -> Self {
        Self {
            shift,
            ctrl,
            alt,
            super_key,
            hyper: false,
            meta: false,
        }
    }

    /// Records an explicitly reported Hyper modifier.
    #[must_use]
    pub const fn with_hyper(mut self, held: bool) -> Self {
        self.hyper = held;
        self
    }

    /// Records an explicitly reported Meta modifier.
    #[must_use]
    pub const fn with_meta(mut self, held: bool) -> Self {
        self.meta = held;
        self
    }

    /// No modifiers held.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        super_key: false,
        hyper: false,
        meta: false,
    };

    /// Control held, nothing else.
    pub const CTRL: Self = Self {
        shift: false,
        ctrl: true,
        alt: false,
        super_key: false,
        hyper: false,
        meta: false,
    };

    /// Alt held, nothing else.
    pub const ALT: Self = Self {
        shift: false,
        ctrl: false,
        alt: true,
        super_key: false,
        hyper: false,
        meta: false,
    };

    /// Shift held, nothing else.
    pub const SHIFT: Self = Self {
        shift: true,
        ctrl: false,
        alt: false,
        super_key: false,
        hyper: false,
        meta: false,
    };

    /// Returns `true` when no modifier is held.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::KeyModifiers;
    ///
    /// assert!(KeyModifiers::NONE.is_empty());
    /// assert!(!KeyModifiers::CTRL.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.super_key && !self.hyper && !self.meta
    }
}

/// A decoded mouse report: action kind, button, and cell position.
///
/// Position is in 0-based cells from the terminal's top-left. Only produced
/// when the capability snapshot offers mouse support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MouseEvent {
    /// What happened.
    pub kind: MouseKind,
    /// Which button (or [`MouseButton::None`] for move/scroll).
    pub button: MouseButton,
    /// Column in cells from the terminal's left edge (0-based).
    pub cols: u16,
    /// Row in cells from the terminal's top edge (0-based).
    pub rows: u16,
    /// Reported modifiers; absent information is represented by `NONE`.
    pub modifiers: KeyModifiers,
}

impl MouseEvent {
    /// Builds a mouse report from its decoded parts.
    ///
    /// Backends use this after converting wire coordinates to 0-based cells;
    /// only produce reports the capability snapshot offers (absence of mouse
    /// support yields no events, never synthetic ones).
    #[must_use]
    pub const fn new(kind: MouseKind, button: MouseButton, cols: u16, rows: u16) -> Self {
        Self {
            kind,
            button,
            cols,
            rows,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Attaches modifiers reported with the mouse event.
    #[must_use]
    pub const fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Decodes an SGR (1006) mouse report into a [`MouseEvent`].
    ///
    /// Total pure function over the SGR button-code domain: `code` is the
    /// `Cb` parameter, `released` is whether the terminator was `m`
    /// (release) rather than `M` (press/motion). The documented SGR
    /// modifier bits (shift/alt/ctrl) are retained. Coordinates pass
    /// through unchanged (1-based → 0-based
    /// conversion happens in the backend, which owns the wire format).
    ///
    /// Mapping: low two bits select left/middle/right (3 = no button);
    /// bit 6 (`64`) marks wheel (0/1/2/3 = up/down/left/right); bit 5
    /// (`32`) marks motion (button held = drag, no button = hover move).
    #[must_use]
    pub const fn from_sgr(code: u8, cols: u16, rows: u16, released: bool) -> Self {
        let button = match code & 0b11 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::None,
        };
        let kind = if code & 64 != 0 {
            match code & 0b11 {
                0 => MouseKind::ScrollUp,
                1 => MouseKind::ScrollDown,
                2 => MouseKind::ScrollLeft,
                _ => MouseKind::ScrollRight,
            }
        } else if released {
            MouseKind::Up
        } else if code & 32 != 0 {
            match button {
                MouseButton::None => MouseKind::Move,
                _ => MouseKind::Drag,
            }
        } else {
            MouseKind::Down
        };
        let button = match kind {
            MouseKind::ScrollUp
            | MouseKind::ScrollDown
            | MouseKind::ScrollLeft
            | MouseKind::ScrollRight
            | MouseKind::Move => MouseButton::None,
            _ => button,
        };
        Self {
            kind,
            button,
            cols,
            rows,
            modifiers: KeyModifiers::new(code & 4 != 0, code & 16 != 0, code & 8 != 0, false),
        }
    }
}

/// Mouse action kind: press, release, drag, hover-move, or scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MouseKind {
    /// Button pressed.
    Down,
    /// Button released.
    Up,
    /// Motion with a button held.
    Drag,
    /// Hover motion with no button held.
    Move,
    /// Wheel up.
    ScrollUp,
    /// Wheel down.
    ScrollDown,
    /// Horizontal wheel left.
    ScrollLeft,
    /// Horizontal wheel right.
    ScrollRight,
}

/// Mouse button identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MouseButton {
    /// Left (primary) button.
    Left,
    /// Middle button.
    Middle,
    /// Right (secondary) button.
    Right,
    /// No button (move/scroll reports).
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::KeyEncoding as Enc;

    #[test]
    fn only_resize_invalidates_layout() {
        assert!(Event::Resized { cols: 80, rows: 24 }.affects_layout());
        assert!(!Event::Tick.affects_layout());
        assert!(!Event::Custom("hello".to_string()).affects_layout());
    }

    #[test]
    fn only_input_resize_invalidates_layout() {
        assert!(input_resize(80, 24).affects_layout());
        assert!(!key_fixture().affects_layout());
        assert!(!mouse_fixture().affects_layout());
    }

    #[test]
    fn resize_rejects_degenerate_sizes() {
        assert_eq!(InputEvent::resize(0, 24), None);
        assert_eq!(InputEvent::resize(80, 0), None);
        assert_eq!(InputEvent::resize(0, 0), None);
        assert!(InputEvent::resize(80, 24).is_some());
    }

    #[test]
    fn legacy_byte_decoding_covers_controls_and_printables() {
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x09),
            Some(KeyEvent::special(
                SpecialKey::Tab,
                KeyModifiers::NONE,
                Enc::Legacy
            ))
        );
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x0D),
            Some(KeyEvent::special(
                SpecialKey::Enter,
                KeyModifiers::NONE,
                Enc::Legacy
            ))
        );
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x1B),
            Some(KeyEvent::special(
                SpecialKey::Escape,
                KeyModifiers::NONE,
                Enc::Legacy
            ))
        );
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x7F),
            Some(KeyEvent::special(
                SpecialKey::Backspace,
                KeyModifiers::NONE,
                Enc::Legacy
            ))
        );
        // Ctrl+A..Ctrl+Z arrive as 0x01..=0x1A; the letter identity survives,
        // the release event and exact modifier chord do not (degraded).
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x01),
            Some(KeyEvent {
                code: KeyCode::Text("a".to_string()),
                modifiers: KeyModifiers::CTRL,
                kind: KeyEventKind::Press,
                encoding: Enc::Legacy,
            })
        );
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x41),
            Some(KeyEvent {
                code: KeyCode::Text("A".to_string()),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                encoding: Enc::Legacy,
            })
        );
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x20).map(|k| k.is_printable()),
            Some(true)
        );
        // Unmapped C0/C1 controls are skipped by the caller, never error.
        assert_eq!(KeyEvent::decode_legacy_byte(0x00), None);
        assert_eq!(KeyEvent::decode_legacy_byte(0x1F), None);
        assert_eq!(KeyEvent::decode_legacy_byte(0x80), None);
    }

    #[test]
    fn text_cluster_decoding_rejects_empty_and_controls() {
        assert!(KeyEvent::decode_text_cluster("").is_none());
        assert!(KeyEvent::decode_text_cluster("\u{0}").is_none());
        assert!(KeyEvent::decode_text_cluster("\u{9}").is_none());
        let emoji = KeyEvent::decode_text_cluster("👩‍💻").expect("cluster decodes");
        assert_eq!(emoji.code, KeyCode::Text("👩‍💻".to_string()));
        assert!(emoji.is_printable());
        assert!(
            !KeyEvent::special(SpecialKey::Enter, KeyModifiers::NONE, Enc::Legacy).is_printable()
        );
    }

    #[test]
    fn sgr_mouse_mapping_covers_press_drag_move_scroll_release() {
        let at = |kind, button| MouseEvent::new(kind, button, 10, 5);
        // Plain press / release of each button.
        assert_eq!(
            MouseEvent::from_sgr(0, 10, 5, false),
            at(MouseKind::Down, MouseButton::Left)
        );
        assert_eq!(
            MouseEvent::from_sgr(1, 10, 5, false),
            at(MouseKind::Down, MouseButton::Middle)
        );
        assert_eq!(
            MouseEvent::from_sgr(2, 10, 5, false),
            at(MouseKind::Down, MouseButton::Right)
        );
        assert_eq!(
            MouseEvent::from_sgr(0, 10, 5, true),
            at(MouseKind::Up, MouseButton::Left)
        );
        assert_eq!(
            MouseEvent::from_sgr(3, 10, 5, true),
            at(MouseKind::Up, MouseButton::None)
        );
        // Drag (button held + motion bit) vs hover-move (no button + motion).
        assert_eq!(
            MouseEvent::from_sgr(32, 10, 5, false),
            at(MouseKind::Drag, MouseButton::Left)
        );
        assert_eq!(
            MouseEvent::from_sgr(35, 10, 5, false),
            at(MouseKind::Move, MouseButton::None)
        );
        // Wheel.
        assert_eq!(
            MouseEvent::from_sgr(64, 10, 5, false),
            at(MouseKind::ScrollUp, MouseButton::None)
        );
        assert_eq!(
            MouseEvent::from_sgr(65, 10, 5, false),
            at(MouseKind::ScrollDown, MouseButton::None)
        );
        assert_eq!(
            MouseEvent::from_sgr(66, 10, 5, false),
            at(MouseKind::ScrollLeft, MouseButton::None)
        );
        assert_eq!(
            MouseEvent::from_sgr(67, 10, 5, false),
            at(MouseKind::ScrollRight, MouseButton::None)
        );
        // Modifier bits survive independently from button/motion bits.
        assert_eq!(
            MouseEvent::from_sgr(4, 10, 5, false),
            at(MouseKind::Down, MouseButton::Left).with_modifiers(KeyModifiers::SHIFT)
        );
        assert_eq!(
            MouseEvent::from_sgr(68, 10, 5, false),
            at(MouseKind::ScrollUp, MouseButton::None).with_modifiers(KeyModifiers::SHIFT)
        );
        let all = MouseEvent::from_sgr(4 | 8 | 16, 10, 5, false);
        assert_eq!(all.modifiers, KeyModifiers::new(true, true, true, false));
    }

    fn key_fixture() -> InputEvent {
        InputEvent::Key(KeyEvent::special(
            SpecialKey::Enter,
            KeyModifiers::NONE,
            Enc::Legacy,
        ))
    }

    fn mouse_fixture() -> InputEvent {
        InputEvent::Mouse(MouseEvent::new(MouseKind::Down, MouseButton::Left, 1, 1))
    }

    fn input_resize(cols: u16, rows: u16) -> InputEvent {
        InputEvent::Resize { cols, rows }
    }

    #[test]
    fn key_kinds_preserve_reported_identity_and_modifiers() {
        let modifiers = KeyModifiers::new(true, true, true, true)
            .with_hyper(true)
            .with_meta(true);
        let key = KeyEvent::text("e\u{301}", modifiers, Enc::Legacy).unwrap();
        assert_eq!(key.kind, KeyEventKind::Press);
        for kind in [
            KeyEventKind::Press,
            KeyEventKind::Repeat,
            KeyEventKind::Release,
        ] {
            let reported = key.clone().with_kind(kind);
            assert_eq!(reported.kind, kind);
            assert_eq!(reported.code, key.code);
            assert_eq!(reported.modifiers, modifiers);
            // Kind information alone cannot prove enhanced key identity.
            assert_eq!(reported.encoding, Enc::Legacy);
        }
        assert!(!KeyModifiers::NONE.with_hyper(true).is_empty());
        assert!(!KeyModifiers::NONE.with_meta(true).is_empty());
    }

    #[test]
    fn printable_key_constructors_reject_unicode_controls() {
        for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
            let control = char::from_u32(codepoint).unwrap().to_string();
            assert!(KeyEvent::text(&control, KeyModifiers::NONE, Enc::Legacy).is_none());
            assert!(KeyEvent::decode_text_cluster(&control).is_none());
        }
        assert_eq!(
            KeyEvent::decode_legacy_byte(0x08),
            Some(KeyEvent::special(
                SpecialKey::Backspace,
                KeyModifiers::NONE,
                Enc::Legacy
            ))
        );
        assert!(KeyEvent::text("👩‍💻日本語", KeyModifiers::NONE, Enc::Legacy).is_some());
    }

    #[test]
    fn terminal_focus_and_untrusted_paste_do_not_assert_layout_changes() {
        assert!(!InputEvent::Focus(true).affects_layout());
        assert!(!InputEvent::Focus(false).affects_layout());
        assert!(!InputEvent::Paste("one\ntwo\u{1b}[2J".into()).affects_layout());
    }
}
