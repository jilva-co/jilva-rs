//! Capability domain types: plain-data terminal profiles.
//!
//! **Phase-2 status:** this module is the single canonical definition of the
//! capability model (`docs/PHASE2_BACKEND_CONTRACT.md` §4). `jilva-core` owns
//! these types; `jilva-terminal` populates them through detection and the
//! loop consumes one domain struct at a time (never the aggregate, except at
//! the two seams where [`TerminalCapabilities`] travels).
//!
//! Platform-neutral and TTY-free: everything here is data plus total pure
//! functions over injected string slices. Nothing reads the process
//! environment, touches a terminal handle, or branches on the OS. Detection
//! (reading `std::env`, probing a TTY) lives in `jilva-terminal` and calls
//! into [`color_tier_from_env`], [`background_from_colorfgbg`], and
//! [`no_color_active`] with values it collected.
//!
//! Conservatism rule (contract §4.3): on uncertainty the baseline is
//! [`ColorTier::Ansi16`], never [`ColorTier::Mono`] and never a higher tier.
//! [`ColorTier::Mono`] requires positive evidence — currently only the
//! presence of `NO_COLOR`. Guessing is forbidden: every function below is
//! total over its inputs and unknown values fall through to the baseline.

/// Colour capability tier driving style degradation.
///
/// Resolution always degrades downward: truecolor → 256-colour → 16-colour
/// → mono. [`ColorTier::Mono`] means "no colour at all" and is only ever
/// selected on positive evidence (`NO_COLOR` present); the absence of
/// evidence selects [`ColorTier::Ansi16`], not mono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorTier {
    /// 24-bit truecolor (`COLORTERM=truecolor` or equivalent evidence).
    Truecolor,
    /// 256-colour palette (`TERM` containing `256color`, or equivalent).
    Palette256,
    /// 16-colour palette. The conservative baseline: selected whenever no
    /// positive evidence for any other tier exists.
    Ansi16,
    /// No colour. Selected only on positive evidence (`NO_COLOR` present).
    Mono,
}

impl ColorTier {
    /// Returns the conservative baseline used until detection reports
    /// positive evidence for another tier.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::ColorTier;
    ///
    /// assert_eq!(ColorTier::baseline(), ColorTier::Ansi16);
    /// ```
    #[must_use]
    pub const fn baseline() -> Self {
        Self::Ansi16
    }

    /// Returns `true` for [`ColorTier::Mono`], the only tier in which colour
    /// output must be suppressed entirely.
    #[must_use]
    pub const fn is_mono(self) -> bool {
        matches!(self, Self::Mono)
    }
}

/// Per-attribute support flags travelling with a [`ColorProfile`].
///
/// Plain data: constructing a value claims nothing about the real terminal.
/// Use [`AttrSupport::for_tier`] for the conservative tier-derived defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct AttrSupport {
    /// Whether bold rendering is available.
    pub bold: bool,
    /// Whether dim rendering is available.
    pub dim: bool,
    /// Whether italic rendering is available.
    pub italic: bool,
    /// Whether underline rendering is available.
    pub underline: bool,
    /// Whether reverse-video rendering is available. Reverse video is
    /// structural (selection/focus indication), not decorative, so it is the
    /// only attribute retained under `NO_COLOR`.
    pub reverse: bool,
    /// Whether strikethrough rendering is available.
    pub strikethrough: bool,
}

