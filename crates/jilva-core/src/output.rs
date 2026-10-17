//! Typed output commands: the instruction vocabulary the backend executes.
//!
//! **Phase-2 status:** `OutputCommand` is the `loop → terminal` seam payload
//! (`docs/PHASE2_BACKEND_CONTRACT.md` §3). Commands are plain data — the
//! backend executes them literally and correctly, never interpreting style.
//! Ordering is significant: `Backend::submit` buffers FIFO and `flush`
//! commits in order; cursor minimisation and diffing are Phase-3 renderer
//! concerns, not represented here.
//!
//! [`OutputCommand::PrintSegment`] carries an owned Unicode string. It
//! does not impose a scalar-per-cell model or perform grapheme segmentation.
//! [`OutputCommand::SetAttributes`] takes resolved attributes only, never
//! theme tokens or ANSI bytes. Every backend validates a complete submitted
//! batch before buffering it, so untrusted text cannot inject terminal
//! controls through directly constructed commands.

use crate::capabilities::ColorTier;
use crate::layout::Rect;
use std::io;

/// Instruction vocabulary the backend executes, in order.
///
/// Additive-only evolution: new variants may be added (RFC-0004 lists
/// run-length compression as an open question); existing variants never
/// change shape. The Phase-3 renderer needed none.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputCommand {
    /// Place the cursor at 0-based cell coordinates.
    MoveTo {
        /// Column in cells from the left edge.
        x: u16,
        /// Row in cells from the top edge.
        y: u16,
    },
    /// Write Unicode text at the cursor, without scalar-per-cell splitting.
    /// May be empty (a no-op the backend skips without error).
    /// Control characters are rejected at submission, including when this
    /// variant is constructed directly.
    PrintSegment {
        /// Unicode text to write, validated before submission is accepted.
        text: String,
    },
    /// Apply fully-resolved visual attributes from this point on.
    SetAttributes(ResolvedAttributes),
    /// Return to terminal default attributes.
    ResetAttributes,
    /// Hide the cursor.
    HideCursor,
    /// Show the cursor.
    ShowCursor,
    /// Reset a rectangle to blank (space, per `Cell::blank` ground state).
    ClearRegion {
        /// Rectangle to blank.
        rect: Rect,
    },
}

impl OutputCommand {
    /// Builds a [`OutputCommand::MoveTo`] command.
    #[must_use]
    pub const fn move_to(x: u16, y: u16) -> Self {
        Self::MoveTo { x, y }
    }

    /// Builds a [`OutputCommand::PrintSegment`] command from Unicode text.
    ///
    /// Validation is deferred to backend submission. Use
    /// [`Self::try_print_segment`] to validate untrusted text immediately.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::OutputCommand;
    ///
    /// let cmd = OutputCommand::print_segment("héllo 👩‍💻");
    /// assert!(matches!(cmd, OutputCommand::PrintSegment { .. }));
    /// ```
    #[must_use]
    pub fn print_segment(text: impl Into<String>) -> Self {
        Self::PrintSegment { text: text.into() }
    }

    /// Builds a text command after checking that it contains no controls.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidInput`] for C0, DEL, or C1 controls,
    /// including ESC, tab, newline, carriage return, and OSC/CSI introducers.
    /// Formatting and cursor movement belong in typed output commands.
    pub fn try_print_segment(text: impl Into<String>) -> io::Result<Self> {
        let command = Self::print_segment(text);
        command.validate()?;
        Ok(command)
    }

