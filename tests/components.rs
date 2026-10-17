//! Components end to end, through the facade only (RFC-0008 slice B).
//!
//! Two counter components run under the real driver against the headless
//! test double. The model stores no counts: each component's state lives in
//! a framework-owned slot, and Tab moves focus between them.

use jilva::{
    App, Component, Constraint, Context, Effects, Event, InputEvent, KeyEncoding, KeyEvent,
    KeyModifiers, Model, Outcome, OutputCommand, SpecialKey, TestDoubleBackend, View,
    cluster_width, graphemes, run_with,
};

#[derive(Clone, PartialEq)]
struct Counter(&'static str);

impl Component for Counter {
    type State = i64;
    type Message = ();

    fn init(&self) -> jilva::Result<i64> {
        Ok(0)
    }

    fn update(
        &self,
        count: &mut i64,
        event: Event,
        _effects: &mut Effects,
    ) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => *count += 1,
            Some("-") => *count -= 1,
            _ => return Ok(Outcome::Ignored),
        }
        Ok(Outcome::repaint())
    }

    fn view(&self, count: &i64, cx: &Context) -> jilva::Result<View> {
        let mark = if cx.is_focused() { ">" } else { " " };
        Ok(View::text(&format!("{mark}{}={count}", self.0)))
    }

    fn focusable(&self) -> bool {
        true
    }
}

struct Pair;

impl Model for Pair {
    type Message = ();

    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        if event.key_text() == Some("q") {
            effects.quit();
            return Ok(Outcome::unchanged());
        }
        Ok(Outcome::Ignored)
    }

    fn view(&self, _cx: &Context) -> jilva::Result<View> {
        Ok(View::row([
            (Constraint::Length(5), View::component(Counter("a"))),
            (Constraint::Length(5), View::component(Counter("b"))),
        ]))
    }
}

fn tab() -> InputEvent {
    InputEvent::Key(KeyEvent::special(
        SpecialKey::Tab,
        KeyModifiers::NONE,
        KeyEncoding::Legacy,
    ))
}

fn backend(script: Vec<InputEvent>) -> TestDoubleBackend {
    let mut backend = TestDoubleBackend::with_size(TestDoubleBackend::full_profile(), 10, 1);
    backend.script_all(script);
    backend.fail_poll_when_exhausted();
    backend
}

/// Replays flushed streams onto a one-row grid.
fn screen(backend: &TestDoubleBackend) -> String {
    let mut row = vec![" ".to_string(); 10];
    let mut x = 0usize;
    for command in backend.flushed_command_batches().iter().flatten() {
        match command {
            OutputCommand::MoveTo { x: to, .. } => x = usize::from(*to),
            OutputCommand::PrintSegment { text } => {
                for cluster in graphemes(text) {
                    if let Some(cell) = row.get_mut(x) {
                        *cell = cluster.to_string();
                    }
                    x += usize::from(cluster_width(cluster));
                }
            }
            OutputCommand::ClearRegion { .. } => row.fill(" ".to_string()),
            _ => {}
        }
    }
    row.concat()
}

#[test]
fn each_component_keeps_its_own_state_and_tab_moves_focus() {
    let key = TestDoubleBackend::key;
    let mut backend = backend(vec![key('+'), tab(), key('+'), key('+'), key('q')]);
    run_with(&mut backend, App::new(Pair)).expect("runs to quit");
    assert_eq!(screen(&backend), " a=1 >b=2 ");
    assert!(!backend.is_entered());
}

#[test]
fn the_same_script_gives_the_same_frame_stream_with_components() {
    let key = TestDoubleBackend::key;
    let run = || {
        let mut backend = backend(vec![key('+'), tab(), key('-'), tab(), key('+'), key('q')]);
        run_with(&mut backend, App::new(Pair)).expect("runs to quit");
        backend.flushed_command_batches().to_vec()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_focus_move_repaints_only_the_span_between_the_focus_marks() {
    let mut backend = backend(vec![tab(), TestDoubleBackend::key('q')]);
    run_with(&mut backend, App::new(Pair)).expect("runs to quit");
    let batches = backend.flushed_command_batches();
    // The marks at columns 0 and 5 changed; printing through the unchanged
    // cells between them is cheaper than a second cursor move.
    assert_eq!(
        batches[1],
        [
            OutputCommand::move_to(0, 0),
            OutputCommand::print_segment(" a=0 >"),
        ]
    );
}