impl AttrSupport {
    /// Returns support for no attributes at all.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
            strikethrough: false,
        }
    }

    /// Returns support for every attribute.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            bold: true,
            dim: true,
            italic: true,
            underline: true,
            reverse: true,
            strikethrough: true,
        }
    }

    /// Returns the conservative tier-derived defaults.
    ///
    /// Total over all tiers: truecolor and 256-colour claim the full set;
    /// 16-colour drops italic (historically the least reliable attribute on
    /// 16-colour terminals); mono keeps only reverse video, which is
    /// structural rather than decorative. The match is exhaustive over the
    /// current tiers, so adding a tier is a compile error here until its
    /// defaults are chosen — totality enforced by the compiler, never a
    /// silent optimistic guess.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{AttrSupport, ColorTier};
    ///
    /// assert_eq!(AttrSupport::for_tier(ColorTier::Truecolor), AttrSupport::all());
    /// let mono = AttrSupport::for_tier(ColorTier::Mono);
    /// assert!(mono.reverse);
    /// assert!(!mono.bold);
    /// ```
    #[must_use]
    pub const fn for_tier(tier: ColorTier) -> Self {
        match tier {
            ColorTier::Truecolor | ColorTier::Palette256 => Self::all(),
            ColorTier::Ansi16 => Self {
                bold: true,
                dim: true,
                italic: false,
                underline: true,
                reverse: true,
                strikethrough: true,
            },
            ColorTier::Mono => Self {
                bold: false,
                dim: false,
                italic: false,
                underline: false,
                reverse: true,
                strikethrough: false,
            },
        }
    }

    /// Returns this value with decorative attributes stripped, retaining only
    /// structural reverse video.
    ///
    /// This is the `NO_COLOR` policy helper (RFC-3 §5 Jilva policy):
    /// `NO_COLOR` present (any value, including empty) suppresses decoration
    /// while keeping reverse video for selection/focus structure.
    #[must_use]
    pub const fn stripped_for_no_color(self) -> Self {
        Self {
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: self.reverse,
            strikethrough: false,
        }
    }
}

/// Hint about the terminal background brightness.
///
/// [`BackgroundHint::Unknown`] means "no auto-switch": consumers keep the
/// application theme instead of guessing (the rejected alternative of
/// guessing from absence of evidence is recorded in RFC-3 Alt-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BackgroundHint {
    /// Dark background reported (e.g. via `COLORFGBG`).
    Dark,
    /// Light background reported (e.g. via `COLORFGBG`).
    Light,
    /// No evidence. Consumers MUST NOT switch themes on this value.
    Unknown,
}

/// Colour domain of a capability snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ColorProfile {
    /// The detected colour tier.
    pub color: ColorTier,
    /// Per-attribute support flags.
    pub attrs: AttrSupport,
    /// Background-brightness hint.
    pub background: BackgroundHint,
}

impl ColorProfile {
    /// Builds a colour profile from its three domain values.
    #[must_use]
    pub const fn new(color: ColorTier, attrs: AttrSupport, background: BackgroundHint) -> Self {
        Self {
            color,
            attrs,
            background,
        }
    }

    /// Returns the conservative baseline: 16 colours, tier-derived
    /// attributes, unknown background.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{BackgroundHint, ColorProfile, ColorTier};
    ///
    /// let profile = ColorProfile::ansi16_baseline();
    /// assert_eq!(profile.color, ColorTier::Ansi16);
    /// assert_eq!(profile.background, BackgroundHint::Unknown);
    /// ```
    #[must_use]
    pub const fn ansi16_baseline() -> Self {
        Self {
            color: ColorTier::Ansi16,
            attrs: AttrSupport::for_tier(ColorTier::Ansi16),
            background: BackgroundHint::Unknown,
        }
    }

    /// Returns the `NO_COLOR` profile: mono tier with decorative attributes
    /// stripped (structural reverse video retained) and unknown background.
    #[must_use]
    pub const fn mono_no_color() -> Self {
        Self {
            color: ColorTier::Mono,
            attrs: AttrSupport::for_tier(ColorTier::Mono),
            background: BackgroundHint::Unknown,
        }
    }
}

