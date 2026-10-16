//! The backend seam: everything touching a real terminal hides here.
//!
//! **Phase-1 status:** the smallest honest interface — setup, teardown, and
//! a name for diagnostics. Reading input events, writing draw instruction
//! streams, and reporting size/capabilities are RFC detail and are
//! deliberately absent so no fake backend can pretend to draw.

use std::io;

/// Narrow interface isolating all terminal interaction.
///
/// The framework drives everything else; the backend only enters and leaves
/// terminal mode. Backend selection happens at the app-construction edge, so
/// swapping in [`crate::TestDoubleBackend`] never changes production code.
/// Implementations must restore the terminal in `leave` on every exit path.
pub trait Backend {
    /// Returns a human-readable backend name for diagnostics and errors.
    fn name(&self) -> &str;

    /// Enters terminal mode (alternate screen, raw mode, or equivalents).
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] when the terminal cannot be entered.
    fn enter(&mut self) -> io::Result<()>;

    /// Restores the terminal, undoing everything [`Backend::enter`] did.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] when the terminal cannot be restored.
    fn leave(&mut self) -> io::Result<()>;
}