    /// Validates a command before a backend accepts it.
    ///
    /// Backends must validate every command in a batch before changing
    /// their pending output. This catches direct `PrintSegment` construction
    /// as well as the convenience constructor. It does not strip or repair
    /// input, and it does not claim to validate grapheme widths.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidInput`] for terminal control text.
    pub fn validate(&self) -> io::Result<()> {
        if let Self::PrintSegment { text } = self
            && let Some((offset, control)) = text.char_indices().find(|(_, ch)| ch.is_control())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "output text contains terminal control U+{:04X} at byte {offset}",
                    u32::from(control)
                ),
            ));
        }
        Ok(())
    }

    /// Validates text and terminal-cell geometry against one screen size.
    ///
    /// This is a bounds check, not text shaping or layout. Print widths remain
    /// the caller's responsibility. Empty rectangles do nothing at any origin.
    /// Backends check complete batches before submission and again before
    /// emission so a consumed resize cannot make queued coordinates unsafe.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidInput`] for zero screen dimensions,
    /// terminal control text, or cursor/rectangle coordinates outside the
    /// screen. Arithmetic uses wider integers and cannot wrap at `u16::MAX`.
    pub fn validate_for_size(&self, cols: u16, rows: u16) -> io::Result<()> {
        self.validate()?;
        if cols == 0 || rows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output requires a nonzero screen size",
            ));
        }
        let valid = match self {
            Self::MoveTo { x, y } => *x < cols && *y < rows,
            Self::ClearRegion { rect } if !rect.is_empty() => {
                u32::from(rect.x) + u32::from(rect.width) <= u32::from(cols)
                    && u32::from(rect.y) + u32::from(rect.height) <= u32::from(rows)
            }
            _ => true,
        };
        if !valid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output geometry exceeds the current screen",
            ));
        }
        Ok(())
    }

    /// Builds a [`OutputCommand::ClearRegion`] command.
    #[must_use]
    pub const fn clear_region(rect: Rect) -> Self {
        Self::ClearRegion { rect }
    }

    /// Returns `true` for commands that move no cursor, write no text, and
    /// change no attributes (currently only the empty print segment).
    ///
    /// Backends may skip such commands; skipping MUST be the only
    /// optimisation — order of the remaining commands is preserved exactly.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        match self {
            Self::PrintSegment { text } => text.is_empty(),
            Self::MoveTo { .. }
            | Self::SetAttributes(_)
            | Self::ResetAttributes
            | Self::HideCursor
            | Self::ShowCursor
            | Self::ClearRegion { .. } => false,
        }
    }
}

/// Fully-resolved visual attributes: the only styling the backend receives.
///
/// Plain data over core-owned colour/attr types — never style tokens, never
/// ANSI bytes. Resolution (token → attributes) is a pure core function owned
/// by the theme track; the backend applies what it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ResolvedAttributes {
    /// Foreground colour.
    pub fg: ResolvedColor,
    /// Background colour.
    pub bg: ResolvedColor,
    /// Bold.
    pub bold: bool,
    /// Dim.
    pub dim: bool,
    /// Italic.
    pub italic: bool,
    /// Underline.
    pub underline: bool,
    /// Reverse video (structural; retained under `NO_COLOR`).
    pub reverse: bool,
    /// Strikethrough.
    pub strikethrough: bool,
}

impl ResolvedAttributes {
    /// Terminal defaults: default colours, no attributes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{ResolvedAttributes, ResolvedColor};
    ///
    /// let attrs = ResolvedAttributes::plain();
    /// assert_eq!(attrs.fg, ResolvedColor::Default);
    /// assert!(!attrs.bold);
    /// ```
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            fg: ResolvedColor::Default,
            bg: ResolvedColor::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
            strikethrough: false,
        }
    }

    /// Builds attributes from a foreground/background pair, no decorations.
    #[must_use]
    pub const fn new(fg: ResolvedColor, bg: ResolvedColor) -> Self {
        Self {
            fg,
            bg,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
            strikethrough: false,
        }
    }

    /// Returns these attributes with bold set (scripting helper).
    #[must_use]
    pub const fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Returns these attributes downgraded to `tier` (see
    /// [`ResolvedColor::downgrade`]); attribute flags pass through unchanged.
    #[must_use]
    pub const fn downgraded_to(self, tier: ColorTier) -> Self {
        Self {
            fg: self.fg.downgrade(tier),
            bg: self.bg.downgrade(tier),
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            reverse: self.reverse,
            strikethrough: self.strikethrough,
        }
    }
}

/// A resolved colour value: what the backend turns into escape bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResolvedColor {
    /// Terminal default colour (the backend may emit a reset sequence).
    Default,
    /// One of the 16 ANSI colours.
    Ansi16(Ansi16Color),
    /// 256-colour palette index.
    Palette(u8),
    /// 24-bit truecolor.
    Rgb(u8, u8, u8),
}