/// Key-encoding source recorded on every decoded key event.
///
/// The encoding tells the loop which degradation ladder to apply: legacy
/// byte encodings cannot distinguish several key combinations (e.g. `Ctrl+I`
/// from `Tab`, `Ctrl+M` from `Enter`). Platform input APIs may report key kinds
/// even without negotiated enhanced identity. See
/// `jilva-core/src/events.rs` for the documented degradation rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyEncoding {
    /// Unenhanced key identity (conservative default), including legacy byte
    /// streams and baseline platform APIs. Event kinds are preserved separately.
    Legacy,
    /// Enhanced encoding (e.g. Kitty keyboard protocol) with unambiguous
    /// key identity, modifiers, and event kinds.
    Enhanced,
}

/// Input domain of a capability snapshot.
///
/// Phase 2 records successfully established session modes, starting from
/// conservative defaults. Color support implies no input support. Changing
/// profile data does not reconfigure a terminal; input mode changes require
/// a fresh session until an explicit reconfiguration protocol exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct InputProfile {
    /// Key encoding the backend offers.
    pub key_encoding: KeyEncoding,
    /// Whether mouse reporting is offered. Absence MUST yield no mouse
    /// events, never synthetic ones.
    pub mouse: bool,
    /// Whether bracketed paste is offered.
    pub bracketed_paste: bool,
    /// Whether terminal focus gained/lost reports are enabled. This says
    /// nothing about application widget focus.
    pub focus_events: bool,
}

impl InputProfile {
    /// Builds an input profile from its domain values.
    #[must_use]
    pub const fn new(key_encoding: KeyEncoding, mouse: bool, bracketed_paste: bool) -> Self {
        Self {
            key_encoding,
            mouse,
            bracketed_paste,
            focus_events: false,
        }
    }

    /// Sets whether terminal focus reports are enabled.
    ///
    /// Backends must enable the matching terminal mode before advertising
    /// this flag. Changing a snapshot alone does not reconfigure input.
    #[must_use]
    pub const fn with_focus_events(mut self, enabled: bool) -> Self {
        self.focus_events = enabled;
        self
    }

    /// Returns the conservative baseline: legacy encoding, mouse off,
    /// bracketed paste off.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{InputProfile, KeyEncoding};
    ///
    /// let profile = InputProfile::conservative();
    /// assert_eq!(profile.key_encoding, KeyEncoding::Legacy);
    /// assert!(!profile.mouse);
    /// ```
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            key_encoding: KeyEncoding::Legacy,
            mouse: false,
            bracketed_paste: false,
            focus_events: false,
        }
    }
}

/// Screen domain of a capability snapshot.
///
/// No alternate screen or resize reporting is assumed before setup. A
/// successful query establishes geometry; consumed resize events update
/// geometry and the containing snapshot's screen version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ScreenProfile {
    /// Whether the alternate screen is available.
    pub alternate_screen: bool,
    /// Whether the backend delivers resize notifications (otherwise the
    /// loop must poll size).
    pub resize_reports: bool,
    /// Last observed terminal dimensions in cells, or `None` before a
    /// successful size query. Observed dimensions must both be non-zero.
    pub geometry: Option<(u16, u16)>,
}

impl ScreenProfile {
    /// Builds a screen profile from its domain values.
    #[must_use]
    pub const fn new(alternate_screen: bool, resize_reports: bool) -> Self {
        Self {
            alternate_screen,
            resize_reports,
            geometry: None,
        }
    }

    /// Records observed dimensions, rejecting a zero width or height.
    #[must_use]
    pub const fn with_size(mut self, cols: u16, rows: u16) -> Option<Self> {
        if cols == 0 || rows == 0 {
            return None;
        }
        self.geometry = Some((cols, rows));
        Some(self)
    }

    /// Returns the conservative baseline: no alternate screen, no resize
    /// reports.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::ScreenProfile;
    ///
    /// let profile = ScreenProfile::conservative();
    /// assert!(!profile.alternate_screen);
    /// assert!(!profile.resize_reports);
    /// ```
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            alternate_screen: false,
            resize_reports: false,
            geometry: None,
        }
    }
}

