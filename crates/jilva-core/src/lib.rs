//! Framework foundations for `jilva-rs`.
//!
//! Typed events, capabilities, output instructions, a grapheme-aware [`Buffer`],
//! pure [`diff`], flex [`Layout`], and owned [`View`] descriptions.
//! [`Model`] and the sans-IO [`App`] runtime run through [`Driven`];
//! [`Component`]s have framework-owned slots, identity, and focus.
//! Semantic [`Theme`] palettes resolve roles and states across terminal tiers.
//! Grid and a widget library are planned.
//!
//! Platform-agnostic: this crate performs no terminal I/O and names no
//! backend. See `docs/README.md#architecture` for the boundary contract.

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
pub use component::{Component, ComponentId, Key};
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
pub use style::{ResolvedStyle, ResolvedTheme, SemanticToken, StyleState, Theme};
pub use view::{Backdrop, BorderStyle, Span, View};
