//! Terminal backend abstraction (`docs/ARCHITECTURE.md` §3.5 and §3.10).
//!
//! **Phase-2 status:** the full backend seam ([`Backend`],
//! `docs/PHASE2_BACKEND_CONTRACT.md` §1) with the crossterm-based
//! `CrosstermBackend` (the default `crossterm` feature) and the in-memory
//! [`TestDoubleBackend`] for headless tests.
//!
//! Shared vocabulary ([`InputEvent`], [`OutputCommand`],
//! [`TerminalCapabilities`], …) is core-owned per contract §4.1 and
//! re-exported through [`backend`]; this crate adds detection plus the two
//! backends.
//!
//! The only crate in the workspace allowed to touch terminal I/O or depend
//! on a terminal library. It depends on `jilva-core` for shared types only
//! and never sees app state, components, or style tokens.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod backend;
pub mod capabilities;
#[cfg(feature = "crossterm")]
pub mod crossterm_backend;
pub mod test_double;

pub use backend::{
    Ansi16Color, AttrSupport, Backend, BackgroundHint, ColorProfile, ColorTier, InputEvent,
    InputProfile, KeyCode, KeyEncoding, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
    MouseEvent, MouseKind, OutputCommand, Rect, ResolvedAttributes, ResolvedColor, ScreenProfile,
    SpecialKey, TerminalCapabilities,
};
pub use capabilities::{
    CapabilitySource, DetectionReport, EnvSnapshot, detect, detect_from_env,
    detect_report_from_env, detect_with_overrides, refresh,
};
#[cfg(feature = "crossterm")]
pub use crossterm_backend::{BackendOptions, CrosstermBackend, decode_event};
pub use test_double::TestDoubleBackend;
