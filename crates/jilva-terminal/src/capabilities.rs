//! Conservative capability detection, without escape-sequence probing.
//!
//! Environment collection and output-handle observation live here. Core
//! owns the profile data and pure string parsers. Environment hints are
//! inferences, not terminal responses; [`DetectionReport`] makes their
//! source explicit. Native backend mode options are separate, explicit
//! input/screen overrides and must be advertised only after successful setup.
//!
//! Detection does not enable modes. A backend caches one snapshot at entry;
//! `capabilities()` does not repeat detection. Snapshot identities belong to
//! a backend instance, not a process-global counter. Dynamic input-mode
//! reconfiguration and frame-boundary application are deferred.

use std::io::{self, IsTerminal};

use jilva_core::{
    AttrSupport, BackgroundHint, ColorProfile, ColorTier, InputProfile, ScreenProfile,
    TerminalCapabilities, background_from_colorfgbg, color_tier_from_env,
};

/// The relevant environment values, injectable without ambient mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EnvSnapshot {
    /// Value of `COLORTERM` when readable as Unicode.
    pub colorterm: Option<String>,
    /// Value of `TERM` when readable as Unicode.
    pub term: Option<String>,
    /// Value of `TERM_PROGRAM`; a program name alone proves no color tier.
    pub term_program: Option<String>,
    /// Value of `COLORFGBG` when readable as Unicode.
    pub colorfgbg: Option<String>,
    /// Any presence of `NO_COLOR`, including an empty or non-Unicode value.
    /// This is Jilva's stronger RFC-0003 policy, not the informal standard's
    /// nonempty-only condition.
    pub no_color_present: bool,
}

impl EnvSnapshot {
    /// Builds a snapshot from explicit values for tests and embedded hosts.
    #[must_use]
    pub fn new(
        colorterm: Option<String>,
        term: Option<String>,
        term_program: Option<String>,
        colorfgbg: Option<String>,
        no_color_present: bool,
    ) -> Self {
        Self {
            colorterm,
            term,
            term_program,
            colorfgbg,
            no_color_present,
        }
    }

    /// Returns a snapshot with no environment evidence.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(None, None, None, None, false)
    }

    /// Collects the live process environment without touching terminal modes.
    /// Unreadable color hints are ignored. Non-Unicode `NO_COLOR` still
    /// counts as present, because suppression must not depend on decoding.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            colorterm: std::env::var("COLORTERM").ok(),
            term: std::env::var("TERM").ok(),
            term_program: std::env::var("TERM_PROGRAM").ok(),
            colorfgbg: std::env::var("COLORFGBG").ok(),
            no_color_present: std::env::var_os("NO_COLOR").is_some(),
        }
    }

    /// Maps injected values under an explicit assumption of terminal output.
    /// Real backends must pass their actual output-handle observation to
    /// [`detect_from_env`] instead.
    #[must_use]
    pub fn detect_assuming_tty(&self) -> TerminalCapabilities {
        detect_from_env(self, true)
    }
}

/// Why a detected domain value was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilitySource {
    /// No stronger evidence; conservative profile policy was used.
    ConservativeDefault,
    /// A readable environment hint suggests support; no probe confirmed it.
    EnvironmentInference,
    /// `NO_COLOR` requested Jilva's explicit environment suppression policy.
    EnvironmentPolicy,
    /// A caller explicitly selected a profile for this instance.
    UserOverride,
    /// The output-handle observation requires control-free redirected output.
    RedirectedOutput,
}

/// Detected data together with the observed fact and inference sources.
///
/// Input and screen start conservative; this report does not claim terminal
/// modes were enabled. It is not a security attestation of environment values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DetectionReport {
    /// The canonical core-owned profile snapshot.
    pub capabilities: TerminalCapabilities,
    /// The output-handle observation supplied by the caller.
    pub output_is_tty: bool,
    /// Source of the color tier and attribute policy.
    pub color_source: CapabilitySource,
    /// Source of the background hint, which remains an inference.
    pub background_source: CapabilitySource,
}

/// Detects live environment hints and observes whether stdout is a terminal.
///
/// This enables no modes, emits no bytes, and cannot hang on a terminal
/// response. Backends owning another output handle must use its observation
/// with [`detect_from_env`] instead.
#[must_use]
pub fn detect() -> TerminalCapabilities {
    detect_from_env(&EnvSnapshot::from_process(), io::stdout().is_terminal())
}

