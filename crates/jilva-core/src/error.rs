//! Coherent error taxonomy for the workspace.
//!
//! **Phase-1 status:** the variants below fix the categories the facade will
//! report (backend I/O, lifecycle, layout, render, user callbacks). Rich
//! context, backtraces, and per-crate error types are RFC detail; this enum
//! is the minimal honest starting point and depends on nothing but `std`.
//!
//! Framework operations use this error type through the facade. The low-level
//! terminal backend deliberately returns [`std::io::Result`] to retain I/O
//! error kinds and sources; callers can convert inward via [`From`].

use std::fmt;
use std::io;

/// The error type reported through the `jilva` facade.
///
/// Each variant names one failure domain from `docs/ARCHITECTURE.md` §3.9.
/// Payloads are plain data (`String` context or the underlying
/// [`std::io::Error`]); backend-specific types never appear here.
///
/// Marked `#[non_exhaustive]` so new failure domains can be added without
/// breaking downstream exhaustive matches before `1.0`.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Underlying OS / terminal I/O failure.
    Io(io::Error),
    /// Backend seam failure (setup, teardown, capability query).
    Backend(String),
    /// Lifecycle failure (invalid transition, teardown problem).
    Lifecycle(String),
    /// Layout failure (unsatisfiable or invalid constraints).
    Layout(String),
    /// Render failure (frame could not be produced or flushed).
    Render(String),
    /// Error propagated from a user-supplied callback.
    Callback(String),
    /// One source event produced more follow-up messages than the runtime
    /// processes in one batch (RFC-2 §1 rule 4). Reported instead of hanging.
    UpdateOverflow {
        /// The follow-up limit that was exceeded.
        limit: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "terminal I/O error: {error}"),
            Self::Backend(message) => write!(f, "backend error: {message}"),
            Self::Lifecycle(message) => write!(f, "lifecycle error: {message}"),
            Self::Layout(message) => write!(f, "layout error: {message}"),
            Self::Render(message) => write!(f, "render error: {message}"),
            Self::Callback(message) => write!(f, "user callback error: {message}"),
            Self::UpdateOverflow { limit } => write!(
                f,
                "update overflow: one event produced more than {limit} follow-up messages"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Backend(_)
            | Self::Lifecycle(_)
            | Self::Layout(_)
            | Self::Render(_)
            | Self::Callback(_)
            | Self::UpdateOverflow { .. } => None,
        }
    }
}

impl From<io::Error> for Error {
    /// Converts an I/O error into [`Error::Io`].
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Fallible-operation alias using the facade [`Error`] type.
///
/// Verbs that can fail return `Result<T>`; rendering a valid frame never
/// panics, and invalid usage surfaces here as data, not as a crash.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_domain_and_message() {
        let error = Error::Layout("over-constrained row".to_string());
        assert_eq!(error.to_string(), "layout error: over-constrained row");
    }

    #[test]
    fn io_converts_with_source_preserved() {
        let io_error = io::Error::other("boom");
        let error = Error::from(io_error);
        assert!(matches!(error, Error::Io(_)));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn update_overflow_names_its_limit() {
        let error = Error::UpdateOverflow { limit: 4 };
        assert_eq!(
            error.to_string(),
            "update overflow: one event produced more than 4 follow-up messages"
        );
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn non_io_variants_have_no_source() {
        let error = Error::Callback("user said no".to_string());
        assert!(std::error::Error::source(&error).is_none());
        assert_eq!(error.to_string(), "user callback error: user said no");
    }
}