/// Aggregate capability snapshot travelling terminal → loop and loop → frame.
///
/// Consumption rule (normative, RFC-3 P1): every consumer takes its own
/// domain struct ([`ColorProfile`] for style, [`InputProfile`] for input,
/// [`ScreenProfile`] for renderer/lifecycle) — NEVER this aggregate — except
/// at the two seams where the aggregate travels.
///
/// The `snapshot_id` identifies which detection run produced this snapshot.
/// The per-domain counters (`color_version`, `input_version`,
/// `screen_version`) start at 1 for a fresh detection; a refresh bumps (by
/// exactly one, saturating) only the domains whose values changed, so the
/// loop can tell which domains moved without comparing every field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TerminalCapabilities {
    /// Colour domain.
    pub color: ColorProfile,
    /// Input domain.
    pub input: InputProfile,
    /// Screen domain.
    pub screen: ScreenProfile,
    /// Whether reduced motion was requested (loop disables animation).
    pub reduced_motion: bool,
    /// Identity: which detection run this snapshot came from.
    pub snapshot_id: u64,
    /// Causality counter for the colour domain.
    pub color_version: u64,
    /// Causality counter for the input domain.
    pub input_version: u64,
    /// Causality counter for the screen domain.
    pub screen_version: u64,
}

impl TerminalCapabilities {
    /// Builds a snapshot from its domain values; all domain versions start
    /// at 1 (fresh detection, nothing superseded yet).
    #[must_use]
    pub const fn new(
        color: ColorProfile,
        input: InputProfile,
        screen: ScreenProfile,
        reduced_motion: bool,
        snapshot_id: u64,
    ) -> Self {
        Self {
            color,
            input,
            screen,
            reduced_motion,
            snapshot_id,
            color_version: 1,
            input_version: 1,
            screen_version: 1,
        }
    }

    /// Returns the conservative baseline snapshot: Ansi16 colour, legacy
    /// input, conservative screen, no reduced motion, fresh domain
    /// versions. `snapshot_id` is caller-assigned (the detector owns the
    /// identity counter).
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{ColorTier, TerminalCapabilities};
    ///
    /// let caps = TerminalCapabilities::conservative(7);
    /// assert_eq!(caps.snapshot_id, 7);
    /// assert_eq!(caps.color.color, ColorTier::Ansi16);
    /// assert_eq!((caps.color_version, caps.input_version, caps.screen_version), (1, 1, 1));
    /// ```
    #[must_use]
    pub const fn conservative(snapshot_id: u64) -> Self {
        Self {
            color: ColorProfile::ansi16_baseline(),
            input: InputProfile::conservative(),
            screen: ScreenProfile::conservative(),
            reduced_motion: false,
            snapshot_id,
            color_version: 1,
            input_version: 1,
            screen_version: 1,
        }
    }

    /// Derives a refreshed snapshot from `previous`: domains whose values
    /// differ get their version bumped by exactly one (saturating at
    /// `u64::MAX`); unchanged domains keep their counter. `snapshot_id` is
    /// caller-assigned and MUST differ from `previous.snapshot_id` whenever
    /// any domain changed (the detector owns that identity counter).
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{ColorProfile, TerminalCapabilities};
    ///
    /// let before = TerminalCapabilities::conservative(1);
    /// let after = before.refreshed(ColorProfile::mono_no_color(), before.input, before.screen, false, 2);
    /// assert_eq!(after.color_version, 2);
    /// assert_eq!(after.input_version, 1);
    /// assert_eq!(after.snapshot_id, 2);
    /// ```
    #[must_use]
    pub const fn refreshed(
        self,
        color: ColorProfile,
        input: InputProfile,
        screen: ScreenProfile,
        reduced_motion: bool,
        snapshot_id: u64,
    ) -> Self {
        let color_changed = !const_color_eq(self.color, color);
        let input_changed = !const_input_eq(self.input, input);
        let screen_changed = !const_screen_eq(self.screen, screen);
        Self {
            color,
            input,
            screen,
            reduced_motion,
            snapshot_id,
            color_version: if color_changed {
                self.color_version.saturating_add(1)
            } else {
                self.color_version
            },
            input_version: if input_changed {
                self.input_version.saturating_add(1)
            } else {
                self.input_version
            },
            screen_version: if screen_changed {
                self.screen_version.saturating_add(1)
            } else {
                self.screen_version
            },
        }
    }

