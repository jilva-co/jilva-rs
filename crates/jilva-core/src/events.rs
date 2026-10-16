//! State and event propagation taxonomy (`docs/ARCHITECTURE.md` §3.4).
//!
//! **Phase-1 status:** a minimal event vocabulary proving the shape of the
//! taxonomy (timer, resize, app-defined message). The full ladder — keyboard
//! encodings, mouse modes, tick sources, focus and dispatch order — is RFC
//! detail. Flow stays unidirectional: events flow in, updates reconcile, and
//! components never touch the terminal.

/// An event delivered to the application for one transactional update.
///
/// Exactly one update follows one event; intermediate states are never
/// painted. Marked `#[non_exhaustive]` so keyboard, mouse, and future variants
/// can be added without breaking downstream exhaustive matches before `1.0`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// Timer tick, for animation and polling.
    Tick,
    /// Terminal resized; layout must be recomputed for the new dimensions.
    Resized {
        /// New terminal width in cells (columns).
        cols: u16,
        /// New terminal height in cells (rows).
        rows: u16,
    },
    /// Application-defined message through the user-message channel.
    Custom(String),
}

impl Event {
    /// Returns `true` when handling this event must invalidate layout.
    ///
    /// Only [`Event::Resized`] forces re-layout today; ticks and custom
    /// messages are paint-only until an RFC says otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::Event;
    ///
    /// assert!(Event::Resized { cols: 80, rows: 24 }.affects_layout());
    /// assert!(!Event::Tick.affects_layout());
    /// ```
    #[must_use]
    pub fn affects_layout(&self) -> bool {
        match self {
            Self::Resized { .. } => true,
            Self::Tick | Self::Custom(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_resize_invalidates_layout() {
        assert!(Event::Resized { cols: 80, rows: 24 }.affects_layout());
        assert!(!Event::Tick.affects_layout());
        assert!(!Event::Custom("hello".to_string()).affects_layout());
    }
}