/// Maps injected environment values and an observed output-handle TTY flag.
///
/// No ambient reads, I/O, or global state occur here. Each fresh independent
/// detection starts at snapshot identity 1 and domain versions 1; a backend
/// stamps its own monotonically increasing identity when caching the value.
/// IDs from separate backend instances are not globally comparable.
#[must_use]
pub fn detect_from_env(env: &EnvSnapshot, output_is_tty: bool) -> TerminalCapabilities {
    detect_report_from_env(env, output_is_tty).capabilities
}

/// Detects injected values and returns the source of each color decision.
#[must_use]
pub fn detect_report_from_env(env: &EnvSnapshot, output_is_tty: bool) -> DetectionReport {
    detect_with_overrides(env, output_is_tty, None)
}

/// Detects injected values with an optional explicit per-instance color profile.
///
/// Precedence: redirected output, explicit override, `NO_COLOR`, environment
/// hints, conservative defaults. Overrides can deliberately supersede
/// `NO_COLOR` on a terminal. Redirected output always has monochrome color,
/// no attributes (including reverse), and unknown background. This profile
/// alone is not permission to emit cursor/screen control sequences: the real
/// backend also rejects acquiring an interactive session on redirected I/O.
///
/// Unknown terminal hints fall back to Ansi16. A terminal program name alone
/// cannot establish truecolor support. No input/screen capability is inferred
/// from a color tier. An override changes data, never active terminal modes.
#[must_use]
pub fn detect_with_overrides(
    env: &EnvSnapshot,
    output_is_tty: bool,
    color_override: Option<ColorProfile>,
) -> DetectionReport {
    let (color, color_source, background_source) = if !output_is_tty {
        (
            ColorProfile::new(
                ColorTier::Mono,
                AttrSupport::none(),
                BackgroundHint::Unknown,
            ),
            CapabilitySource::RedirectedOutput,
            CapabilitySource::RedirectedOutput,
        )
    } else if let Some(color) = color_override {
        (
            color,
            CapabilitySource::UserOverride,
            CapabilitySource::UserOverride,
        )
    } else {
        let tier = color_tier_from_env(
            env.colorterm.as_deref(),
            env.term.as_deref(),
            env.term_program.as_deref(),
            env.no_color_present.then_some(""),
        );
        let color_source = if env.no_color_present {
            CapabilitySource::EnvironmentPolicy
        } else if tier != ColorTier::Ansi16 {
            CapabilitySource::EnvironmentInference
        } else {
            CapabilitySource::ConservativeDefault
        };
        let background = background_from_colorfgbg(env.colorfgbg.as_deref());
        let background_source = if background == BackgroundHint::Unknown {
            CapabilitySource::ConservativeDefault
        } else {
            CapabilitySource::EnvironmentInference
        };
        (
            ColorProfile::new(tier, AttrSupport::for_tier(tier), background),
            color_source,
            background_source,
        )
    };
    DetectionReport {
        capabilities: TerminalCapabilities::new(
            color,
            InputProfile::conservative(),
            ScreenProfile::conservative(),
            false,
            1,
        ),
        output_is_tty,
        color_source,
        background_source,
    }
}