impl ResolvedColor {
    /// Downgrades this colour to what `tier` can represent. Total over all
    /// inputs: never fails, never guesses beyond the documented rules.
    ///
    /// - `Truecolor`: identity.
    /// - `Palette256`: `Rgb` maps onto the 6×6×6 colour cube
    ///   (or the grayscale ramp when `r == g == b`); other values pass
    ///   through.
    /// - `Ansi16`: `Palette(i)` with `i < 16` maps onto the
    ///   matching ANSI colour; `Rgb` maps onto the nearest ANSI colour by
    ///   squared RGB distance over the standard 16-entry table. All other
    ///   palette indices degrade to [`ResolvedColor::Default`] rather than
    ///   a guessed approximation.
    /// - `Mono`: everything becomes [`ResolvedColor::Default`] — colour
    ///   output is suppressed entirely.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::{ColorTier, ResolvedColor};
    ///
    /// assert_eq!(
    ///     ResolvedColor::Rgb(255, 0, 0).downgrade(ColorTier::Mono),
    ///     ResolvedColor::Default
    /// );
    /// ```
    #[must_use]
    pub const fn downgrade(self, tier: ColorTier) -> Self {
        match tier {
            ColorTier::Truecolor => self,
            ColorTier::Palette256 => match self {
                Self::Rgb(r, g, b) => Self::Palette(rgb_to_palette256(r, g, b)),
                _ => self,
            },
            ColorTier::Ansi16 => match self {
                Self::Default => Self::Default,
                Self::Ansi16(_) => self,
                Self::Palette(i) => match Ansi16Color::from_index(i) {
                    Some(color) => Self::Ansi16(color),
                    None => Self::Default,
                },
                Self::Rgb(r, g, b) => Self::Ansi16(nearest_ansi16(r, g, b)),
            },
            ColorTier::Mono => Self::Default,
        }
    }
}

/// The 16 ANSI colours, in standard palette order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ansi16Color {
    /// Palette index 0.
    Black,
    /// Palette index 1.
    Red,
    /// Palette index 2.
    Green,
    /// Palette index 3.
    Yellow,
    /// Palette index 4.
    Blue,
    /// Palette index 5.
    Magenta,
    /// Palette index 6.
    Cyan,
    /// Palette index 7.
    White,
    /// Palette index 8.
    BrightBlack,
    /// Palette index 9.
    BrightRed,
    /// Palette index 10.
    BrightGreen,
    /// Palette index 11.
    BrightYellow,
    /// Palette index 12.
    BrightBlue,
    /// Palette index 13.
    BrightMagenta,
    /// Palette index 14.
    BrightCyan,
    /// Palette index 15.
    BrightWhite,
}

impl Ansi16Color {
    /// Returns the palette index (0–15) of this colour.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Ansi16Color;
    ///
    /// assert_eq!(Ansi16Color::Red.index(), 1);
    /// assert_eq!(Ansi16Color::BrightWhite.index(), 15);
    /// ```
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::Black => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Yellow => 3,
            Self::Blue => 4,
            Self::Magenta => 5,
            Self::Cyan => 6,
            Self::White => 7,
            Self::BrightBlack => 8,
            Self::BrightRed => 9,
            Self::BrightGreen => 10,
            Self::BrightYellow => 11,
            Self::BrightBlue => 12,
            Self::BrightMagenta => 13,
            Self::BrightCyan => 14,
            Self::BrightWhite => 15,
        }
    }

    /// Maps a palette index below 16 onto its ANSI colour; returns `None`
    /// for indices `>= 16` (no guessing from extended entries).
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Black),
            1 => Some(Self::Red),
            2 => Some(Self::Green),
            3 => Some(Self::Yellow),
            4 => Some(Self::Blue),
            5 => Some(Self::Magenta),
            6 => Some(Self::Cyan),
            7 => Some(Self::White),
            8 => Some(Self::BrightBlack),
            9 => Some(Self::BrightRed),
            10 => Some(Self::BrightGreen),
            11 => Some(Self::BrightYellow),
            12 => Some(Self::BrightBlue),
            13 => Some(Self::BrightMagenta),
            14 => Some(Self::BrightCyan),
            15 => Some(Self::BrightWhite),
            _ => None,
        }
    }

    /// Standard RGB triple of this colour (used for nearest-match downgrade).
    const fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Black => (0, 0, 0),
            Self::Red => (170, 0, 0),
            Self::Green => (0, 170, 0),
            Self::Yellow => (170, 85, 0),
            Self::Blue => (0, 0, 170),
            Self::Magenta => (170, 0, 170),
            Self::Cyan => (0, 170, 170),
            Self::White => (170, 170, 170),
            Self::BrightBlack => (85, 85, 85),
            Self::BrightRed => (255, 85, 85),
            Self::BrightGreen => (85, 255, 85),
            Self::BrightYellow => (255, 255, 85),
            Self::BrightBlue => (85, 85, 255),
            Self::BrightMagenta => (255, 85, 255),
            Self::BrightCyan => (85, 255, 255),
            Self::BrightWhite => (255, 255, 255),
        }
    }
}

