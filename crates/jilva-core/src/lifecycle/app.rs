//! The sans-IO application runtime (`docs/RFC/0008-app-runtime.md` §2–§3).
//!
//! [`App`] turns (input, time) into output commands. It performs no I/O,
//! reads no clock and names no backend; a driver outside core pumps it
//! through the [`Driven`] trait. Same model, capabilities, size and
//! (time, input) sequence give a bit-identical command stream.

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

use crate::capabilities::TerminalCapabilities;
use crate::error::{Error, Result};
use crate::events::{Change, Event, InputEvent, KeyEventKind, Outcome};
use crate::layout::Rect;
use crate::lifecycle::LifecycleState;
use crate::output::OutputCommand;
use crate::renderer::Renderer;
use crate::view::View;

/// The application root: Elm-shaped state with a fallible `update` and a
/// pure `view` (RFC-2 §1).
///
/// `update` receives every event — keys, resizes, follow-up messages — and
/// reports both whether it consumed the event and what changed through
/// [`Outcome`]. `view` describes the whole screen from `&self`; it performs
/// no I/O and receives no coordinates.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use jilva_core::{
///     App, Context, Driven, Effects, Event, InputEvent, KeyEvent, Model, Outcome, Result,
///     TerminalCapabilities, View,
/// };
///
/// struct Counter {
///     count: i64,
/// }
///
/// impl Model for Counter {
///     type Message = ();
///
///     fn update(&mut self, event: Event, effects: &mut Effects) -> Result<Outcome> {
///         match event.key_text() {
///             Some("+") => self.count += 1,
///             Some("q") => effects.quit(),
///             _ => return Ok(Outcome::Ignored),
///         }
///         Ok(Outcome::repaint())
///     }
///
///     fn view(&self, _cx: &Context) -> Result<View> {
///         Ok(View::text(&format!("count: {}", self.count)))
///     }
/// }
///
/// let key = |byte| InputEvent::Key(KeyEvent::decode_legacy_byte(byte).expect("printable"));
/// let caps = TerminalCapabilities::conservative(1);
/// let mut app = App::new(Counter { count: 0 });
/// assert!(!app.start(&caps, (12, 1), Duration::ZERO)?.is_empty());
/// assert!(!app.handle(key(b'+'), Duration::from_millis(5))?.is_empty());
/// assert!(app.handle(key(b'x'), Duration::from_millis(6))?.is_empty()); // ignored
/// app.handle(key(b'q'), Duration::from_millis(7))?;
/// assert!(app.is_finished());
/// assert_eq!(app.into_model().count, 1);
/// # Ok::<(), jilva_core::Error>(())
/// ```
pub trait Model {
    /// The application's own follow-up message type (`()` when unused).
    type Message;

    /// Applies one event to the model.
    ///
    /// Return [`Outcome::Ignored`] for events the model does not handle;
    /// ignored key events fall through to the runtime defaults (Ctrl+C
    /// quits). Request follow-ups or quitting through `effects`.
    ///
    /// # Errors
    ///
    /// An error abandons the batch: nothing is painted, the previous frame
    /// stays, and the driver restores the terminal and returns the error.
    fn update(
        &mut self,
        event: Event<Self::Message>,
        effects: &mut Effects<Self::Message>,
    ) -> Result<Outcome>;

    /// Describes the whole screen. Pure: same model and context, same view.
    ///
    /// # Errors
    ///
    /// An error aborts the frame cleanly; nothing is painted.
    fn view(&self, cx: &Context) -> Result<View>;
}

/// The bounded effect queue an update receives (RFC-2 §1 rule 1).
///
/// Effects are data: the runtime performs them after the update returns,
/// never reentrantly. Follow-up messages run FIFO in the same batch, before
/// the next `view`.
///
/// # Example
///
/// ```rust
/// use jilva_core::Effects;
///
/// let mut effects: Effects<&str> = Effects::new();
/// effects.send("refresh");
/// assert_eq!(effects.pending(), 1);
/// assert!(!effects.quit_requested());
/// effects.quit();
/// assert!(effects.quit_requested());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects<M = ()> {
    messages: VecDeque<M>,
    quit: bool,
}

