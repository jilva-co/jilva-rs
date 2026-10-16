//! Capability model and detection results (`docs/ARCHITECTURE.md` §3.10).
//!
//! **Phase-1 status:** only the plain-data profile exists. Startup
//! detection, caching, and the degradation policy that consumes the profile
//! are RFC detail. Capability *profiles* are data; capability *detection*
//! will live in this crate, never in `jilva-core`.

/// Detected terminal capabilities, threaded explicitly into style resolution
/// and rendering — never sniffed ad hoc mid-frame.
///
/// Plain data: constructing a profile claims nothing about the real
/// terminal. [`CapabilityProfile::basic`] is the conservative baseline used
/// until real detection lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CapabilityProfile {
    /// Whether 24-bit truecolor output is supported.
    pub truecolor: bool,
    /// Whether mouse reporting is available.
    pub mouse: bool,
    /// Whether the alternate screen is available.
    pub alternate_screen: bool,
}

impl CapabilityProfile {
    /// Returns the conservative baseline: no advanced capabilities assumed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_terminal::CapabilityProfile;
    ///
    /// let profile = CapabilityProfile::basic();
    /// assert!(!profile.truecolor);
    /// ```
    #[must_use]
    pub const fn basic() -> Self {
        Self {
            truecolor: false,
            mouse: false,
            alternate_screen: false,
        }
    }

    /// Returns a full modern-terminal profile (truecolor, mouse, alt-screen).
    ///
    /// Intended for tests describing best-case degradation behaviour, not as
    /// a claim about the host terminal.
    #[must_use]
    pub const fn full() -> Self {
        Self {
            truecolor: true,
            mouse: true,
            alternate_screen: true,
        }
    }
}

impl Default for CapabilityProfile {
    /// Returns the conservative [`CapabilityProfile::basic`] baseline.
    fn default() -> Self {
        Self::basic()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_assumes_nothing() {
        let profile = CapabilityProfile::basic();
        assert!(!profile.truecolor);
        assert!(!profile.mouse);
        assert!(!profile.alternate_screen);
        assert_eq!(CapabilityProfile::default(), profile);
    }

    #[test]
    fn full_claims_everything() {
        let profile = CapabilityProfile::full();
        assert!(profile.truecolor && profile.mouse && profile.alternate_screen);
    }
}