/// Maps an RGB triple onto the 256-colour palette: grayscale ramp when the
/// channels are equal, otherwise the 6×6×6 cube (integer steps, no float).
const fn rgb_to_palette256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        return (232 + (r as u16 - 8) * 24 / 256) as u8;
    }
    (16 + (r as u16 * 6 / 256) * 36 + (g as u16 * 6 / 256) * 6 + (b as u16 * 6 / 256)) as u8
}

/// Nearest ANSI colour by squared RGB distance over the standard table.
const fn nearest_ansi16(r: u8, g: u8, b: u8) -> Ansi16Color {
    let candidates = [
        Ansi16Color::Black,
        Ansi16Color::Red,
        Ansi16Color::Green,
        Ansi16Color::Yellow,
        Ansi16Color::Blue,
        Ansi16Color::Magenta,
        Ansi16Color::Cyan,
        Ansi16Color::White,
        Ansi16Color::BrightBlack,
        Ansi16Color::BrightRed,
        Ansi16Color::BrightGreen,
        Ansi16Color::BrightYellow,
        Ansi16Color::BrightBlue,
        Ansi16Color::BrightMagenta,
        Ansi16Color::BrightCyan,
        Ansi16Color::BrightWhite,
    ];
    let mut best = candidates[0];
    let mut best_distance = u32::MAX;
    let mut i = 0;
    while i < candidates.len() {
        let (cr, cg, cb) = candidates[i].rgb();
        let distance = dist2(r, cr) + dist2(g, cg) + dist2(b, cb);
        if distance < best_distance {
            best_distance = distance;
            best = candidates[i];
        }
        i += 1;
    }
    best
}

