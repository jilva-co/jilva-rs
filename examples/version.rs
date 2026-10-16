//! Honest Phase-1 demo: everything actually implemented, nothing faked.
//!
//! Prints the facade version, exercises the error taxonomy, lays out one
//! cell-geometery region, and drives the in-memory test-double backend.
//! This is NOT a TUI hello-world — there is no renderer or backend yet, and
//! this example refuses to pretend otherwise.
//!
//! Run with: `cargo run -p jilva --example version`

use jilva::{
    Backend, CapabilityProfile, Cell, ComponentId, Error, Event, LifecycleState, Rect,
    SemanticToken, TestDoubleBackend,
};

fn main() {
    println!("jilva facade version: {}", jilva::version());

    let region = Rect::new(0, 0, 80, 24);
    println!(
        "layout region: {}x{} cells, area {}",
        region.width,
        region.height,
        region.area()
    );

    let event = Event::Resized { cols: 80, rows: 24 };
    println!("event affects layout: {}", event.affects_layout());

    let error = Error::Layout("example: no constraints to solve yet".to_string());
    println!("error taxonomy: {error}");

    let mut backend = TestDoubleBackend::new(CapabilityProfile::basic());
    // Justification for expect: the in-memory double is infallible by
    // construction; a failure here would mean the seam itself is broken.
    backend.enter().expect("test-double enter is infallible");
    println!(
        "backend '{}' entered: {}",
        backend.name(),
        backend.is_entered()
    );
    backend.leave().expect("test-double leave is infallible");

    println!(
        "component {}, token '{}', cell blank: {}",
        ComponentId::new(1),
        SemanticToken::Accent.as_str(),
        Cell::blank().is_blank()
    );
    println!(
        "lifecycle setup->running legal: {}",
        LifecycleState::Setup.can_transition_to(LifecycleState::Running)
    );
}
