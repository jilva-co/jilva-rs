//! Style token model (`docs/README.md#theming-contract`).
//!
//! Themes are immutable values with partial overrides, not global registries.
//! Resolve once per theme/colour profile, then look up semantic attributes.
//! State overlays compose before capability degradation.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::capabilities::ColorProfile;
use crate::output::{ResolvedAttributes, ResolvedColor};

/// Semantic style token referenced by components.
///
/// Fourteen canonical roles, plus the original `Selected` compatibility spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SemanticToken {
    /// Application canvas.
    Background,
    /// Raised panels and cards.
    Surface,
    /// Popups above a panel.
    Overlay,
    /// Borders and dividers.
    Border,
    /// Primary text.
    Text,
    /// Primary accent for selection, focus, and brand.
    Accent,
    /// De-emphasised secondary text.
    Muted,
    /// Destructive or erroneous states.
    Danger,
    /// Successful operations.
    Success,
    /// Warnings needing attention.
    Warning,
    /// Informational status.
    Info,
    /// Selected item fill.
    Selection,
    /// Text caret.
    Cursor,
    /// Search matches and marks.
    Highlight,
    /// Compatibility spelling of [`SemanticToken::Selection`].
    /// Resolves identically, while keeping the original `"selected"` name.
    Selected,
}

impl SemanticToken {
    /// Canonical roles in stable palette order; excludes the compatibility alias.
    pub const ALL: [Self; 14] = [
        Self::Background,
        Self::Surface,
        Self::Overlay,
        Self::Border,
        Self::Text,
        Self::Muted,
        Self::Accent,
        Self::Success,
        Self::Warning,
        Self::Danger,
        Self::Info,
        Self::Selection,
        Self::Cursor,
        Self::Highlight,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Background => 0,
            Self::Surface => 1,
            Self::Overlay => 2,
            Self::Border => 3,
            Self::Text => 4,
            Self::Muted => 5,
            Self::Accent => 6,
            Self::Success => 7,
            Self::Warning => 8,
            Self::Danger => 9,
            Self::Info => 10,
            Self::Selection | Self::Selected => 11,
            Self::Cursor => 12,
            Self::Highlight => 13,
        }
    }

    /// Returns the stable token name used in themes and snapshots.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::SemanticToken;
    ///
    /// assert_eq!(SemanticToken::Accent.as_str(), "accent");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Surface => "surface",
            Self::Overlay => "overlay",
            Self::Border => "border",
            Self::Text => "text",
            Self::Accent => "accent",
            Self::Muted => "muted",
            Self::Danger => "danger",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Selection => "selection",
            Self::Cursor => "cursor",
            Self::Highlight => "highlight",
            Self::Selected => "selected",
        }
    }
}

/// Co-present visual states. Disabled absorbs the other states.
///
/// This changes appearance only; widgets remain responsible for refusing
/// disabled input and drawing structural focus markers in monochrome.
///
/// ```rust
/// use jilva_core::StyleState;
/// let state = StyleState::new().focused(true).selected(true);
/// assert_eq!(state, StyleState::new().selected(true).focused(true));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StyleState(u8);