const fn dist2(a: u8, b: u8) -> u32 {
    (a.abs_diff(b) as u32) * (a.abs_diff(b) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Rect;

    #[test]
    fn output_text_rejects_every_c0_del_and_c1_control() {
        for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
            let control = char::from_u32(codepoint).unwrap();
            let command = OutputCommand::PrintSegment {
                text: format!("safe{control}tail"),
            };
            let error = command.validate().unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("byte 4"));
            // Error context must not echo an attacker's control sequence.
            assert!(!error.to_string().chars().any(char::is_control));
            assert!(OutputCommand::try_print_segment(control.to_string()).is_err());
        }
    }

    #[test]
    fn direct_command_construction_cannot_bypass_text_validation() {
        for payload in ["\u{1b}[2J", "\u{1b}]52;c;Zm9v\u{7}", "\u{9b}2J", "one\ntwo"] {
            assert!(
                OutputCommand::PrintSegment {
                    text: payload.into()
                }
                .validate()
                .is_err()
            );
            assert!(OutputCommand::try_print_segment(payload).is_err());
        }
        for command in [
            OutputCommand::move_to(0, 0),
            OutputCommand::SetAttributes(ResolvedAttributes::plain()),
            OutputCommand::ResetAttributes,
            OutputCommand::HideCursor,
            OutputCommand::ShowCursor,
            OutputCommand::clear_region(Rect::new(0, 0, 80, 24)),
        ] {
            assert!(command.validate().is_ok());
        }
    }

    #[test]
    fn output_geometry_checks_edges_without_wrapping() {
        for command in [
            OutputCommand::move_to(79, 23),
            OutputCommand::clear_region(Rect::new(0, 0, 80, 24)),
            OutputCommand::clear_region(Rect::new(79, 23, 1, 1)),
            OutputCommand::clear_region(Rect::new(u16::MAX, u16::MAX, 0, 1)),
        ] {
            assert!(command.validate_for_size(80, 24).is_ok());
        }
        for command in [
            OutputCommand::move_to(80, 23),
            OutputCommand::move_to(79, 24),
            OutputCommand::clear_region(Rect::new(79, 23, 2, 1)),
            OutputCommand::clear_region(Rect::new(0, 23, 1, 2)),
            OutputCommand::clear_region(Rect::new(u16::MAX, 0, 2, 1)),
        ] {
            assert_eq!(
                command.validate_for_size(80, 24).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        let print = OutputCommand::print_segment("safe");
        assert!(print.validate_for_size(0, 24).is_err());
        assert!(print.validate_for_size(80, 0).is_err());
        assert!(
            OutputCommand::move_to(u16::MAX - 1, 0)
                .validate_for_size(u16::MAX, 1)
                .is_ok()
        );
    }

    #[test]
    fn cluster_payloads_travel_whole() {
        // One cluster string per segment — never one scalar per cell.
        let cluster = "👩‍💻";
        let cmd = OutputCommand::print_segment(cluster);
        match cmd {
            OutputCommand::PrintSegment { text } => assert_eq!(text, cluster),
            _ => panic!("expected PrintSegment"),
        }
        assert!(OutputCommand::print_segment("").is_noop());
        for text in ["", "e\u{301}", "👩‍💻", "🇺🇿", "日本語", "several graphemes"] {
            let command = OutputCommand::try_print_segment(text).unwrap();
            assert_eq!(command, OutputCommand::PrintSegment { text: text.into() });
        }
        assert!(!OutputCommand::HideCursor.is_noop());
        assert!(!OutputCommand::move_to(1, 1).is_noop());
    }

    #[test]
    fn downgrade_is_total_and_monotone_downward() {
        let rgb = ResolvedColor::Rgb(255, 0, 0);
        assert_eq!(rgb.downgrade(ColorTier::Truecolor), rgb);
        assert_eq!(
            rgb.downgrade(ColorTier::Palette256),
            ResolvedColor::Palette(196)
        );
        assert_eq!(
            rgb.downgrade(ColorTier::Ansi16),
            ResolvedColor::Ansi16(Ansi16Color::Red)
        );
        assert_eq!(rgb.downgrade(ColorTier::Mono), ResolvedColor::Default);
        // Grayscale lands on the ramp, not the cube: 232 + (128-8)*24/256 = 243.
        assert_eq!(
            ResolvedColor::Rgb(128, 128, 128).downgrade(ColorTier::Palette256),
            ResolvedColor::Palette(243)
        );
        // Low palette indices survive to Ansi16; extended ones do not guess.
        assert_eq!(
            ResolvedColor::Palette(1).downgrade(ColorTier::Ansi16),
            ResolvedColor::Ansi16(Ansi16Color::Red)
        );
        assert_eq!(
            ResolvedColor::Palette(200).downgrade(ColorTier::Ansi16),
            ResolvedColor::Default
        );
        assert_eq!(
            ResolvedColor::Default.downgrade(ColorTier::Ansi16),
            ResolvedColor::Default
        );
        // Attribute flags survive downgrade; colours move.
        let attrs = ResolvedAttributes {
            fg: rgb,
            bg: ResolvedColor::Default,
            bold: true,
            dim: false,
            italic: false,
            underline: false,
            reverse: true,
            strikethrough: false,
        };
        let mono = attrs.downgraded_to(ColorTier::Mono);
        assert_eq!(mono.fg, ResolvedColor::Default);
        assert!(mono.bold && mono.reverse);
    }

    #[test]
    fn ansi16_indices_round_trip() {
        for i in 0..16u8 {
            let color = Ansi16Color::from_index(i).expect("0..16 maps");
            assert_eq!(color.index(), i);
        }
        assert_eq!(Ansi16Color::from_index(16), None);
        assert_eq!(Ansi16Color::from_index(255), None);
    }
}
