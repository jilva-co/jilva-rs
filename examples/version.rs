//! Facade foundation smoke example, runnable without a terminal.
//!
//! Prints the facade version, exercises the error taxonomy, lays out one
//! cell-geometry region, and drives the in-memory test backend. The real
//! terminal engine has its own `backend_demo` example.
//!
//! Run with: `cargo run -p jilva --example version`

use jilva::{
    Backend, Cell, ComponentId, Error, Event, LifecycleState, Rect, SemanticToken,
    TerminalCapabilities, TestDoubleBackend,
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

    let mut backend = TestDoubleBackend::new(TerminalCapabilities::conservative(0));
    // No failures are configured in this small headless example.
    backend.enter().expect("no injected enter failure");
    println!(
        "backend '{}' entered: {}",
        backend.name(),
        backend.is_entered()
    );
    backend.leave().expect("no injected leave failure");

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
