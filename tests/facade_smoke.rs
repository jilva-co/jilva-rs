//! Facade smoke tests: the whole Phase-1 surface through `jilva` only.
//!
//! These tests import the facade crate exclusively — never `jilva-core` or
//! `jilva-terminal` directly — so anything they need must be reachable
//! through the facade (keeps `docs/ARCHITECTURE.md` §7 honest).
//!
//! Note on `expect` usage below: the in-memory test-double backend is
//! infallible by construction, so `expect` here asserts the seam contract
//! rather than hiding fallible I/O.

use jilva::{
    Backend, CapabilityProfile, Cell, ComponentId, Error, Event, LifecycleState, Rect,
    SemanticToken, TestDoubleBackend,
};

#[test]
fn facade_reports_unreleased_version() {
    assert_eq!(jilva::version(), "0.0.0");
}

#[test]
fn facade_error_taxonomy_displays() {
    let error = Error::Backend("no real backend yet".to_string());
    assert_eq!(error.to_string(), "backend error: no real backend yet");
}

#[test]
fn facade_layout_geometry_holds() {
    let region = Rect::new(0, 0, 80, 24);
    assert_eq!(region.area(), 1920);
    assert!(!region.is_empty());
    assert!(Rect::new(0, 0, 0, 0).is_empty());
}

#[test]
fn facade_lifecycle_rejects_illegal_transitions() {
    assert!(LifecycleState::Setup.can_transition_to(LifecycleState::Running));
    assert!(!LifecycleState::Setup.can_transition_to(LifecycleState::Shutdown));
}

#[test]
fn facade_events_classify_layout_impact() {
    assert!(
        Event::Resized {
            cols: 100,
            rows: 30
        }
        .affects_layout()
    );
    assert!(!Event::Tick.affects_layout());
}

#[test]
fn facade_test_double_backend_round_trips() {
    let mut backend = TestDoubleBackend::new(CapabilityProfile::basic());
    assert_eq!(backend.name(), "test-double");
    backend.enter().expect("test-double enter is infallible");
    assert!(backend.is_entered());
    backend.leave().expect("test-double leave is infallible");
    assert!(!backend.is_entered());
    assert_eq!(backend.log(), &["enter".to_string(), "leave".to_string()]);
}

#[test]
fn facade_identity_style_and_cell_basics() {
    assert_eq!(ComponentId::new(3).get(), 3);
    assert_eq!(SemanticToken::Danger.as_str(), "danger");
    assert!(Cell::blank().is_blank());
    assert!(!Cell::new('#').is_blank());
}