    /// Applies a consumed resize report to the cached snapshot.
    ///
    /// Returns `None` for zero dimensions. A changed geometry advances
    /// only `screen_version` and the session-local `snapshot_id` (both
    /// saturating); repeated reports leave every counter unchanged. Color
    /// changes remain repaint-relevant, while this change is layout-relevant.
    /// No input mode is changed by resize.
    #[must_use]
    pub const fn resized(self, cols: u16, rows: u16) -> Option<Self> {
        let Some(screen) = self.screen.with_size(cols, rows) else {
            return None;
        };
        if const_screen_eq(self.screen, screen) {
            return Some(self);
        }
        Some(self.refreshed(
            self.color,
            self.input,
            screen,
            self.reduced_motion,
            self.snapshot_id.saturating_add(1),
        ))
    }
}

// `const` field-wise equality helpers (`PartialEq` is not `const`-callable).
const fn const_color_eq(a: ColorProfile, b: ColorProfile) -> bool {
    a.color as u8 == b.color as u8
        && a.attrs.bold == b.attrs.bold
        && a.attrs.dim == b.attrs.dim
        && a.attrs.italic == b.attrs.italic
        && a.attrs.underline == b.attrs.underline
        && a.attrs.reverse == b.attrs.reverse
        && a.attrs.strikethrough == b.attrs.strikethrough
        && a.background as u8 == b.background as u8
}

const fn const_input_eq(a: InputProfile, b: InputProfile) -> bool {
    a.key_encoding as u8 == b.key_encoding as u8
        && a.mouse == b.mouse
        && a.bracketed_paste == b.bracketed_paste
        && a.focus_events == b.focus_events
}

const fn const_screen_eq(a: ScreenProfile, b: ScreenProfile) -> bool {
    let geometry_equal = match (a.geometry, b.geometry) {
        (None, None) => true,
        (Some((a_cols, a_rows)), Some((b_cols, b_rows))) => a_cols == b_cols && a_rows == b_rows,
        _ => false,
    };
    a.alternate_screen == b.alternate_screen
        && a.resize_reports == b.resize_reports
        && geometry_equal
}

/// Reports whether `NO_COLOR` is active.
///
/// Jilva's RFC-0003 policy is presence-based: the variable being present
/// with any value, including the empty string, disables colour by default.
/// This deliberately goes beyond the [informal standard](https://no-color.org/),
/// which requires a nonempty value and addresses colour alone. Jilva also
/// strips decorative attributes while retaining structural reverse video.
/// The argument is the variable's value when present, or `None` when absent.
/// The detector records presence through `std::env::var_os`, so non-Unicode
/// values suppress color too; tests pass literals. This helper remains
/// TTY-free and platform-neutral.
///
/// # Example
///
/// ```rust
/// use jilva_core::no_color_active;
///
/// assert!(no_color_active(Some("")));
/// assert!(no_color_active(Some("0")));
/// assert!(!no_color_active(None));
/// ```
#[must_use]
pub const fn no_color_active(no_color: Option<&str>) -> bool {
    no_color.is_some()
}

