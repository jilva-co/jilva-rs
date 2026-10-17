//! Public facade for `jilva-rs`: the only crate users depend on directly.
//!
//! Phase 2 exposes synchronous terminal sessions, typed input and output,
//! capability snapshots, and a deterministic headless backend. Phase 3 adds
//! the rendering engine: a grapheme-aware cell [`Buffer`], damage tracking,
//! and a pure [`diff`] / [`Renderer`] that produce backend instruction
//! streams, and a flex [`Layout`] that splits a [`Rect`] into regions. The
//! runtime slice (RFC-0008 Draft) adds the [`Model`] trait, the sans-IO
//! [`App`] runtime, and [`run_with`] (plus `run` with the default
//! `crossterm` feature) to drive it. This facade contains re-exports and
//! documentation; components, grid layout, and widgets belong to later
//! milestones.
//!
//! # A whole app
//!
//! ```rust,no_run
//! use jilva::{Align, App, Context, Effects, Event, Model, Outcome, View};
//!
//! struct Counter {
//!     count: i64,
//! }
//!
//! impl Model for Counter {
//!     type Message = ();
//!
//!     fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
//!         match event.key_text() {
//!             Some("+") => self.count += 1,
//!             Some("-") => self.count -= 1,
//!             Some("q") => effects.quit(),
//!             _ => return Ok(Outcome::Ignored),
//!         }
//!         Ok(Outcome::repaint())
//!     }
//!
//!     fn view(&self, _cx: &Context) -> jilva::Result<View> {
//!         Ok(View::text(&format!("count: {}", self.count)).text_align(Align::Center))
//!     }
//! }
//!
//! # #[cfg(feature = "crossterm")]
//! fn main() -> jilva::Result<()> {
//!     let app = jilva::run(App::new(Counter { count: 0 }))?;
//!     println!("final count: {}", app.model().count);
//!     Ok(())
//! }
//! # #[cfg(not(feature = "crossterm"))]
//! # fn main() {}
//! ```
//!
//! The default `crossterm` feature provides the real terminal backend.
//! Disable default features for a terminal-library-free headless build.
//!
//! # Example
//!
//! ```rust
//! assert_eq!(jilva::version(), "0.0.0");
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub use jilva_core::component::ComponentId;
pub use jilva_core::error::{Error, Result};
pub use jilva_core::events::{
    Change, Event, InputEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, Outcome,
};
pub use jilva_core::events::{MouseButton, MouseEvent, MouseKind, SpecialKey};
pub use jilva_core::layout::{Align, Constraint, Direction, Layout, Rect};
pub use jilva_core::lifecycle::{App, Context, Driven, Effects, LifecycleState, Model};
pub use jilva_core::motion::{Easing, Lerp, Transition};
pub use jilva_core::output::{Ansi16Color, OutputCommand, ResolvedAttributes, ResolvedColor};
pub use jilva_core::renderer::{
    Buffer, Cell, Damage, DiffOptions, Renderer, cluster_width, diff, graphemes, redraw,
    text_width, truncate_to_width,
};
pub use jilva_core::style::SemanticToken;
pub use jilva_core::view::{Backdrop, BorderStyle, Span, View};
pub use jilva_core::{
    AttrSupport, BackgroundHint, ColorProfile, ColorTier, InputProfile, KeyEncoding, ScreenProfile,
    TerminalCapabilities,
};
pub use jilva_terminal::backend::Backend;
pub use jilva_terminal::capabilities::{
    CapabilitySource, DetectionReport, EnvSnapshot, detect, detect_from_env,
    detect_report_from_env, detect_with_overrides, refresh,
};
#[cfg(feature = "crossterm")]
pub use jilva_terminal::crossterm_backend::{BackendOptions, CrosstermBackend};
#[cfg(feature = "crossterm")]
pub use jilva_terminal::driver::run;
pub use jilva_terminal::driver::run_with;
pub use jilva_terminal::test_double::{FailOp, TestDoubleBackend};

/// Returns the facade version users see.
///
/// Still `0.0.0`: unreleased. Tracks the workspace
/// version so the facade semver promise (`docs/API_DESIGN.md` §2) has one
/// observable source of truth from day one.
///
/// # Example
///
/// ```rust
/// assert_eq!(jilva::version(), "0.0.0");
/// ```
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