impl StyleState {
    /// Normal state with no overlays.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }
    /// Adds or removes the hover state.
    #[must_use]
    pub const fn hovered(self, value: bool) -> Self {
        self.with(1, value)
    }
    /// Adds or removes the pressed/active state.
    #[must_use]
    pub const fn active(self, value: bool) -> Self {
        self.with(2, value)
    }
    /// Adds or removes keyboard focus.
    #[must_use]
    pub const fn focused(self, value: bool) -> Self {
        self.with(4, value)
    }
    /// Adds or removes selection.
    #[must_use]
    pub const fn selected(self, value: bool) -> Self {
        self.with(8, value)
    }
    /// Adds or removes the disabled state.
    #[must_use]
    pub const fn disabled(self, value: bool) -> Self {
        self.with(16, value)
    }
    const fn with(self, bit: u8, value: bool) -> Self {
        Self(if value { self.0 | bit } else { self.0 & !bit })
    }
    const fn index(self) -> usize {
        if self.0 & 16 != 0 {
            16
        } else {
            self.0 as usize
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThemeData {
    name: &'static str,
    tokens: [ResolvedAttributes; 14],
    extensions: BTreeMap<&'static str, ResolvedAttributes>,
}

/// An immutable, cheaply cloned semantic palette.
///
/// Overrides replace one complete role, leaving other roles untouched.
/// Repeated overrides compose in call order (last wins). Extensions are
/// application-defined styles and are not inherited when switching themes.
/// No file parsing, environment reads, or global registry is involved.
///
/// ```rust
/// use jilva_core::{Theme, SemanticToken, ResolvedAttributes, ResolvedColor};
/// let brand = Theme::dark().with_token(
///     SemanticToken::Accent,
///     ResolvedAttributes::new(ResolvedColor::Rgb(180, 140, 255), ResolvedColor::Default),
/// );
/// assert_eq!(brand.name(), "jilva-dark");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme(Arc<ThemeData>);

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// The curated dark theme, also the default.
    #[must_use]
    pub fn dark() -> Self {
        Self::builtin(false)
    }
    /// The curated light theme. Never chosen from an unknown background hint.
    #[must_use]
    pub fn light() -> Self {
        Self::builtin(true)
    }
    /// The name used by diagnostics and galleries.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.0.name
    }
    /// Names a derived theme without registering it globally.
    #[must_use]
    pub fn named(mut self, name: &'static str) -> Self {
        Arc::make_mut(&mut self.0).name = name;
        self
    }
    /// Overrides one complete role. Unspecified roles retain the base palette.
    #[must_use]
    pub fn with_token(mut self, token: SemanticToken, attrs: ResolvedAttributes) -> Self {
        Arc::make_mut(&mut self.0).tokens[token.index()] = attrs;
        self
    }
    /// Applies a sequence of role overrides in order.
    #[must_use]
    pub fn with_overrides(
        mut self,
        overrides: impl IntoIterator<Item = (SemanticToken, ResolvedAttributes)>,
    ) -> Self {
        let data = Arc::make_mut(&mut self.0);
        for (token, attrs) in overrides {
            data.tokens[token.index()] = attrs;
        }
        self
    }
    /// Defines an application-specific role. A later definition replaces it.
    #[must_use]
    pub fn with_extension(mut self, name: &'static str, attrs: ResolvedAttributes) -> Self {
        Arc::make_mut(&mut self.0).extensions.insert(name, attrs);
        self
    }
    /// Resolves all roles and state combinations for one colour profile.
    ///
    /// Merged theme values are retained independently of this cache. Colour
    /// and attribute support affect quantization; size, input, and time do not.
    /// Lookups on the returned value allocate nothing.
    #[must_use]
    pub fn resolve(&self, profile: &ColorProfile) -> ResolvedTheme {
        let styles = |attrs| {
            std::array::from_fn(|state| {
                self.overlay(attrs, StyleState(state as u8), profile)
                    .for_profile(profile)
            })
        };
        let mut tokens = self.0.tokens;
        if profile.color.is_mono() {
            for token in [
                SemanticToken::Selection,
                SemanticToken::Cursor,
                SemanticToken::Highlight,
            ] {
                tokens[token.index()].reverse = true;
            }
        }
        ResolvedTheme(Arc::new(ResolvedThemeData {
            theme: self.clone(),
            profile: *profile,
            tokens: tokens.map(styles),
            extensions: self
                .0
                .extensions
                .iter()
                .map(|(&key, &attrs)| (key, styles(attrs)))
                .collect(),
        }))
    }

    fn overlay(
        &self,
        mut attrs: ResolvedAttributes,
        state: StyleState,
        profile: &ColorProfile,
    ) -> ResolvedAttributes {
        let role = |token: SemanticToken| self.0.tokens[token.index()];
        if state.0 & 16 != 0 {
            attrs.fg = role(SemanticToken::Muted).fg;
            attrs.bold = false;
            attrs.dim = true;
            attrs.italic = false;
            attrs.underline = false;
            attrs.reverse = false;
            attrs.strikethrough = false;
            return attrs;
        }
        if state.0 & 1 != 0 {
            attrs.bg = role(SemanticToken::Surface).bg;
        }
        if state.0 & 2 != 0 {
            attrs.bg = role(SemanticToken::Overlay).bg;
            attrs.bold = true;
        }
        if state.0 & 4 != 0 {
            attrs.underline = true;
            if profile.color.is_mono() {
                attrs.reverse = true;
            }
        }
        if state.0 & 8 != 0 {
            let selection = role(SemanticToken::Selection);
            attrs.fg = selection.fg;
            attrs.bg = selection.bg;
            if profile.color.is_mono() {
                attrs.reverse = true;
            }
        }
        attrs
    }

    fn builtin(light: bool) -> Self {
        let rgb = |(r, g, b)| ResolvedColor::Rgb(r, g, b);
        let (
            canvas,
            surface,
            overlay,
            text,
            muted,
            border,
            accent,
            success,
            warning,
            danger,
            info,
            selection,
            highlight,
        ) = if light {
            (
                (248, 250, 252),
                (241, 245, 249),
                (226, 232, 240),
                (23, 37, 84),
                (71, 85, 105),
                (100, 116, 139),
                (67, 56, 202),
                (4, 120, 87),
                (146, 64, 14),
                (159, 18, 57),
                (7, 89, 133),
                (219, 234, 254),
                (254, 240, 138),
            )
        } else {
            (
                (17, 24, 39),
                (30, 41, 59),
                (51, 65, 85),
                (226, 232, 240),
                (148, 163, 184),
                (100, 116, 139),
                (129, 140, 248),
                (110, 231, 183),
                (252, 211, 77),
                (253, 164, 175),
                (125, 211, 252),
                (51, 65, 85),
                (76, 61, 24),
            )
        };
        let ink = |fg| ResolvedAttributes::new(rgb(fg), rgb(canvas));
        let fill = |fg, bg| ResolvedAttributes::new(rgb(fg), rgb(bg));
        Self(Arc::new(ThemeData {
            name: if light { "jilva-light" } else { "jilva-dark" },
            tokens: [
                fill(text, canvas),
                fill(text, surface),
                fill(text, overlay),
                ink(border),
                ink(text),
                ink(muted),
                ink(accent),
                ink(success),
                ink(warning),
                ink(danger),
                ink(info),
                fill(text, selection),
                fill(canvas, text),
                fill(text, highlight),
            ],
            extensions: BTreeMap::new(),
        }))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ResolvedThemeData {
    theme: Theme,
    profile: ColorProfile,
    tokens: [[ResolvedAttributes; 17]; 14],
    extensions: BTreeMap<&'static str, [ResolvedAttributes; 17]>,
}

/// A shared immutable theme cache for one colour/attribute profile.
///
/// Cloning shares the cache. Focus changes and resizes do not rebuild it.
/// Unsupported decorations are removed; mono retains only supported reverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTheme(Arc<ResolvedThemeData>);

impl ResolvedTheme {
    /// Name of the original theme.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.0.theme.name()
    }
    /// The profile against which these attributes were resolved.
    #[must_use]
    pub fn profile(&self) -> &ColorProfile {
        &self.0.profile
    }
    /// The merged, capability-independent source palette.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.0.theme
    }
    /// Normal attributes of a semantic role.
    #[must_use]
    pub fn style(&self, token: SemanticToken) -> ResolvedAttributes {
        self.style_with(token, StyleState::new())
    }
    /// Attributes with co-present state overlays applied.
    #[must_use]
    pub fn style_with(&self, token: SemanticToken, state: StyleState) -> ResolvedAttributes {
        self.0.tokens[token.index()][state.index()]
    }
    /// Looks up an extension, falling back to Text with an explicit diagnostic.
    ///
    /// Missing keys never panic or log from the core. Applications may use
    /// `missing_extension` to surface a diagnostic in their own tooling.
    #[must_use]
    pub fn extension(&self, name: &str, state: StyleState) -> ResolvedStyle {
        let found = self.0.extensions.get(name);
        ResolvedStyle {
            attributes: found.map_or_else(
                || self.style_with(SemanticToken::Text, state),
                |styles| styles[state.index()],
            ),
            missing_extension: found.is_none(),
        }
    }
    /// Retargets quantization, sharing the cache when the profile is unchanged.
    #[must_use]
    pub fn for_profile(&self, profile: &ColorProfile) -> Self {
        if self.profile() == profile {
            self.clone()
        } else {
            self.0.theme.resolve(profile)
        }
    }
}

/// Resolved extension attributes with a missing-role diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedStyle {
    /// Capability-effective attributes, always usable even on a missing key.
    pub attributes: ResolvedAttributes,
    /// Whether fallback to Text was needed.
    pub missing_extension: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::ColorTier;
    use crate::{AttrSupport, BackgroundHint, Context, TerminalCapabilities};
    use std::time::Duration;

