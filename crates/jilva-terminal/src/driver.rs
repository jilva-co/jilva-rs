//! The run driver (`docs/RFC/0008-app-runtime.md` §4).
//!
//! The driver owns exactly three things: the terminal session, the wall
//! clock, and the event-pump loop. It reads input from a [`Backend`], hands
//! it with the elapsed time to a core-defined [`Driven`] runtime, and writes
//! back the output commands it returns. Dispatch, update, view, layout and
//! paint all stay in `jilva-core` (`docs/ARCHITECTURE.md` §6 rules 1 and 3):
//! the `D: Driven` parameter is opaque here and never inspected.

use std::time::{Duration, Instant};

use jilva_core::{Driven, OutputCommand, Result};

use crate::backend::Backend;

/// Longest single wait for input while idle. Input wakes the loop at once;
/// this only bounds how often an idle loop re-checks that it should continue.
const IDLE_POLL: Duration = Duration::from_millis(250);

/// Runs `app` in the real terminal until it finishes, then restores the
/// terminal and returns the app.
///
/// Uses [`CrosstermBackend`](crate::CrosstermBackend) with default options
/// (alternate screen, hidden cursor). See [`run_with`] for the exit-path
/// guarantees and for choosing another backend.
///
/// # Errors
///
/// Returns the first error from the terminal session or the app; the
/// terminal is restored first.
#[cfg(feature = "crossterm")]
pub fn run<D: Driven>(app: D) -> Result<D> {
    run_with(&mut crate::CrosstermBackend::new(), app)
}

/// Runs `app` on `backend` until it finishes, then restores the terminal and
/// returns the app.
///
/// The loop is `enter` → [`Driven::start`] → { `poll` → `read` →
/// [`Driven::handle`] → `submit` → `flush` } → `leave`. Time passed to the
/// app is the elapsed wall-clock time since the session started.
///
/// The terminal is restored on every exit path the process survives: normal
/// return, an error from the app or the backend, and panic unwind (a guard
/// calls `leave`). A panic configured to abort cannot be intercepted.
///
/// # Example
///
/// Headless, with a scripted backend and a trivial runtime:
///
/// ```rust
/// use std::time::Duration;
/// use jilva_core::{Driven, InputEvent, OutputCommand, Result, TerminalCapabilities};
/// use jilva_terminal::{TestDoubleBackend, run_with};
///
/// /// Finishes on the first key.
/// struct UntilKey(bool);
///
/// impl Driven for UntilKey {
///     fn start(
///         &mut self,
///         _caps: &TerminalCapabilities,
///         _size: (u16, u16),
///         _now: Duration,
///     ) -> Result<Vec<OutputCommand>> {
///         Ok(vec![OutputCommand::print_segment("ready")])
///     }
///     fn handle(&mut self, input: InputEvent, _now: Duration) -> Result<Vec<OutputCommand>> {
///         self.0 = matches!(input, InputEvent::Key(_));
///         Ok(Vec::new())
///     }
///     fn is_finished(&self) -> bool {
///         self.0
///     }
/// }
///
/// let mut backend = TestDoubleBackend::default();
/// backend.script(TestDoubleBackend::key('q'));
/// backend.fail_poll_when_exhausted(); // a missing quit fails instead of spinning
/// let app = run_with(&mut backend, UntilKey(false))?;
/// assert!(app.is_finished());
/// assert!(!backend.is_entered());
/// # Ok::<(), jilva_core::Error>(())
/// ```
///
/// # Errors
///
/// Returns the first error from `enter`, the app, or the I/O loop; then any
/// error from restoring the terminal.
pub fn run_with<D: Driven, B: Backend + ?Sized>(backend: &mut B, mut app: D) -> Result<D> {
    if let Err(error) = backend.enter() {
        // A failed rollback keeps pending cleanup; `leave` retries it.
        let _ = backend.leave();
        return Err(error.into());
    }
    let mut session = SessionGuard {
        backend,
        left: false,
    };
    let started = Instant::now();
    let pumped = pump(session.backend, &mut app, || started.elapsed());
    session.left = true;
    let restored = session.backend.leave();
    pumped?;
    restored?;
    Ok(app)
}

/// Calls `leave` if the loop unwinds before the normal teardown.
struct SessionGuard<'a, B: Backend + ?Sized> {
    backend: &'a mut B,
    left: bool,
}

impl<B: Backend + ?Sized> Drop for SessionGuard<'_, B> {
    fn drop(&mut self) {
        if !self.left {
            // Best effort during unwind; there is no caller to report to.
            let _ = self.backend.leave();
        }
    }
}