impl<M> Effects<M> {
    /// Creates an empty queue. The runtime makes one per source event; tests
    /// of a model's `update` make their own.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            quit: false,
        }
    }

    /// Asks the runtime to stop after this batch. No frame is painted for a
    /// batch that quits; the driver then restores the terminal.
    pub const fn quit(&mut self) {
        self.quit = true;
    }

    /// Queues a follow-up message, delivered as [`Event::Message`] after the
    /// current update returns.
    pub fn send(&mut self, message: M) {
        self.messages.push_back(message);
    }

    /// Returns `true` once [`Effects::quit`] was called.
    #[must_use]
    pub const fn quit_requested(&self) -> bool {
        self.quit
    }

    /// Returns the number of queued follow-up messages.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.messages.len()
    }

    /// Takes the oldest queued message.
    fn next_message(&mut self) -> Option<M> {
        self.messages.pop_front()
    }
}

impl<M> Default for Effects<M> {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only input to [`Model::view`]: screen size, capabilities, and the
/// injected time.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use jilva_core::{Context, Rect, TerminalCapabilities};
///
/// let cx = Context::new((80, 24), TerminalCapabilities::conservative(0), Duration::ZERO);
/// assert_eq!(cx.size(), (80, 24));
/// assert_eq!(cx.area(), Rect::new(0, 0, 80, 24));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    size: (u16, u16),
    capabilities: TerminalCapabilities,
    now: Duration,
}

impl Context {
    /// Creates a context. The runtime builds one per frame; tests of a
    /// model's `view` build their own.
    #[must_use]
    pub const fn new(size: (u16, u16), capabilities: TerminalCapabilities, now: Duration) -> Self {
        Self {
            size,
            capabilities,
            now,
        }
    }

    /// Screen size in cells (columns, rows).
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        self.size
    }

    /// The whole screen as a rectangle at the origin.
    #[must_use]
    pub const fn area(&self) -> Rect {
        Rect::new(0, 0, self.size.0, self.size.1)
    }

    /// The terminal's capability snapshot for this session.
    #[must_use]
    pub const fn capabilities(&self) -> &TerminalCapabilities {
        &self.capabilities
    }

    /// Time since the driver started, as injected by the driver. Never read
    /// from a clock inside core.
    #[must_use]
    pub const fn now(&self) -> Duration {
        self.now
    }
}

/// Something a terminal driver can pump: input and time in, output
/// commands out (`docs/RFC/0008-app-runtime.md` §3).
///
/// Object-safe on purpose: a driver needs no knowledge of the model type.
pub trait Driven {
    /// Begins the session and returns the first frame.
    ///
    /// # Errors
    ///
    /// Fails when called twice, for a zero-sized screen, or when the first
    /// `view` fails.
    fn start(
        &mut self,
        capabilities: &TerminalCapabilities,
        size: (u16, u16),
        now: Duration,
    ) -> Result<Vec<OutputCommand>>;

    /// Handles one source event and returns at most one frame of commands
    /// (empty when nothing changed).
    ///
    /// # Errors
    ///
    /// Fails outside a running session, when an update or view fails, or
    /// with [`Error::UpdateOverflow`].
    fn handle(&mut self, input: InputEvent, now: Duration) -> Result<Vec<OutputCommand>>;

    /// Returns `true` once the session asked to quit.
    fn is_finished(&self) -> bool;
}

/// The per-session part of the runtime: renderer plus current context.
struct Session {
    renderer: Renderer,
    context: Context,
}

/// The sans-IO runtime around a [`Model`].
///
/// Implements RFC-2 §1 for the app root: one source event → one update →
/// FIFO follow-ups → at most one `view` and one diff. Defaults: key releases
/// are dropped, Ctrl+C quits when the model ignores it, a resize always
/// repaints at the new size. See [`Model`] for a complete example.
pub struct App<M: Model> {
    model: M,
    state: LifecycleState,
    session: Option<Session>,
    /// Last painted view and its context, for the debug under-report check.
    #[cfg(debug_assertions)]
    last_frame: Option<(View, Context)>,
}