    fn profile(tier: ColorTier) -> ColorProfile {
        ColorProfile::new(tier, AttrSupport::for_tier(tier), BackgroundHint::Unknown)
    }

    #[test]
    fn token_names_are_stable() {
        assert_eq!(SemanticToken::Accent.as_str(), "accent");
        assert_eq!(SemanticToken::Muted.as_str(), "muted");
        assert_eq!(SemanticToken::Danger.as_str(), "danger");
        assert_eq!(SemanticToken::Selected.as_str(), "selected");
    }

    #[test]
    fn canonical_roles_and_the_original_selected_spelling_coexist() {
        let names: std::collections::BTreeSet<_> =
            SemanticToken::ALL.map(SemanticToken::as_str).into();
        assert_eq!(names.len(), 14);
        assert!(names.contains("selection"));
        let theme = Theme::dark().resolve(&profile(ColorTier::Truecolor));
        for state in 0..32 {
            assert_eq!(
                theme.style_with(SemanticToken::Selected, StyleState(state)),
                theme.style_with(SemanticToken::Selection, StyleState(state)),
            );
        }
    }

    #[test]
    fn overrides_are_isolated_composable_and_do_not_mutate_the_base() {
        let base = Theme::dark();
        let old = base.resolve(&profile(ColorTier::Truecolor));
        let red = ResolvedAttributes::new(ResolvedColor::Rgb(240, 20, 30), ResolvedColor::Default);
        let blue = ResolvedAttributes::new(ResolvedColor::Rgb(20, 30, 240), ResolvedColor::Default);
        let custom = base
            .clone()
            .with_token(SemanticToken::Accent, red)
            .with_overrides([(SemanticToken::Accent, blue)])
            .named("brand");
        let new = custom.resolve(&profile(ColorTier::Truecolor));
        for token in SemanticToken::ALL {
            assert_eq!(
                new.style(token),
                if token == SemanticToken::Accent {
                    blue
                } else {
                    old.style(token)
                }
            );
        }
        assert_eq!(base.name(), "jilva-dark");
        assert_eq!(custom.name(), "brand");
        assert_ne!(
            base.resolve(&profile(ColorTier::Truecolor))
                .style(SemanticToken::Accent),
            blue
        );
    }

