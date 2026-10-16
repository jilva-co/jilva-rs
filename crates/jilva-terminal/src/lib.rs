//! Terminal backend abstraction (`docs/ARCHITECTURE.md` §3.5 and §3.10).
//!
//! **Phase-1 status:** this crate defines only the backend seam
//! ([`Backend`]), the capability model ([`CapabilityProfile`]), and the
//! in-memory [`TestDoubleBackend`] for headless tests. There is no real
//! backend implementation yet — the default backend choice (ADR-0003) and
//! capability detection arrive via the RFC process.
//!
//! The only crate in the workspace allowed to touch terminal I/O or depend
//! on a terminal library. Depends on nothing in-workspace and never sees app
//! state, components, or style tokens.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod backend;
pub mod capabilities;
pub mod test_double;

pub use backend::Backend;
pub use capabilities::CapabilityProfile;
pub use test_double::TestDoubleBackend;
