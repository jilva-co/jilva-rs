//! The backend seam: everything touching a real terminal hides here.
//!
//! Contract (`docs/README.md#terminal`): identity, session, observation,
//! emission, and cached capabilities. The framework drives; the backend
//! obeys. The backend never consumes components, app state, or style tokens.
//! Rectangles are plain terminal cell geometry for clear commands.
//!
//! ## Canonical types
//!
//! Shared plain-data types (`InputEvent`, `OutputCommand`,
//! `TerminalCapabilities` and sub-types) are owned by `jilva-core` and
//! re-exported below.

use std::io;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Canonical shared types.
//
// `jilva-core` owns every shared plain-data type; this module re-exports
// them so backend implementors have a single import hub. Definitions live
// in `jilva-core/src/{capabilities,events,output,layout}.rs`.
// ---------------------------------------------------------------------------

pub use jilva_core::{
    Ansi16Color, AttrSupport, BackgroundHint, ColorProfile, ColorTier, InputEvent, InputProfile,
    KeyCode, KeyEncoding, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseKind,
    OutputCommand, Rect, ResolvedAttributes, ResolvedColor, ScreenProfile, SpecialKey,
    TerminalCapabilities,
};

// ---------------------------------------------------------------------------
// The seam.
// ---------------------------------------------------------------------------

/// Narrow interface isolating all terminal interaction.
///
/// The framework drives everything else; the backend only translates between
/// decoded [`InputEvent`]s / [`OutputCommand`]s and the real terminal.
/// Backend selection happens at the app-construction edge, so swapping in
/// [`crate::TestDoubleBackend`] never changes production code.
///
/// ## Session rules
///
/// One live session per backend instance. `enter` on an entered backend is an
/// error; `leave` on a fully inactive backend is `Ok(())` (idempotent, so `Drop`
/// guards and double-teardown paths are safe). `size` / `poll` / `read` /
/// `submit` / `flush` outside a live session fail loud. All mutating methods
/// take `&mut self`, so there is no reentrancy by construction.
///
/// Every method is required: both production backends implement the full
/// trait, and the fail-loud session policy below is enforced by each
/// implementor. Shared data and error semantics are identical for the real
/// backend and the headless substitute.
pub trait Backend {
    /// Returns a human-readable backend name for diagnostics and errors.
    fn name(&self) -> &str;

    /// Acquires terminal resources and caches one capability snapshot.
    /// Optional modes are enabled only according to supported session options.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] when the terminal cannot be entered, or when
    /// called on an already-entered backend. A failed rollback retains pending
    /// cleanup state for a later `leave` or Drop attempt.
    fn enter(&mut self) -> io::Result<()>;

    /// Restores the terminal, undoing everything [`Backend::enter`] did, in
    /// reverse order.
    ///
    /// Idempotent when fully inactive. A failed setup or prior cleanup can
    /// leave resource flags pending even when the session is not usable;
    /// calling `leave` retries those resources and can still return an error.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] when the terminal cannot be restored. Teardown
    /// still attempts every restoration step and retains failed resource flags
    /// for retry. Successful teardown discards pending input/output.
    fn leave(&mut self) -> io::Result<()>;

    /// Returns cached terminal dimensions in cells (columns, rows), updated
    /// when a resize event is consumed.
    ///
    /// A zero width or height is reported as an error, never as `(0, 0)`.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] outside a live session, when the size cannot
    /// be queried, or when the terminal reports a degenerate size.
    fn size(&self) -> io::Result<(u16, u16)>;

    /// Returns `true` when the input source is ready. `Duration::ZERO` makes
    /// this a non-blocking poll. Filtering unsupported input can still cause
    /// the subsequent [`Backend::read`] to return `None`.
    ///
    /// Uses the supplied wait bound subject to OS scheduling. The headless
    /// backend returns immediately for every timeout.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] outside a live session or when the terminal
    /// cannot be polled.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;

    /// Drains one decoded input event, or `Ok(None)` when the queue is empty.
    ///
    /// `Ok(None)` is normal operation. Unsupported or capability-gated input
    /// is counted and skipped within a bounded read; the next call can
    /// continue draining. Terminal-library I/O errors are propagated with their
    /// source and kind. A zero-time readiness probe prevents waiting for input;
    /// callers must not read Crossterm input concurrently outside this seam.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] outside a live session or on terminal I/O
    /// failure (distinct from a skipped undecodable sequence, which is not
    /// an error).
    fn read(&mut self) -> io::Result<Option<InputEvent>>;

    /// Validates the complete batch, then buffers commands in FIFO order
    /// without terminal writes. Invalid text or geometry leaves the existing
    /// pending batch untouched. ESC, C0, DEL and C1 text are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] outside a live session or for invalid commands.
    fn submit(&mut self, commands: &[OutputCommand]) -> io::Result<()>;

    /// Commits everything buffered by [`Backend::submit`] since the last
    /// `flush` to the terminal, in order. After `Ok(())` the buffer is empty.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] on failure. An attempted flush discards its pending
    /// batch even on failure: some bytes may already have reached the terminal,
    /// so automatic replay would duplicate an unknown prefix. A caller may
    /// explicitly submit a fresh recovery batch. OS writes are not atomic.
    fn flush(&mut self) -> io::Result<()>;

    /// Returns the cached capability snapshot established at [`Backend::enter`],
    /// with geometry/version updates for consumed resize events.
    ///
    /// Never re-probes the terminal. Before enter, returns the backend's
    /// conservative or explicitly injected profile.
    fn capabilities(&self) -> TerminalCapabilities;
}
