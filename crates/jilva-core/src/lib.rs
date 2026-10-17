//! Framework foundations for `jilva-rs`.
//!
//! Phase 2 adds typed terminal events, resolved output instructions,
//! capability profiles, and pure validation/versioning helpers to the
//! foundation types. There is no application runtime, rendering engine,
//! layout solver, theme implementation, or widget system here.
//!
//! Platform-agnostic: this crate performs no terminal I/O and names no
//! backend. See `docs/ARCHITECTURE.md` §3 and §6 for the boundary contract.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod capabilities;
pub mod component;
pub mod error;
pub mod events;
pub mod layout;
pub mod lifecycle;
pub mod output;
pub mod renderer;
pub mod style;

pub use capabilities::{
    AttrSupport, BackgroundHint, ColorProfile, ColorTier, InputProfile, KeyEncoding, ScreenProfile,
    TerminalCapabilities, background_from_colorfgbg, color_tier_from_env, no_color_active,
};
pub use component::ComponentId;
pub use error::{Error, Result};
pub use events::{
    Event, InputEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseKind, SpecialKey,
};
pub use layout::Rect;
pub use lifecycle::LifecycleState;
pub use output::{Ansi16Color, OutputCommand, ResolvedAttributes, ResolvedColor};
pub use renderer::Cell;
pub use style::SemanticToken;
