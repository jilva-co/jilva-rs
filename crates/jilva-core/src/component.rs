//! Component identity (`docs/ARCHITECTURE.md` §3.3).
//!
//! **Phase-1 status:** only the identity handle exists, so reconciliation has
//! a stable vocabulary to build on. Whether identity is positional, keyed, or
//! explicit — and the state-ownership, event-bubbling, and composition rules —
//! are RFC-2 detail. This handle makes no claim beyond uniqueness by value.

use std::fmt;

/// Stable identity handle distinguishing components across frames.
///
/// Two components with equal ids are the "same component, new data";
/// different ids mean different components. Allocation policy (who mints ids
/// and how collisions are reported) is RFC-2 detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(u64);

impl ComponentId {
    /// Creates an identity handle from a raw value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_core::ComponentId;
    ///
    /// let id = ComponentId::new(7);
    /// assert_eq!(id.get(), 7);
    /// ```
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the raw identity value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ComponentId {
    /// Formats the raw identity value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "component#{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_round_trips_and_displays() {
        let id = ComponentId::new(7);
        assert_eq!(id.get(), 7);
        assert_eq!(id.to_string(), "component#7");
        assert_eq!(id, ComponentId::new(7));
        assert_ne!(id, ComponentId::new(8));
    }
}
