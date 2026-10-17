//! App lifecycle (`docs/README.md#runtime`).
//!
//! [`LifecycleState`] is the state vocabulary. The runtime slice adds the
//! [`Model`] trait an application implements and the sans-IO [`App`]
//! runtime that turns (input, time) into output commands behind the
//! [`Driven`] trait. Terminal setup, the clock, and restoration on every exit
//! path belong to the driver in `jilva-terminal`; nothing here performs I/O.

mod app;
mod tree;

pub(crate) use app::FollowUps;
pub use app::{App, Context, Driven, Effects, Model};

/// Lifecycle state of an application run.
///
/// Legal transitions form a line: `Setup -> Running -> Shutdown`.
/// Anything else is rejected by [`LifecycleState::can_transition_to`]
/// so misuse fails as data, never as silent misbehaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LifecycleState {
    /// Terminal setup and initial configuration, before the first frame.
    Setup,
    /// Event loop is driving components and the renderer.
    Running,
    /// Loop has drained and teardown (terminal restoration) is due or done.
    Shutdown,
}

impl LifecycleState {
    /// Returns `true` when moving from `self` to `next` is a legal transition.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::LifecycleState;
    ///
    /// assert!(LifecycleState::Setup.can_transition_to(LifecycleState::Running));
    /// assert!(!LifecycleState::Setup.can_transition_to(LifecycleState::Shutdown));
    /// ```
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Setup, Self::Running) | (Self::Running, Self::Shutdown)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transitions_follow_the_line() {
        assert!(LifecycleState::Setup.can_transition_to(LifecycleState::Running));
        assert!(LifecycleState::Running.can_transition_to(LifecycleState::Shutdown));
    }

    #[test]
    fn illegal_transitions_are_rejected() {
        assert!(!LifecycleState::Setup.can_transition_to(LifecycleState::Shutdown));
        assert!(!LifecycleState::Running.can_transition_to(LifecycleState::Setup));
        assert!(!LifecycleState::Shutdown.can_transition_to(LifecycleState::Running));
        assert!(!LifecycleState::Shutdown.can_transition_to(LifecycleState::Shutdown));
    }
}
