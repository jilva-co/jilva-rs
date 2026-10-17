//! Framework foundations for `jilva-rs`.
//!
//! Phase 2 added typed terminal events, resolved output instructions,
//! capability profiles, and pure validation/versioning helpers. Phase 3 adds
//! the rendering engine: a grapheme-aware cell [`Buffer`], damage tracking,
//! and a pure [`diff`] that turns two buffers into an instruction stream.
//! A first Phase 4 slice adds a one-dimensional flex [`Layout`] solver and an
//! owned [`View`] description with a paint pass. The runtime slice
//! (RFC-0008 Draft) adds the [`Model`] trait and the sans-IO [`App`] runtime
//! that a terminal driver pumps through [`Driven`]. There is no component
//! tree, grid layout, theme implementation, or widget system here yet.
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
pub mod motion;
pub mod output;
pub mod renderer;
pub mod style;
pub mod view;

pub use capabilities::{
    AttrSupport, BackgroundHint, ColorProfile, ColorTier, InputProfile, KeyEncoding, ScreenProfile,
    TerminalCapabilities, background_from_colorfgbg, color_tier_from_env, no_color_active,
};
pub use component::ComponentId;
pub use error::{Error, Result};
pub use events::{
    Change, Event, InputEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
    MouseEvent, MouseKind, Outcome, SpecialKey,
};
pub use layout::{Align, Constraint, Direction, Layout, Rect};
pub use lifecycle::{App, Context, Driven, Effects, LifecycleState, Model};
pub use motion::{Easing, Lerp, Transition};
pub use output::{Ansi16Color, OutputCommand, ResolvedAttributes, ResolvedColor};
pub use renderer::{Buffer, Cell, Damage, DiffOptions, Renderer, diff, redraw};
pub use style::SemanticToken;
pub use view::{Backdrop, BorderStyle, Span, View};
