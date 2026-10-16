//! Style token model (`docs/ARCHITECTURE.md` §3.8).
//!
//! **Phase-1 status:** only the token vocabulary and the capability-tier
//! vocabulary exist. The theme schema, token-to-attribute resolution, the
//! curated default theme, and per-tier snapshot tests are RFC-3 detail.
//! Components reference [`SemanticToken`]s here, never raw ANSI values.

/// Terminal colour capability tier driving style degradation.
///
/// Resolution always degrades downward: truecolor → 256-colour → 16-colour.
/// Detection itself lives in `jilva-terminal`; this enum is the plain-data
/// profile that detection reports and style resolution consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorDepth {
    /// 24-bit truecolor.
    TrueColor,
    /// 256-colour palette.
    Ansi256,
    /// 16-colour palette.
    Ansi16,
}

/// Semantic style token referenced by components.
///
/// Names are illustrative of intent (`accent`, `muted`, …), never colours.
/// Concrete attributes for each token at each [`ColorDepth`] are RFC-3
/// detail; this enum fixes *where* tokens live so that decision has a home.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SemanticToken {
    /// Primary accent for selection, focus, and brand.
    Accent,
    /// De-emphasised secondary text.
    Muted,
    /// Destructive or erroneous states.
    Danger,
    /// Currently selected item.
    Selected,
}

impl SemanticToken {
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
            Self::Accent => "accent",
            Self::Muted => "muted",
            Self::Danger => "danger",
            Self::Selected => "selected",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_names_are_stable() {
        assert_eq!(SemanticToken::Accent.as_str(), "accent");
        assert_eq!(SemanticToken::Muted.as_str(), "muted");
        assert_eq!(SemanticToken::Danger.as_str(), "danger");
        assert_eq!(SemanticToken::Selected.as_str(), "selected");
    }
}