impl<M: Model> App<M> {
    /// Follow-up messages processed per source event before
    /// [`Error::UpdateOverflow`] is reported instead of hanging.
    pub const FOLLOW_UP_LIMIT: usize = 1024;

    /// Wraps a model. Nothing runs until [`Driven::start`].
    #[must_use]
    pub const fn new(model: M) -> Self {
        Self {
            model,
            state: LifecycleState::Setup,
            session: None,
            #[cfg(debug_assertions)]
            last_frame: None,
        }
    }

    /// Returns the model.
    #[must_use]
    pub const fn model(&self) -> &M {
        &self.model
    }

    /// Consumes the runtime and returns the model.
    #[must_use]
    pub fn into_model(self) -> M {
        self.model
    }

    /// Returns the lifecycle state: setup, running, or shut down.
    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        self.state
    }

    /// Runs one update plus its follow-ups and returns the batch's change
    /// class and whether it asked to quit.
    fn dispatch(&mut self, event: Event<M::Message>, ctrl_c: bool) -> Result<(Change, bool)> {
        let mut effects = Effects::new();
        let outcome = self.model.update(event, &mut effects)?;
        if ctrl_c && outcome == Outcome::Ignored {
            effects.quit();
        }
        let mut change = outcome.change();
        let mut follow_ups = 0;
        while let Some(message) = effects.next_message() {
            follow_ups += 1;
            if follow_ups > Self::FOLLOW_UP_LIMIT {
                return Err(Error::UpdateOverflow {
                    limit: Self::FOLLOW_UP_LIMIT,
                });
            }
            let outcome = self.model.update(Event::Message(message), &mut effects)?;
            change = change.max(outcome.change());
        }
        Ok((change, effects.quit_requested()))
    }

    /// Describes and paints one frame; on success records it for the debug
    /// check.
    fn paint(&mut self) -> Result<Vec<OutputCommand>> {
        let Some(session) = self.session.as_mut() else {
            return Err(Error::Lifecycle("App has no session to paint".to_string()));
        };
        let view = self.model.view(&session.context)?;
        let frame = session
            .renderer
            .render(&view, &session.context.capabilities);
        #[cfg(debug_assertions)]
        {
            self.last_frame = Some((view, session.context));
        }
        Ok(frame)
    }

    /// RFC-2 §5 under-reporting check: a batch that reported no change must
    /// describe the same view as the last frame (with the same context, so
    /// only model changes count). Debug builds only.
    #[cfg(debug_assertions)]
    fn check_unchanged(&self) {
        let Some((last_view, last_context)) = &self.last_frame else {
            return;
        };
        if let Ok(view) = self.model.view(last_context) {
            debug_assert!(
                view == *last_view,
                "update reported no change (Outcome::Ignored or Change::None) but view() \
                 now describes a different screen; return Outcome::repaint() or \
                 Outcome::layout() instead"
            );
        }
    }
}

