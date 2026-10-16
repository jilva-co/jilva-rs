//! Framework foundations for `jilva-rs`.
//!
//! **Phase-1 status:** this crate contains only the minimal, honestly-typed
//! building blocks needed for a compilable, documented, and tested workspace
//! (identity, event taxonomy, cell geometry, style tokens, lifecycle states,
//! and the error taxonomy). There is deliberately **no** run loop, renderer,
//! layout solver, theme, or widget here — those arrive via the RFC process
//! described in `docs/RFC/`.
//!
//! Platform-agnostic: this crate performs no terminal I/O and names no
//! backend. See `docs/ARCHITECTURE.md` §3 and §6 for the boundary contract.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod component;
pub mod error;
pub mod events;
pub mod layout;
pub mod lifecycle;
pub mod renderer;
pub mod style;

pub use component::ComponentId;
pub use error::{Error, Result};
pub use events::Event;
pub use layout::Rect;
pub use lifecycle::LifecycleState;
pub use renderer::Cell;
pub use style::{ColorDepth, SemanticToken};