/// Builds a refreshed color snapshot without reconfiguring active input modes.
///
/// Pure data helper only: the caller decides when to apply it. The backend
/// does not invoke it implicitly mid-operation. Previously established input
/// modes, screen support/geometry, and motion preference are retained.
/// `snapshot_id` advances locally; only changed domains advance their
/// version. Input-mode changes require leaving and entering a new session.
#[must_use]
pub fn refresh(
    previous: &TerminalCapabilities,
    env: &EnvSnapshot,
    output_is_tty: bool,
) -> TerminalCapabilities {
    let color = detect_from_env(env, output_is_tty).color;
    previous.refreshed(
        color,
        previous.input,
        previous.screen,
        previous.reduced_motion,
        previous.snapshot_id.saturating_add(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jilva_core::KeyEncoding;

    fn env_with(colorterm: Option<&str>, term: Option<&str>) -> EnvSnapshot {
        EnvSnapshot::new(
            colorterm.map(str::to_owned),
            term.map(str::to_owned),
            None,
            None,
            false,
        )
    }

    #[test]
    fn injected_color_matrix_is_conservative_and_deterministic() {
        let cases = [
            (Some("truecolor"), None, ColorTier::Truecolor),
            (Some("24BIT"), None, ColorTier::Truecolor),
            (None, Some("xterm-256color"), ColorTier::Palette256),
            (None, Some("xterm"), ColorTier::Ansi16),
            (None, Some("unknown-terminal"), ColorTier::Ansi16),
            (None, Some("dumb"), ColorTier::Ansi16),
            (None, None, ColorTier::Ansi16),
        ];
        for (colorterm, term, expected) in cases {
            let env = env_with(colorterm, term);
            let first = detect_from_env(&env, true);
            assert_eq!(first, detect_from_env(&env, true));
            assert_eq!(first.color.color, expected);
            assert_eq!(first.snapshot_id, 1);
            assert_eq!(first.input, InputProfile::conservative());
            assert_eq!(first.screen, ScreenProfile::conservative());
        }
    }

    #[test]
    fn no_color_presence_applies_jilva_policy_to_empty_and_nonempty_values() {
        // EnvSnapshot preserves presence because every value, including an
        // empty/non-Unicode one, has the same Jilva policy.
        let mut env = env_with(Some("truecolor"), None);
        env.no_color_present = true;
        let report = detect_report_from_env(&env, true);
        assert_eq!(report.capabilities.color.color, ColorTier::Mono);
        assert_eq!(report.color_source, CapabilitySource::EnvironmentPolicy);
        assert_eq!(
            report.capabilities.color.attrs,
            AttrSupport::for_tier(ColorTier::Mono)
        );
        assert!(report.capabilities.color.attrs.reverse);
        for value in ["", "0", "1", "false"] {
            assert_eq!(
                color_tier_from_env(Some("truecolor"), None, None, Some(value)),
                ColorTier::Mono
            );
        }
    }

    #[test]
    fn redirected_output_suppresses_all_attributes_and_overrides() {
        let rich = ColorProfile::new(
            ColorTier::Truecolor,
            AttrSupport::all(),
            BackgroundHint::Dark,
        );
        let report = detect_with_overrides(&env_with(Some("truecolor"), None), false, Some(rich));
        assert!(!report.output_is_tty);
        assert_eq!(report.color_source, CapabilitySource::RedirectedOutput);
        assert_eq!(report.background_source, CapabilitySource::RedirectedOutput);
        assert_eq!(report.capabilities.color.color, ColorTier::Mono);
        assert_eq!(report.capabilities.color.attrs, AttrSupport::none());
        assert_eq!(
            report.capabilities.color.background,
            BackgroundHint::Unknown
        );
        assert!(!report.capabilities.input.mouse);
        assert!(!report.capabilities.screen.alternate_screen);
    }

    #[test]
    fn report_distinguishes_inference_default_and_explicit_override() {
        let base = detect_report_from_env(&EnvSnapshot::empty(), true);
        assert_eq!(base.color_source, CapabilitySource::ConservativeDefault);
        let mut env = env_with(Some("truecolor"), None);
        env.colorfgbg = Some("15;0".into());
        let inferred = detect_report_from_env(&env, true);
        assert_eq!(
            inferred.color_source,
            CapabilitySource::EnvironmentInference
        );
        assert_eq!(
            inferred.background_source,
            CapabilitySource::EnvironmentInference
        );
        assert_eq!(inferred.capabilities.color.background, BackgroundHint::Dark);
        env.no_color_present = true;
        let chosen = ColorProfile::ansi16_baseline();
        let overridden = detect_with_overrides(&env, true, Some(chosen));
        assert_eq!(overridden.color_source, CapabilitySource::UserOverride);
        assert_eq!(overridden.capabilities.color, chosen);
    }

    #[test]
    fn refresh_preserves_established_modes_geometry_and_domain_causality() {
        let first = TerminalCapabilities::new(
            ColorProfile::ansi16_baseline(),
            InputProfile::new(KeyEncoding::Enhanced, true, true).with_focus_events(true),
            ScreenProfile::new(true, true).with_size(80, 24).unwrap(),
            true,
            12,
        );
        let same = refresh(&first, &EnvSnapshot::empty(), true);
        assert_eq!(same.snapshot_id, 13);
        assert_eq!(same.color_version, first.color_version);
        let changed = refresh(&same, &env_with(Some("truecolor"), None), true);
        assert_eq!(changed.snapshot_id, 14);
        assert_eq!(changed.color_version, first.color_version + 1);
        assert_eq!(changed.input_version, first.input_version);
        assert_eq!(changed.screen_version, first.screen_version);
        assert_eq!(changed.input, first.input);
        assert_eq!(changed.screen, first.screen);
        assert!(changed.reduced_motion);
    }
}