impl<M: Model> fmt::Debug for App<M> {
    /// Shows the model's type name and the lifecycle state; the model itself
    /// need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("App")
            .field("model", &std::any::type_name::<M>())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<M: Model> Driven for App<M> {
    fn start(
        &mut self,
        capabilities: &TerminalCapabilities,
        size: (u16, u16),
        now: Duration,
    ) -> Result<Vec<OutputCommand>> {
        if self.state != LifecycleState::Setup {
            return Err(Error::Lifecycle("App::start called twice".to_string()));
        }
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::Render(
                "screen size must be non-zero in both dimensions".to_string(),
            ));
        }
        self.session = Some(Session {
            renderer: Renderer::new(size.0, size.1),
            context: Context::new(size, *capabilities, now),
        });
        match self.paint() {
            Ok(frame) => {
                self.state = LifecycleState::Running;
                Ok(frame)
            }
            Err(error) => {
                self.session = None;
                Err(error)
            }
        }
    }

    fn handle(&mut self, input: InputEvent, now: Duration) -> Result<Vec<OutputCommand>> {
        if self.state != LifecycleState::Running {
            return Err(Error::Lifecycle(format!(
                "App::handle needs a running session, found {:?}",
                self.state
            )));
        }
        if let InputEvent::Key(key) = &input
            && key.kind == KeyEventKind::Release
        {
            return Ok(Vec::new());
        }
        let Some(session) = self.session.as_mut() else {
            return Err(Error::Lifecycle("running App has no session".to_string()));
        };
        session.context.now = now;
        let mut forced = Change::None;
        if let InputEvent::Resize { cols, rows } = input {
            session.renderer.resize(cols, rows);
            session.context.size = (cols, rows);
            forced = Change::Layout;
        }
        let ctrl_c = matches!(&input, InputEvent::Key(key) if key.is_ctrl("c"));

        let (change, quit) = self.dispatch(Event::from(input), ctrl_c)?;
        if quit {
            self.state = LifecycleState::Shutdown;
            return Ok(Vec::new());
        }
        if forced.max(change) == Change::None {
            #[cfg(debug_assertions)]
            self.check_unchanged();
            return Ok(Vec::new());
        }
        self.paint()
    }

    fn is_finished(&self) -> bool {
        self.state == LifecycleState::Shutdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{KeyEvent, KeyModifiers, SpecialKey};
    use crate::{KeyEncoding, MouseButton, MouseEvent, MouseKind};

    /// A counter that records every event it saw.
    #[derive(Default)]
    struct Counter {
        count: i64,
        seen: Vec<String>,
        consume_ctrl_c: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Msg {
        Add(i64),
        Chain(usize),
        Fail,
    }

    impl Model for Counter {
        type Message = Msg;

        fn update(&mut self, event: Event<Msg>, effects: &mut Effects<Msg>) -> Result<Outcome> {
            self.seen.push(format!("{event:?}"));
            if let Event::Message(message) = event {
                match message {
                    Msg::Add(n) => self.count += n,
                    Msg::Chain(0) => {}
                    Msg::Chain(n) => effects.send(Msg::Chain(n - 1)),
                    Msg::Fail => return Err(Error::Callback("asked to fail".to_string())),
                }
                return Ok(Outcome::repaint());
            }
            if self.consume_ctrl_c && event.key().is_some_and(|key| key.is_ctrl("c")) {
                return Ok(Outcome::unchanged());
            }
            match event.key_text() {
                Some("+") => self.count += 1,
                Some("-") => self.count -= 1,
                Some("d") => {
                    effects.send(Msg::Add(10));
                    effects.send(Msg::Add(100));
                }
                Some("c") => effects.send(Msg::Chain(usize::MAX)),
                Some("e") => effects.send(Msg::Fail),
                Some("n") => return Ok(Outcome::unchanged()),
                Some("q") => effects.quit(),
                _ => return Ok(Outcome::Ignored),
            }
            Ok(Outcome::repaint())
        }

        fn view(&self, cx: &Context) -> Result<View> {
            if self.count < -5 {
                return Err(Error::Render("count below range".to_string()));
            }
            let (cols, rows) = cx.size();
            Ok(View::text(&format!("{} {cols}x{rows}", self.count)))
        }
    }

    fn caps() -> TerminalCapabilities {
        TerminalCapabilities::conservative(1)
    }

    fn key(byte: u8) -> InputEvent {
        InputEvent::Key(KeyEvent::decode_legacy_byte(byte).expect("decodable byte"))
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn started(model: Counter) -> App<Counter> {
        let mut app = App::new(model);
        let first = app.start(&caps(), (12, 2), ms(0)).expect("first frame");
        assert!(!first.is_empty());
        app
    }

    #[test]
    fn a_consumed_key_paints_and_an_ignored_key_emits_nothing() {
        let mut app = started(Counter::default());
        assert!(!app.handle(key(b'+'), ms(1)).expect("update").is_empty());
        assert_eq!(app.model().count, 1);
        assert!(app.handle(key(b'x'), ms(2)).expect("update").is_empty());
        assert!(app.handle(key(b'n'), ms(3)).expect("update").is_empty());
        assert_eq!(app.state(), LifecycleState::Running);
    }

    #[test]
    fn key_releases_never_reach_the_model() {
        let mut app = started(Counter::default());
        let release = KeyEvent::decode_legacy_byte(b'+')
            .expect("printable")
            .with_kind(KeyEventKind::Release);
        assert!(
            app.handle(InputEvent::Key(release), ms(1))
                .expect("dropped")
                .is_empty()
        );
        assert_eq!(app.model().count, 0);
        assert!(app.model().seen.is_empty());
        let repeat = KeyEvent::decode_legacy_byte(b'+')
            .expect("printable")
            .with_kind(KeyEventKind::Repeat);
        app.handle(InputEvent::Key(repeat), ms(2)).expect("update");
        assert_eq!(app.model().count, 1);
    }

    #[test]
    fn quit_ends_the_session_without_a_frame() {
        let mut app = started(Counter::default());
        assert!(app.handle(key(b'q'), ms(1)).expect("quit").is_empty());
        assert!(app.is_finished());
        assert_eq!(app.state(), LifecycleState::Shutdown);
        assert!(matches!(
            app.handle(key(b'+'), ms(2)),
            Err(Error::Lifecycle(_))
        ));
    }

    #[test]
    fn ctrl_c_quits_only_when_the_model_ignores_it() {
        let mut app = started(Counter::default());
        app.handle(key(0x03), ms(1)).expect("default quit");
        assert!(app.is_finished());

        let mut app = started(Counter {
            consume_ctrl_c: true,
            ..Counter::default()
        });
        app.handle(key(0x03), ms(1)).expect("consumed");
        assert!(!app.is_finished());
    }

    #[test]
    fn follow_ups_run_fifo_in_the_same_batch() {
        let mut app = started(Counter::default());
        let frame = app.handle(key(b'd'), ms(1)).expect("batch");
        assert!(!frame.is_empty());
        assert_eq!(app.model().count, 110);
        let seen = &app.model().seen;
        assert_eq!(seen.len(), 3);
        assert!(seen[1].contains("Add(10)") && seen[2].contains("Add(100)"));
    }

    #[test]
    fn runaway_follow_ups_report_overflow_instead_of_hanging() {
        let mut app = started(Counter::default());
        let error = app.handle(key(b'c'), ms(1)).expect_err("overflow");
        assert!(matches!(
            error,
            Error::UpdateOverflow {
                limit: App::<Counter>::FOLLOW_UP_LIMIT
            }
        ));
        assert_eq!(
            app.model().seen.len(),
            App::<Counter>::FOLLOW_UP_LIMIT + 1,
            "the source update plus exactly the limit of follow-ups"
        );
    }

    #[test]
    fn an_update_error_paints_nothing_and_the_session_continues() {
        let mut app = started(Counter::default());
        let error = app.handle(key(b'e'), ms(1)).expect_err("callback");
        assert!(matches!(error, Error::Callback(_)));
        assert_eq!(app.state(), LifecycleState::Running);
        assert!(!app.handle(key(b'+'), ms(2)).expect("update").is_empty());
    }

    #[test]
    fn a_view_error_aborts_the_frame() {
        let mut app = started(Counter {
            count: -5,
            ..Counter::default()
        });
        let error = app.handle(key(b'-'), ms(1)).expect_err("view fails");
        assert!(matches!(error, Error::Render(_)));
        let first_view_failure = App::new(Counter {
            count: -9,
            ..Counter::default()
        })
        .start(&caps(), (4, 1), ms(0));
        assert!(matches!(first_view_failure, Err(Error::Render(_))));
    }

    #[test]
    fn a_resize_repaints_at_the_new_size_even_when_ignored() {
        let mut app = started(Counter::default());
        let frame = app
            .handle(InputEvent::Resize { cols: 9, rows: 1 }, ms(1))
            .expect("resize");
        assert!(frame.iter().any(|command| matches!(
            command,
            OutputCommand::PrintSegment { text } if text.contains("0 9x1")
        )));
        assert!(app.model().seen[0].contains("Resize"));
    }

    #[test]
    fn other_input_reaches_the_model_as_dispatch_events() {
        let mut app = started(Counter::default());
        let mouse = MouseEvent::new(MouseKind::Down, MouseButton::Left, 1, 1);
        app.handle(InputEvent::Mouse(mouse), ms(1)).expect("mouse");
        app.handle(InputEvent::Focus(true), ms(2)).expect("focus");
        app.handle(InputEvent::Paste("hi".to_string()), ms(3))
            .expect("paste");
        let tab = KeyEvent::special(SpecialKey::Tab, KeyModifiers::NONE, KeyEncoding::Legacy);
        app.handle(InputEvent::Key(tab), ms(4)).expect("tab");
        let seen = &app.model().seen;
        assert!(seen[0].starts_with("Mouse"));
        assert_eq!(seen[1], "TerminalFocus(true)");
        assert_eq!(seen[2], "Paste(\"hi\")");
        assert!(seen[3].starts_with("Key"));
    }

    #[test]
    fn lifecycle_misuse_fails_loudly() {
        let mut app = App::new(Counter::default());
        assert!(matches!(
            app.handle(key(b'+'), ms(0)),
            Err(Error::Lifecycle(_))
        ));
        assert!(matches!(
            app.start(&caps(), (0, 3), ms(0)),
            Err(Error::Render(_))
        ));
        app.start(&caps(), (4, 1), ms(0)).expect("starts");
        assert!(matches!(
            app.start(&caps(), (4, 1), ms(0)),
            Err(Error::Lifecycle(_))
        ));
    }

    #[test]
    fn the_same_script_gives_the_same_stream() {
        let script = [key(b'+'), key(b'+'), key(b'x'), key(b'-'), key(b'd')];
        let run = || {
            let mut app = App::new(Counter::default());
            let mut stream = app.start(&caps(), (12, 2), ms(0)).expect("first frame");
            for (step, input) in (1u64..).zip(script.iter().cloned()) {
                stream.extend(app.handle(input, ms(step * 16)).expect("update"));
            }
            stream
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn view_receives_the_injected_context() {
        struct Clock;
        impl Model for Clock {
            type Message = ();
            fn update(&mut self, _event: Event, _effects: &mut Effects) -> Result<Outcome> {
                Ok(Outcome::repaint())
            }
            fn view(&self, cx: &Context) -> Result<View> {
                let tier = cx.capabilities().color.color;
                Ok(View::text(&format!("{}ms {tier:?}", cx.now().as_millis())))
            }
        }
        let mut app = App::new(Clock);
        app.start(&caps(), (20, 1), ms(0)).expect("first frame");
        let frame = app.handle(key(b'x'), ms(42)).expect("update");
        assert!(frame.iter().any(|command| matches!(
            command,
            OutputCommand::PrintSegment { text } if text.contains("42ms")
        )));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "update reported no change")]
    fn under_reporting_is_caught_in_debug_builds() {
        struct Liar {
            count: u32,
        }
        impl Model for Liar {
            type Message = ();
            fn update(&mut self, _event: Event, _effects: &mut Effects) -> Result<Outcome> {
                self.count += 1;
                Ok(Outcome::Ignored)
            }
            fn view(&self, _cx: &Context) -> Result<View> {
                Ok(View::text(&self.count.to_string()))
            }
        }
        let mut app = App::new(Liar { count: 0 });
        app.start(&caps(), (4, 1), ms(0)).expect("first frame");
        let _ = app.handle(key(b'x'), ms(1));
    }

    #[test]
    fn debug_output_names_the_model_type_and_state() {
        let app = App::new(Counter::default());
        let shown = format!("{app:?}");
        assert!(
            shown.contains("Counter") && shown.contains("Setup"),
            "{shown}"
        );
    }

    #[test]
    fn effects_queue_is_inspectable_for_model_tests() {
        let mut effects: Effects<Msg> = Effects::default();
        effects.send(Msg::Add(1));
        effects.send(Msg::Add(2));
        assert_eq!(effects.pending(), 2);
        assert_eq!(effects.next_message(), Some(Msg::Add(1)));
        assert!(!effects.quit_requested());
    }
}