    #[test]
    fn overlay_precedence_and_disabled_absorption_are_explicit() {
        let profile = profile(ColorTier::Truecolor);
        for theme in [Theme::dark(), Theme::light()] {
            let resolved = theme.resolve(&profile);
            let selected =
                resolved.style_with(SemanticToken::Text, StyleState::new().selected(true));
            let both = resolved.style_with(
                SemanticToken::Text,
                StyleState::new().selected(true).focused(true),
            );
            assert_eq!(both.fg, selected.fg);
            assert_eq!(both.bg, selected.bg);
            assert!(!selected.underline);
            assert!(both.underline);
            let stacked = resolved.style_with(
                SemanticToken::Text,
                StyleState::new()
                    .hovered(true)
                    .active(true)
                    .focused(true)
                    .selected(true),
            );
            assert_eq!((stacked.fg, stacked.bg), (selected.fg, selected.bg));
            assert!(stacked.bold && stacked.underline);
            let hovered = resolved.style_with(SemanticToken::Text, StyleState::new().hovered(true));
            let active = resolved.style_with(
                SemanticToken::Text,
                StyleState::new().hovered(true).active(true),
            );
            assert_eq!(hovered.bg, resolved.style(SemanticToken::Surface).bg);
            assert_eq!(active.bg, resolved.style(SemanticToken::Overlay).bg);
            for token in SemanticToken::ALL {
                let disabled = resolved.style_with(token, StyleState::new().disabled(true));
                assert_eq!(disabled.fg, resolved.style(SemanticToken::Muted).fg);
                assert!(disabled.dim);
                for state in 16..32 {
                    assert_eq!(resolved.style_with(token, StyleState(state)), disabled);
                }
            }
        }
    }

