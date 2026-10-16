//! Public facade for `jilva-rs`: the only crate users depend on directly.
//!
//! **Phase-1 status:** re-exports only — no logic lives here, per
//! `docs/ARCHITECTURE.md` §3.1 and §6. There is no `App`, no run loop, no
//! widget, and no real backend yet; those arrive via the RFC process. What
//! exists is the curated surface in its final shape: every item below is
//! reachable through this crate so that `examples/` and `tests/` only ever
//! import the facade, keeping it honest.
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
pub use jilva_core::events::Event;
pub use jilva_core::layout::Rect;
pub use jilva_core::lifecycle::LifecycleState;
pub use jilva_core::renderer::Cell;
pub use jilva_core::style::{ColorDepth, SemanticToken};
pub use jilva_terminal::backend::Backend;
pub use jilva_terminal::capabilities::CapabilityProfile;
pub use jilva_terminal::test_double::TestDoubleBackend;

/// Returns the facade version users see.
///
/// Still `0.0.0`: Phase-1 foundation, unreleased. Tracks the workspace
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