/// Derives the [`ColorTier`] from injected environment values.
///
/// Pure and total: unknown, empty, or contradictory values fall through to
/// the [`ColorTier::Ansi16`] baseline. Arguments are the raw values of
/// `COLORTERM`, `TERM`, `TERM_PROGRAM`, and `NO_COLOR` (`None` = unset);
/// the caller (in `jilva-terminal`) collects them from the process
/// environment so this function never touches ambient state.
///
/// Precedence, highest first (each step is positive evidence; nothing is
/// inferred from absence):
///
/// 1. `NO_COLOR` present ⇒ [`ColorTier::Mono`].
/// 2. `COLORTERM` of `truecolor`/`24bit` (ASCII case-insensitive) or
///    `TERM` naming `truecolor`/`24bit` ⇒ [`ColorTier::Truecolor`].
/// 3. `TERM` containing `256color` (ASCII case-insensitive) ⇒
///    [`ColorTier::Palette256`].
/// 4. Anything else — including `dumb`, empty, and unset — ⇒
///    [`ColorTier::Ansi16`]. A `dumb` terminal is NOT mapped to mono: the
///    conservative default is Ansi16 unless positive evidence says
///    otherwise (contract §4.3).
///
/// `TERM_PROGRAM` is retained as an input for compatibility, but a program
/// name alone is insufficient evidence of color support: nested terminals
/// and multiplexers may inherit a name while supporting fewer features.
/// All environment-based support is inference, not a terminal probe.
///
/// # Example
///
/// ```rust
/// use jilva_core::{ColorTier, color_tier_from_env};
///
/// assert_eq!(color_tier_from_env(Some("truecolor"), None, None, None), ColorTier::Truecolor);
/// assert_eq!(color_tier_from_env(None, Some("xterm-256color"), None, None), ColorTier::Palette256);
/// assert_eq!(color_tier_from_env(None, Some("dumb"), None, None), ColorTier::Ansi16);
/// assert_eq!(color_tier_from_env(Some("truecolor"), None, None, Some("")), ColorTier::Mono);
/// ```
#[must_use]
pub fn color_tier_from_env(
    colorterm: Option<&str>,
    term: Option<&str>,
    _term_program: Option<&str>,
    no_color: Option<&str>,
) -> ColorTier {
    if no_color_active(no_color) {
        return ColorTier::Mono;
    }
    if colorterm.is_some_and(|value| eq_fold(value, "truecolor") || eq_fold(value, "24bit")) {
        return ColorTier::Truecolor;
    }
    if term.is_some_and(|term| contains_fold(term, "truecolor") || contains_fold(term, "24bit")) {
        return ColorTier::Truecolor;
    }
    if term.is_some_and(|term| contains_fold(term, "256color") || contains_fold(term, "256colours"))
    {
        return ColorTier::Palette256;
    }
    ColorTier::Ansi16
}

/// Derives the [`BackgroundHint`] from an injected `COLORFGBG` value.
///
/// `COLORFGBG` has the shape `fg;bg` (xterm/rxvt convention) where the last
/// field is the background palette index. Indices 0–6 and 8 are dark
/// backgrounds, 7 and 9–15 are light ones; anything unparseable (or unset)
/// yields [`BackgroundHint::Unknown`] — never a guess.
///
/// # Example
///
/// ```rust
/// use jilva_core::{BackgroundHint, background_from_colorfgbg};
///
/// assert_eq!(background_from_colorfgbg(Some("15;0")), BackgroundHint::Dark);
/// assert_eq!(background_from_colorfgbg(Some("0;15")), BackgroundHint::Light);
/// assert_eq!(background_from_colorfgbg(None), BackgroundHint::Unknown);
/// assert_eq!(background_from_colorfgbg(Some("bogus")), BackgroundHint::Unknown);
/// ```
#[must_use]
pub fn background_from_colorfgbg(colorfgbg: Option<&str>) -> BackgroundHint {
    let Some(value) = colorfgbg else {
        return BackgroundHint::Unknown;
    };
    let last = value.rsplit(';').next().unwrap_or("");
    let digits = last.trim();
    if digits.is_empty() {
        return BackgroundHint::Unknown;
    }
    let mut index: u32 = 0;
    for byte in digits.bytes() {
        if !byte.is_ascii_digit() {
            return BackgroundHint::Unknown;
        }
        index = index
            .saturating_mul(10)
            .saturating_add(u32::from(byte - b'0'));
    }
    match index {
        0..=6 | 8 => BackgroundHint::Dark,
        7 | 9..=15 => BackgroundHint::Light,
        _ => BackgroundHint::Unknown,
    }
}