/// The event pump: first frame, then one `handle` per input until finished.
fn pump<D: Driven, B: Backend + ?Sized>(
    backend: &mut B,
    app: &mut D,
    now: impl Fn() -> Duration,
) -> Result<()> {
    let capabilities = backend.capabilities();
    let size = backend.size()?;
    let frame = app.start(&capabilities, size, now())?;
    present(backend, &frame)?;
    while !app.is_finished() {
        if !backend.poll(IDLE_POLL)? {
            continue;
        }
        while let Some(input) = backend.read()? {
            let frame = app.handle(input, now())?;
            present(backend, &frame)?;
            if app.is_finished() {
                break;
            }
        }
    }
    Ok(())
}

/// Submits and flushes one frame; an empty frame costs no backend call.
fn present<B: Backend + ?Sized>(backend: &mut B, frame: &[OutputCommand]) -> std::io::Result<()> {
    if frame.is_empty() {
        return Ok(());
    }
    backend.submit(frame)?;
    backend.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestDoubleBackend;
    use crate::test_double::FailOp;
    use jilva_core::{Error, InputEvent, KeyCode, TerminalCapabilities};
    use std::io;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// A runtime stand-in: prints each key, finishes on `q`, fails on `e`,
    /// panics on `p`.
    #[derive(Debug, Default)]
    struct Echo {
        started_with: Option<(u16, u16)>,
        keys: Vec<String>,
        finished: bool,
    }

    impl Driven for Echo {
        fn start(
            &mut self,
            _capabilities: &TerminalCapabilities,
            size: (u16, u16),
            _now: Duration,
        ) -> Result<Vec<OutputCommand>> {
            self.started_with = Some(size);
            Ok(vec![
                OutputCommand::move_to(0, 0),
                OutputCommand::print_segment("start"),
            ])
        }

        fn handle(&mut self, input: InputEvent, _now: Duration) -> Result<Vec<OutputCommand>> {
            let InputEvent::Key(key) = input else {
                return Ok(Vec::new());
            };
            let KeyCode::Text(text) = key.code else {
                return Ok(Vec::new());
            };
            match text.as_str() {
                "q" => {
                    self.finished = true;
                    Ok(Vec::new())
                }
                "e" => Err(Error::Callback("echo failed".to_string())),
                "p" => panic!("echo panicked"),
                _ => {
                    self.keys.push(text.clone());
                    Ok(vec![OutputCommand::print_segment(text)])
                }
            }
        }

        fn is_finished(&self) -> bool {
            self.finished
        }
    }

    fn scripted(keys: &str) -> TestDoubleBackend {
        let mut backend = TestDoubleBackend::with_size(TestDoubleBackend::full_profile(), 10, 3);
        for key in keys.chars() {
            backend.script(TestDoubleBackend::key(key));
        }
        backend.fail_poll_when_exhausted();
        backend
    }

    #[test]
    fn drives_until_finished_and_restores() {
        let mut backend = scripted("abq");
        let echo = run_with(&mut backend, Echo::default()).expect("runs");
        assert_eq!(echo.started_with, Some((10, 3)));
        assert_eq!(echo.keys, ["a", "b"]);
        assert!(!backend.is_entered());
        // First frame plus one frame per printed key; `q` paints nothing.
        assert_eq!(backend.flushed_command_batches().len(), 3);
    }

    #[test]
    fn an_app_error_restores_the_terminal_and_returns_the_error() {
        let mut backend = scripted("ae");
        let error = run_with(&mut backend, Echo::default()).expect_err("app fails");
        assert!(matches!(error, Error::Callback(_)));
        assert!(!backend.is_entered());
    }

    #[test]
    fn a_panic_unwinds_through_the_guard_and_restores_the_terminal() {
        let mut backend = scripted("ap");
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _ = run_with(&mut backend, Echo::default());
        }));
        assert!(unwound.is_err(), "the app panic propagates");
        assert!(!backend.is_entered());
    }

    #[test]
    fn a_script_without_quit_fails_instead_of_spinning() {
        let mut backend = scripted("ab");
        let error = run_with(&mut backend, Echo::default()).expect_err("exhausted");
        assert!(matches!(error, Error::Io(ref io) if io.kind() == io::ErrorKind::UnexpectedEof));
        assert!(!backend.is_entered());
    }

    #[test]
    fn an_enter_failure_never_starts_the_app() {
        let mut backend = scripted("q");
        backend.fail_enter(io::ErrorKind::PermissionDenied);
        let error = run_with(&mut backend, Echo::default()).expect_err("enter fails");
        assert!(matches!(error, Error::Io(_)));
        assert!(!backend.is_entered());
    }

    #[test]
    fn a_flush_failure_stops_the_loop_and_still_leaves() {
        let mut backend = scripted("aq");
        backend.inject_at(FailOp::Flush, 2, io::ErrorKind::BrokenPipe);
        let error = run_with(&mut backend, Echo::default()).expect_err("flush fails");
        assert!(matches!(error, Error::Io(ref io) if io.kind() == io::ErrorKind::BrokenPipe));
        assert!(!backend.is_entered());
    }
}
