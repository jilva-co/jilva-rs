//! The app runtime end to end, through the facade only (RFC-0008 slice A).
//!
//! A counter model runs under the real driver against the headless test
//! double: scripted keys in, flushed instruction streams out. A tiny virtual
//! screen replays those streams so assertions read like what a user sees.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use jilva::{
    Align, App, Backend, Context, Driven, Effects, Error, Event, InputEvent, KeyEvent,
    KeyEventKind, Model, Outcome, OutputCommand, TerminalCapabilities, TestDoubleBackend, View,
    cluster_width, graphemes, run_with,
};

struct Counter {
    count: i64,
}

impl Model for Counter {
    type Message = ();

    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => self.count += 1,
            Some("-") => self.count -= 1,
            Some("q") => effects.quit(),
            Some("!") => panic!("model panicked on purpose"),
            _ => return Ok(Outcome::Ignored),
        }
        Ok(Outcome::repaint())
    }

    fn view(&self, _cx: &Context) -> jilva::Result<View> {
        Ok(View::text(&format!("count: {}", self.count)).text_align(Align::Center))
    }
}

fn backend(cols: u16, rows: u16, keys: &str) -> TestDoubleBackend {
    let mut backend = TestDoubleBackend::with_size(TestDoubleBackend::full_profile(), cols, rows);
    for key in keys.chars() {
        backend.script(TestDoubleBackend::key(key));
    }
    backend.fail_poll_when_exhausted();
    backend
}

/// Replays flushed streams onto a grid of cells and returns its rows.
fn screen(backend: &TestDoubleBackend, cols: u16, rows: u16) -> Vec<String> {
    let (cols, rows) = (usize::from(cols), usize::from(rows));
    let mut grid = vec![vec![" ".to_string(); cols]; rows];
    let (mut x, mut y) = (0usize, 0usize);
    for command in backend.flushed_command_batches().iter().flatten() {
        match command {
            OutputCommand::MoveTo { x: to_x, y: to_y } => {
                (x, y) = (usize::from(*to_x), usize::from(*to_y));
            }
            OutputCommand::PrintSegment { text } => {
                for cluster in graphemes(text) {
                    let width = usize::from(cluster_width(cluster));
                    if y < rows && x < cols {
                        grid[y][x] = cluster.to_string();
                        if width == 2 && x + 1 < cols {
                            grid[y][x + 1] = String::new();
                        }
                    }
                    x += width;
                }
            }
            OutputCommand::ClearRegion { rect } => {
                for row in grid
                    .iter_mut()
                    .skip(usize::from(rect.y))
                    .take(usize::from(rect.height))
                {
                    for cell in row
                        .iter_mut()
                        .skip(usize::from(rect.x))
                        .take(usize::from(rect.width))
                    {
                        *cell = " ".to_string();
                    }
                }
            }
            _ => {}
        }
    }
    grid.into_iter().map(|row| row.concat()).collect()
}

#[test]
fn scripted_keys_drive_the_counter_and_the_terminal_is_restored() {
    let mut backend = backend(16, 1, "++-q");
    let app = run_with(&mut backend, App::new(Counter { count: 0 })).expect("runs to quit");
    assert_eq!(app.model().count, 1);
    assert!(app.is_finished());
    assert!(!backend.is_entered());
    assert_eq!(screen(&backend, 16, 1), ["    count: 1    "]);
    // First frame plus one per change; quitting paints nothing.
    assert_eq!(backend.flushed_command_batches().len(), 4);
}

#[test]
fn ignored_keys_cost_no_output() {
    let mut backend = backend(16, 1, "xyz+q");
    run_with(&mut backend, App::new(Counter { count: 0 })).expect("runs to quit");
    assert_eq!(backend.flushed_command_batches().len(), 2);
    assert_eq!(screen(&backend, 16, 1), ["    count: 1    "]);
}

#[test]
fn a_change_sends_only_the_changed_cells() {
    let mut backend = backend(16, 1, "+q");
    run_with(&mut backend, App::new(Counter { count: 0 })).expect("runs to quit");
    let batches = backend.flushed_command_batches();
    assert_eq!(
        batches[1],
        [
            OutputCommand::move_to(11, 0),
            OutputCommand::print_segment("1")
        ]
    );
}

#[test]
fn the_same_script_gives_the_same_frame_stream() {
    let run = || {
        let mut backend = backend(20, 3, "+-++x-q");
        run_with(&mut backend, App::new(Counter { count: 0 })).expect("runs to quit");
        backend.flushed_command_batches().to_vec()
    };
    assert_eq!(run(), run());
}

#[test]
fn ctrl_c_quits_by_default() {
    let mut backend = backend(16, 1, "+");
    backend.script(InputEvent::Key(
        KeyEvent::decode_legacy_byte(0x03).expect("ctrl+c"),
    ));
    let app = run_with(&mut backend, App::new(Counter { count: 0 })).expect("quits");
    assert_eq!(app.model().count, 1);
    assert!(!backend.is_entered());
}

#[test]
fn key_releases_are_dropped_before_the_model() {
    let mut backend = backend(16, 1, "");
    let release = KeyEvent::decode_legacy_byte(b'+')
        .expect("printable")
        .with_kind(KeyEventKind::Release);
    backend.script(InputEvent::Key(release));
    backend.script(TestDoubleBackend::key('q'));
    let app = run_with(&mut backend, App::new(Counter { count: 0 })).expect("quits");
    assert_eq!(app.model().count, 0);
}

#[test]
fn a_resize_redraws_at_the_new_size() {
    let mut backend = backend(16, 1, "+");
    backend.script(TestDoubleBackend::resize_event(10, 1).expect("non-zero size"));
    backend.script(TestDoubleBackend::key('q'));
    run_with(&mut backend, App::new(Counter { count: 0 })).expect("quits");
    assert_eq!(screen(&backend, 10, 1), [" count: 1 "]);
}

#[test]
fn a_model_panic_still_restores_the_terminal() {
    let mut backend = backend(16, 1, "+!");
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let _ = run_with(&mut backend, App::new(Counter { count: 0 }));
    }));
    assert!(unwound.is_err());
    assert!(!backend.is_entered());
}

#[test]
fn a_script_that_never_quits_fails_instead_of_hanging() {
    let mut backend = backend(16, 1, "++");
    let error = run_with(&mut backend, App::new(Counter { count: 0 })).expect_err("exhausted");
    assert!(matches!(error, Error::Io(_)));
    assert!(!backend.is_entered());
}

#[test]
fn the_runtime_runs_without_any_backend() {
    let caps = TerminalCapabilities::conservative(1);
    let key = |c: u8| InputEvent::Key(KeyEvent::decode_legacy_byte(c).expect("printable"));
    let mut app = App::new(Counter { count: 0 });
    let first = app
        .start(&caps, (16, 1), Duration::ZERO)
        .expect("first frame");
    assert!(!first.is_empty());
    let frame = app
        .handle(key(b'+'), Duration::from_millis(16))
        .expect("update");
    assert_eq!(
        frame,
        [
            OutputCommand::move_to(11, 0),
            OutputCommand::print_segment("1")
        ]
    );
    assert!(
        app.handle(key(b'?'), Duration::from_millis(32))
            .expect("ignored")
            .is_empty()
    );
}

#[test]
fn backends_stay_usable_through_the_facade_trait() {
    // `run_with` accepts any backend, including a trait object.
    let mut backend = backend(16, 1, "q");
    let dynamic: &mut dyn Backend = &mut backend;
    run_with(dynamic, App::new(Counter { count: 0 })).expect("quits");
    assert!(!backend.is_entered());
}