    #[test]
    fn every_role_and_state_obeys_the_terminal_profile() {
        for tier in [
            ColorTier::Truecolor,
            ColorTier::Palette256,
            ColorTier::Ansi16,
            ColorTier::Mono,
        ] {
            let profile = profile(tier);
            for theme in [Theme::dark(), Theme::light()] {
                let resolved = theme.resolve(&profile);
                for token in SemanticToken::ALL {
                    for state in 0..32 {
                        let attrs = resolved.style_with(token, StyleState(state));
                        assert_eq!(attrs.for_profile(&profile), attrs);
                        for color in [attrs.fg, attrs.bg] {
                            match tier {
                                ColorTier::Truecolor => assert!(matches!(
                                    color,
                                    ResolvedColor::Rgb(..) | ResolvedColor::Default
                                )),
                                ColorTier::Palette256 => assert!(matches!(
                                    color,
                                    ResolvedColor::Palette(_) | ResolvedColor::Default
                                )),
                                ColorTier::Ansi16 => assert!(matches!(
                                    color,
                                    ResolvedColor::Ansi16(_) | ResolvedColor::Default
                                )),
                                ColorTier::Mono => assert_eq!(color, ResolvedColor::Default),
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn mono_keeps_structure_and_cannot_be_overridden_into_decorations() {
        let profile =
            ColorProfile::new(ColorTier::Mono, AttrSupport::all(), BackgroundHint::Unknown);
        let resolved = Theme::dark().resolve(&profile);
        for token in SemanticToken::ALL {
            for state in 0..32 {
                let attrs = resolved.style_with(token, StyleState(state));
                assert!(
                    !(attrs.bold
                        || attrs.dim
                        || attrs.italic
                        || attrs.underline
                        || attrs.strikethrough)
                );
            }
        }
        assert!(resolved.style(SemanticToken::Selection).reverse);
        assert!(resolved.style(SemanticToken::Cursor).reverse);
        assert!(
            resolved
                .style_with(SemanticToken::Text, StyleState::new().focused(true))
                .reverse
        );
        assert!(
            !resolved
                .style_with(
                    SemanticToken::Text,
                    StyleState::new().focused(true).disabled(true)
                )
                .reverse
        );
        let unsupported = resolved.for_profile(&ColorProfile::new(
            ColorTier::Mono,
            AttrSupport::none(),
            BackgroundHint::Unknown,
        ));
        assert!(!unsupported.style(SemanticToken::Selection).reverse);
    }

    #[test]
    fn missing_extensions_have_an_observable_readable_fallback() {
        let red = ResolvedAttributes::plain().with_bold();
        let theme = Theme::dark().with_extension("diff-add", red);
        let resolved = theme.resolve(&profile(ColorTier::Truecolor));
        let found = resolved.extension("diff-add", StyleState::new());
        assert_eq!(found.attributes, red);
        assert!(!found.missing_extension);
        let missing = resolved.extension("unknown", StyleState::new().focused(true));
        assert!(missing.missing_extension);
        assert_eq!(
            missing.attributes,
            resolved.style_with(SemanticToken::Text, StyleState::new().focused(true))
        );
        assert!(
            Theme::light()
                .resolve(&profile(ColorTier::Truecolor))
                .extension("diff-add", StyleState::new())
                .missing_extension
        );
    }

    #[test]
    fn profile_cache_is_shared_until_colour_or_attribute_support_changes() {
        let profile = profile(ColorTier::Truecolor);
        let resolved = Theme::dark().resolve(&profile);
        assert!(Arc::ptr_eq(&resolved.0, &resolved.for_profile(&profile).0));
        let mut restricted = profile;
        restricted.attrs.underline = false;
        let changed = resolved.for_profile(&restricted);
        assert!(!Arc::ptr_eq(&resolved.0, &changed.0));
        assert!(
            !changed
                .style_with(SemanticToken::Text, StyleState::new().focused(true))
                .underline
        );
        let cx = Context::new(
            (80, 24),
            TerminalCapabilities::conservative(1),
            Duration::ZERO,
        );
        let focused = cx.with_focused(true);
        assert!(Arc::ptr_eq(&cx.theme().0, &focused.theme().0));
        let same = cx.clone().with_theme(Theme::dark());
        assert_eq!(cx.theme_version(), same.theme_version());
        assert!(Arc::ptr_eq(&cx.theme().0, &same.theme().0));
        let light = cx.clone().with_theme(Theme::light());
        assert_eq!(light.theme_version(), cx.theme_version() + 1);
        assert_eq!(cx.theme().name(), "jilva-dark");
    }

    #[test]
    fn curated_text_roles_have_legible_truecolor_contrast() {
        fn luminance(color: ResolvedColor) -> f64 {
            let (r, g, b) = color.to_rgb((0, 0, 0));
            let channel = |v: u8| {
                let x = f64::from(v) / 255.0;
                if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
        }
        for theme in [Theme::dark(), Theme::light()] {
            let resolved = theme.resolve(&profile(ColorTier::Truecolor));
            for token in [
                SemanticToken::Text,
                SemanticToken::Muted,
                SemanticToken::Accent,
                SemanticToken::Success,
                SemanticToken::Warning,
                SemanticToken::Danger,
                SemanticToken::Info,
            ] {
                let attrs = resolved.style(token);
                let fg = luminance(attrs.fg);
                let bg = luminance(attrs.bg);
                let ratio = (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05);
                assert!(ratio >= 4.5, "{} {token:?}: contrast {ratio}", theme.name());
            }
        }
    }

    #[test]
    fn theme_role_state_matrix_regression_snapshots() {
        // FNV-1a fingerprints of the complete readable Debug attribute matrix:
        // 14 roles x 17 effective states, dark/light x all four terminal tiers.
        // Kept inline to avoid hundreds of near-identical fixture lines.
        let mut actual = Vec::new();
        for theme in [Theme::dark(), Theme::light()] {
            for tier in [
                ColorTier::Truecolor,
                ColorTier::Palette256,
                ColorTier::Ansi16,
                ColorTier::Mono,
            ] {
                let resolved = theme.resolve(&profile(tier));
                let mut hash = 0xcbf29ce484222325u64;
                for token in SemanticToken::ALL {
                    for state in 0..17 {
                        let sample = format!(
                            "{}:{:?};",
                            token.as_str(),
                            resolved.style_with(token, StyleState(state))
                        );
                        for byte in sample.bytes() {
                            hash ^= u64::from(byte);
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                    }
                }
                actual.push(hash);
            }
        }
        assert_eq!(
            actual,
            [
                592445557573443485,
                16494632165578577568,
                11392774627701614884,
                1622133808147984732,
                9743986973981258536,
                1507084662043845423,
                14608517790598914415,
                1622133808147984732,
            ]
        );
    }
}