/// ASCII case-insensitive equality (byte-wise; no Unicode tables, no alloc).
fn eq_fold(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.eq_ignore_ascii_case(&y))
}

/// ASCII case-insensitive substring search (byte-wise; no Unicode tables).
fn contains_fold(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_matrix_truecolor_evidence() {
        for colorterm in ["truecolor", "TRUECOLOR", "TrueColor", "24bit", "24BIT"] {
            assert_eq!(
                color_tier_from_env(Some(colorterm), None, None, None),
                ColorTier::Truecolor,
                "COLORTERM={colorterm:?}"
            );
        }
        for term in ["xterm-truecolor", "foot-24bit", "XTERM-TRUECOLOR-direct"] {
            assert_eq!(
                color_tier_from_env(None, Some(term), None, None),
                ColorTier::Truecolor,
                "TERM={term:?}"
            );
        }
    }

    #[test]
    fn tier_matrix_256color_evidence() {
        for term in ["xterm-256color", "screen-256color", "XTERM-256COLOR"] {
            assert_eq!(
                color_tier_from_env(None, Some(term), None, None),
                ColorTier::Palette256,
                "TERM={term:?}"
            );
        }
    }

    #[test]
    fn tier_matrix_baseline_on_weak_or_no_evidence() {
        // Unknown, empty, dumb, garbage COLORTERM: all fall to Ansi16.
        for term in [
            None,
            Some(""),
            Some("dumb"),
            Some("vt100"),
            Some("unknown-term"),
        ] {
            assert_eq!(
                color_tier_from_env(None, term, None, None),
                ColorTier::Ansi16,
                "TERM={term:?}"
            );
        }
        assert_eq!(
            color_tier_from_env(Some("yes please"), None, None, None),
            ColorTier::Ansi16
        );
        assert_eq!(
            color_tier_from_env(None, None, Some("not-a-terminal"), None),
            ColorTier::Ansi16
        );
        for program in ["WezTerm", "kitty", "vscode", "Apple_Terminal"] {
            assert_eq!(
                color_tier_from_env(None, None, Some(program), None),
                ColorTier::Ansi16,
                "a program name alone is not color evidence: {program}"
            );
        }
        // A bare "256" without the "color" marker is not evidence.
        assert_eq!(
            color_tier_from_env(None, Some("xterm-256"), None, None),
            ColorTier::Ansi16
        );
    }

    #[test]
    fn tier_matrix_no_color_wins_over_everything() {
        // Present with ANY value — including empty — forces mono.
        for no_color in [Some(""), Some("0"), Some("1"), Some("false")] {
            assert_eq!(
                color_tier_from_env(Some("truecolor"), Some("xterm-truecolor"), None, no_color),
                ColorTier::Mono,
                "NO_COLOR={no_color:?}"
            );
        }
        assert!(no_color_active(Some("")));
        assert!(!no_color_active(None));
    }

    #[test]
    fn background_hint_parsing() {
        assert_eq!(
            background_from_colorfgbg(Some("15;0")),
            BackgroundHint::Dark
        );
        assert_eq!(
            background_from_colorfgbg(Some("0;15")),
            BackgroundHint::Light
        );
        assert_eq!(background_from_colorfgbg(Some("0;8")), BackgroundHint::Dark);
        assert_eq!(
            background_from_colorfgbg(Some(" 0 ; 7 ")),
            BackgroundHint::Light
        );
        for bad in [
            None,
            Some(""),
            Some("bogus"),
            Some("15;"),
            Some(";"),
            Some("1;2;xx"),
            Some("0;256"),
        ] {
            assert_eq!(
                background_from_colorfgbg(bad),
                BackgroundHint::Unknown,
                "COLORFGBG={bad:?}"
            );
        }
    }

    #[test]
    fn attr_defaults_match_tier_policy() {
        assert_eq!(
            AttrSupport::for_tier(ColorTier::Truecolor),
            AttrSupport::all()
        );
        assert_eq!(
            AttrSupport::for_tier(ColorTier::Palette256),
            AttrSupport::all()
        );
        let ansi16 = AttrSupport::for_tier(ColorTier::Ansi16);
        assert!(!ansi16.italic);
        assert!(ansi16.bold && ansi16.reverse);
        let mono = AttrSupport::for_tier(ColorTier::Mono);
        assert!(mono.reverse);
        assert!(!mono.bold && !mono.dim && !mono.italic && !mono.underline && !mono.strikethrough);
        // NO_COLOR stripping keeps structural reverse video only.
        let stripped = AttrSupport::all().stripped_for_no_color();
        assert!(stripped.reverse);
        assert!(!stripped.bold && !stripped.underline);
        assert!(!AttrSupport::none().stripped_for_no_color().reverse);
    }

    #[test]
    fn snapshot_refresh_bumps_only_changed_domains() {
        let before = TerminalCapabilities::conservative(1);
        let same = before.refreshed(before.color, before.input, before.screen, false, 2);
        assert_eq!(
            (same.color_version, same.input_version, same.screen_version),
            (1, 1, 1)
        );
        let color_moved = before.refreshed(
            ColorProfile::mono_no_color(),
            before.input,
            before.screen,
            false,
            3,
        );
        assert_eq!(color_moved.color_version, 2);
        assert_eq!(color_moved.input_version, 1);
        assert_eq!(color_moved.screen_version, 1);
        assert_eq!(color_moved.snapshot_id, 3);
        // Saturating bump at the counter ceiling.
        let mut ceiling = before;
        ceiling.color_version = u64::MAX;
        let bumped = ceiling.refreshed(
            ColorProfile::mono_no_color(),
            ceiling.input,
            ceiling.screen,
            false,
            4,
        );
        assert_eq!(bumped.color_version, u64::MAX);
    }

    #[test]
    fn resize_updates_only_screen_causality_and_repeated_reports_are_stable() {
        let before = TerminalCapabilities::new(
            ColorProfile::mono_no_color(),
            InputProfile::new(KeyEncoding::Enhanced, true, true).with_focus_events(true),
            ScreenProfile::new(true, true).with_size(80, 24).unwrap(),
            true,
            9,
        );
        assert_eq!(before.resized(0, 24), None);
        assert_eq!(before.resized(80, 0), None);
        assert_eq!(before.resized(80, 24), Some(before));
        let after = before.resized(132, 43).unwrap();
        assert_eq!(after.screen.geometry, Some((132, 43)));
        assert_eq!(after.snapshot_id, 10);
        assert_eq!(after.screen_version, before.screen_version + 1);
        assert_eq!(after.color_version, before.color_version);
        assert_eq!(after.input_version, before.input_version);
        assert_eq!(after.color, before.color);
        assert_eq!(after.input, before.input);
        assert!(after.reduced_motion);
        assert_eq!(after.resized(132, 43), Some(after));
    }

    #[test]
    fn focus_mode_change_is_visible_in_the_input_domain_version() {
        let before = TerminalCapabilities::conservative(4);
        let after = before.refreshed(
            before.color,
            before.input.with_focus_events(true),
            before.screen,
            false,
            5,
        );
        assert_eq!(after.input_version, before.input_version + 1);
        assert_eq!(after.color_version, before.color_version);
        assert_eq!(after.screen_version, before.screen_version);
    }
}
