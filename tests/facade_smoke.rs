//! Facade smoke tests: the whole Phase-1 surface through `jilva` only.
//!
//! These tests import the facade crate exclusively — never `jilva-core` or
//! `jilva-terminal` directly — so anything they need must be reachable
//! through the facade (keeps `docs/ARCHITECTURE.md` §7 honest).
//!
//! The test backend uses no OS terminal and has no injected failures here.

use jilva::{
    Backend, Cell, ComponentId, Error, Event, LifecycleState, Rect, SemanticToken,
    TerminalCapabilities, TestDoubleBackend,
};

#[test]
fn facade_reports_unreleased_version() {
    assert_eq!(jilva::version(), "0.0.0");
}

#[test]
fn facade_error_taxonomy_displays() {
    let error = Error::Backend("session unavailable".to_string());
    assert_eq!(error.to_string(), "backend error: session unavailable");
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
    let mut backend = TestDoubleBackend::new(TerminalCapabilities::conservative(0));
    assert_eq!(backend.name(), "test-double");
    backend.enter().expect("no injected enter failure");
    assert!(backend.is_entered());
    backend.leave().expect("no injected leave failure");
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
