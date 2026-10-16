//! In-memory backend for headless tests.
//!
//! [`TestDoubleBackend`] implements [`Backend`] without touching a terminal:
//! `enter`/`leave` flip an in-memory flag and append to a call log that tests
//! assert against. Scripted input events and asserted instruction streams are
//! RFC detail; this double only proves the seam is substitutable today.

use super::backend::Backend;
use super::capabilities::CapabilityProfile;

/// In-memory [`Backend`] stand-in: no TTY, no syscalls, fully scriptable.
///
/// Constructed with an explicit [`CapabilityProfile`] so tests exercise each
/// degradation tier deterministically.
#[derive(Debug, Default)]
pub struct TestDoubleBackend {
    /// Whether the backend is currently entered.
    entered: bool,
    /// The capability profile this double reports.
    profile: CapabilityProfile,
    /// Ordered record of `enter`/`leave` calls for assertions.
    log: Vec<String>,
}

impl TestDoubleBackend {
    /// Creates a test double reporting the given capability profile.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_terminal::{CapabilityProfile, TestDoubleBackend};
    ///
    /// let backend = TestDoubleBackend::new(CapabilityProfile::basic());
    /// assert!(!backend.is_entered());
    /// ```
    #[must_use]
    pub fn new(profile: CapabilityProfile) -> Self {
        Self {
            entered: false,
            profile,
            log: Vec::new(),
        }
    }

    /// Returns `true` while the backend is entered (between `enter` and `leave`).
    #[must_use]
    pub fn is_entered(&self) -> bool {
        self.entered
    }

    /// Returns the capability profile this double reports.
    #[must_use]
    pub const fn profile(&self) -> CapabilityProfile {
        self.profile
    }

    /// Returns the ordered record of `enter`/`leave` calls.
    #[must_use]
    pub fn log(&self) -> &[String] {
        &self.log
    }
}

impl Backend for TestDoubleBackend {
    /// Returns `"test-double"`.
    fn name(&self) -> &str {
        "test-double"
    }

    /// Marks the backend entered and records the call; never fails.
    fn enter(&mut self) -> std::io::Result<()> {
        self.entered = true;
        self.log.push("enter".to_string());
        Ok(())
    }

    /// Marks the backend restored and records the call; never fails.
    fn leave(&mut self) -> std::io::Result<()> {
        self.entered = false;
        self.log.push("leave".to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_leave_are_tracked_in_order() {
        let mut backend = TestDoubleBackend::new(CapabilityProfile::full());
        assert_eq!(backend.name(), "test-double");
        assert!(!backend.is_entered());

        backend.enter().expect("in-memory enter never fails");
        assert!(backend.is_entered());

        backend.leave().expect("in-memory leave never fails");
        assert!(!backend.is_entered());

        assert_eq!(backend.log(), &["enter".to_string(), "leave".to_string()]);
        assert_eq!(backend.profile(), CapabilityProfile::full());
    }
}
