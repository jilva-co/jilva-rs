//! Deterministic, in-memory implementation of the complete [`Backend`] seam.
//!
//! Scripted core-owned [`InputEvent`] values enter a FIFO; successful flushes
//! record the complete [`OutputCommand`] values and batch boundaries. Typed
//! inspections preserve text, modifiers, key kinds, colours, and rectangles.
//! Description helpers remain available for readable diagnostics.
//!
//! Input added before `enter` is immediately ready. Input added during a
//! session becomes ready at the next successful [`poll`](Backend::poll) or
//! [`read`](Backend::read) observation. Poll reports source readiness,
//! so a source containing only gated or invalid units can produce `true`
//! followed by `Ok(None)`. Timeouts never sleep in this headless backend.
//!
//! Submission validates the entire batch before buffering. A flush attempt
//! discards pending output even when it fails: retrying must not replay an
//! unknown partially delivered prefix. This matches the real backend's
//! policy; successful capture remains separate from attempted output.
//!
//! Mode flags model setup and reverse-order restoration without a TTY.
//! Cleanup attempts every active mode and retains failed flags for retry,
//! including after partial initialisation. `Drop` follows the same failure
//! path and swallows errors. Successful teardown discards queued input and
//! pending output so sessions cannot inherit stale work.
//!
//! Failure rules support every operation, a one-shot schedule, and absolute
//! one-based call indices. [`TestDoubleBackend::fail_enter_partial`] and
//! [`TestDoubleBackend::fail_rollback_once`] exercise setup and rollback
//! separately. No threads, process environment, or terminal I/O are used.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use super::backend::{
    AttrSupport, Backend, BackgroundHint, ColorProfile, ColorTier, InputEvent, InputProfile,
    KeyCode, KeyEncoding, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseKind,
    OutputCommand, Rect, ResolvedAttributes, ResolvedColor, ScreenProfile, SpecialKey,
    TerminalCapabilities,
};

/// Maximum source units filtered by one non-blocking read call.
const READ_SOURCE_BUDGET: usize = 64;

// ---------------------------------------------------------------------------
// Failure injection.
// ---------------------------------------------------------------------------

/// Operation selector for failure injection.
///
/// Each variant names one fallible backend operation. Failures are
/// configured per operation independently (every call / next call once / a
/// specific 1-based call index) and reported as [`std::io::Error`]s carrying
/// the requested [`std::io::ErrorKind`]. Facade-side tests use the per-op
/// `fail_*` methods so they never need to name this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FailOp {
    /// [`enter`](Backend::enter).
    Enter,
    /// [`poll`](Backend::poll).
    Poll,
    /// [`read`](Backend::read).
    Read,
    /// [`submit`](Backend::submit).
    Submit,
    /// [`flush`](Backend::flush).
    Flush,
    /// [`leave`](Backend::leave).
    Leave,
}

impl FailOp {
    /// Stable operation name used in the call log and failure catalogue.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Poll => "poll",
            Self::Read => "read",
            Self::Submit => "submit",
            Self::Flush => "flush",
            Self::Leave => "leave",
        }
    }
}

/// When an injected failure fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailMode {
    /// Fire on every call while installed.
    Always,
    /// Fire on the next matching call only, then uninstall.
    Once,
    /// Fire on the given 1-based call index only, then uninstall.
    AtCall(u64),
}

/// One installed failure rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FailRule {
    /// Operation the rule applies to.
    op: FailOp,
    /// Error kind the failing call reports.
    kind: io::ErrorKind,
    /// Firing schedule.
    mode: FailMode,
}

/// Scripted input unit: a decodable event or an undecodable sequence.
///
/// Undecodable units model wire garbage: [`read`](Backend::read) skips them
/// (counted in [`skipped_input`](TestDoubleBackend::skipped_input),
/// debug-visible in the call log) and continues with the next unit — one
/// garbage sequence never poisons the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScriptUnit {
    /// A decodable input event.
    Event(InputEvent),
    /// An undecodable escape sequence.
    Garbage,
}

// ---------------------------------------------------------------------------
// Diagnostic descriptions. Assertions can use the complete typed values.
// ---------------------------------------------------------------------------

/// Stable one-line description of an input event.
///
/// Examples: `key("q")`, `key(Esc)`,
/// `mouse(press-left@3,4)`, `resize(100x30)`. Key descriptions cover the
/// code only; press/modifier/encoding travel in the typed value (asserted
/// by typed equality in-crate).
#[must_use]
pub fn describe_input(event: &InputEvent) -> String {
    match event {
        InputEvent::Key(key) => {
            let code = match &key.code {
                KeyCode::Text(text) => return format!("key({text:?})"),
                KeyCode::Special(SpecialKey::Enter) => "Enter",
                KeyCode::Special(SpecialKey::Tab) => "Tab",
                KeyCode::Special(SpecialKey::BackTab) => "BackTab",
                KeyCode::Special(SpecialKey::Backspace) => "Backspace",
                KeyCode::Special(SpecialKey::Escape) => "Esc",
                KeyCode::Special(SpecialKey::Up) => "Up",
                KeyCode::Special(SpecialKey::Down) => "Down",
                KeyCode::Special(SpecialKey::Left) => "Left",
                KeyCode::Special(SpecialKey::Right) => "Right",
                KeyCode::Special(SpecialKey::Home) => "Home",
                KeyCode::Special(SpecialKey::End) => "End",
                KeyCode::Special(SpecialKey::PageUp) => "PageUp",
                KeyCode::Special(SpecialKey::PageDown) => "PageDown",
                KeyCode::Special(SpecialKey::Delete) => "Delete",
                KeyCode::Special(SpecialKey::Insert) => "Insert",
                KeyCode::Function(number) => return format!("key(F{number})"),
                _ => "?",
            };
            format!("key({code})")
        }
        InputEvent::Mouse(mouse) => {
            let kind = match (&mouse.kind, &mouse.button) {
                (MouseKind::Down, MouseButton::Left) => "press-left",
                (MouseKind::Down, MouseButton::Right) => "press-right",
                (MouseKind::Down, MouseButton::Middle) => "press-middle",
                (MouseKind::Up, MouseButton::Left) => "release-left",
                (MouseKind::Up, MouseButton::Right) => "release-right",
                (MouseKind::Up, MouseButton::Middle) => "release-middle",
                (MouseKind::Drag, MouseButton::Left) => "drag-left",
                (MouseKind::Drag, MouseButton::Right) => "drag-right",
                (MouseKind::Drag, MouseButton::Middle) => "drag-middle",
                (MouseKind::Move, _) => "move",
                (MouseKind::ScrollUp, _) => "scroll-up",
                (MouseKind::ScrollDown, _) => "scroll-down",
                (MouseKind::ScrollLeft, _) => "scroll-left",
                (MouseKind::ScrollRight, _) => "scroll-right",
                _ => "?",
            };
            format!("mouse({kind}@{},{})", mouse.cols, mouse.rows)
        }
        InputEvent::Resize { cols, rows } => format!("resize({cols}x{rows})"),
        InputEvent::Focus(focused) => format!("focus({focused})"),
        InputEvent::Paste(text) => format!("paste({text:?})"),
        _ => "unknown-input".to_string(),
    }
}

/// Stable one-line description of an output command.
///
/// Format is part of this double's test surface: `move(3,4)`,
/// `print("hi")`, `attrs(bold+fg)`, `reset`, `hide-cursor`, `show-cursor`,
/// `clear(0,0,10,5)`.
#[must_use]
pub fn describe_command(command: &OutputCommand) -> String {
    match command {
        OutputCommand::MoveTo { x, y } => format!("move({x},{y})"),
        OutputCommand::PrintSegment { text } => format!("print({text:?})"),
        OutputCommand::SetAttributes(attrs) => describe_attributes(attrs),
        OutputCommand::ResetAttributes => "reset".to_string(),
        OutputCommand::HideCursor => "hide-cursor".to_string(),
        OutputCommand::ShowCursor => "show-cursor".to_string(),
        OutputCommand::ClearRegion { rect } => {
            format!(
                "clear({},{},{},{})",
                rect.x, rect.y, rect.width, rect.height
            )
        }
        _ => "unknown-command".to_string(),
    }
}

/// Stable description of resolved attributes: `attrs(plain)` or flag sets
/// like `attrs(bold+fg)` (`fg`/`bg` mark a non-default colour layer).
fn describe_attributes(attrs: &ResolvedAttributes) -> String {
    let mut flags = Vec::new();
    if attrs.fg != ResolvedColor::Default {
        flags.push("fg");
    }
    if attrs.bg != ResolvedColor::Default {
        flags.push("bg");
    }
    if attrs.bold {
        flags.push("bold");
    }
    if attrs.dim {
        flags.push("dim");
    }
    if attrs.italic {
        flags.push("italic");
    }
    if attrs.underline {
        flags.push("underline");
    }
    if attrs.reverse {
        flags.push("reverse");
    }
    if attrs.strikethrough {
        flags.push("strikethrough");
    }
    if flags.is_empty() {
        return "attrs(plain)".to_string();
    }
    format!("attrs({})", flags.join("+"))
}

// ---------------------------------------------------------------------------
// The double.
// ---------------------------------------------------------------------------

/// Terminal modes tracked independently so failed restoration can be retried.
#[derive(Debug, Default)]
struct SessionModes {
    raw: bool,
    alternate_screen: bool,
    mouse: bool,
    focus: bool,
    paste: bool,
    enhanced: bool,
}

impl SessionModes {
    fn is_clear(&self) -> bool {
        !self.raw
            && !self.alternate_screen
            && !self.mouse
            && !self.focus
            && !self.paste
            && !self.enhanced
    }
}

/// In-memory [`Backend`] stand-in: no TTY, no syscalls, fully scriptable.
///
/// Constructed with an explicit [`TerminalCapabilities`] snapshot so tests
/// exercise each degradation tier deterministically, plus an explicit size
/// (default 80×24). Script input with [`script`](Self::script) (or the `*_cmd`
/// constructors below), drive the session (`enter`/`leave` plus
/// `size`/`poll`/`read`/`submit`/`flush`/`capabilities` through the
/// [`Backend`] trait), then assert against the inspection APIs.
#[derive(Debug)]
pub struct TestDoubleBackend {
    /// Whether the backend is currently entered.
    entered: bool,
    /// The injected capability snapshot, unchanged by session observations.
    profile: TerminalCapabilities,
    /// Cached session snapshot, updated only at enter and consumed resize.
    snapshot: TerminalCapabilities,
    /// Active setup stages, retained independently when cleanup fails.
    modes: SessionModes,
    /// Current terminal size in cells (columns, rows).
    size: (u16, u16),
    /// Ordered record of `enter`/`leave` calls for assertions (kept shape).
    log: Vec<String>,
    /// Ordered record of every mutating call with outcomes.
    call_log: Vec<String>,
    /// Scripted input queue (FIFO).
    script: VecDeque<ScriptUnit>,
    /// Input added during a session; promoted at the next successful poll/read.
    staged_script: VecDeque<ScriptUnit>,
    /// Complete delivered input in consumption order.
    consumed: Vec<InputEvent>,
    /// Submitted-but-unflushed output buffer (FIFO, verbatim).
    buffer: Vec<OutputCommand>,
    /// Complete successfully flushed output in commit order.
    captured: Vec<OutputCommand>,
    /// Per-flush committed groups (flush-boundary markers).
    batches: Vec<Vec<OutputCommand>>,
    /// Skipped input units (mouse-gated + undecodable), debug-visible.
    skipped: usize,
    /// Successful `flush` commits.
    flush_count: usize,
    /// `leave` invocations including best-effort `Drop` attempts.
    restorations: usize,
    /// Pending partial-initialisation failure (consumed once).
    partial_enter: Option<io::ErrorKind>,
    /// One-shot restoration failure during partial-enter rollback.
    rollback_failure: Option<io::ErrorKind>,
    /// Installed failure rules.
    failures: Vec<FailRule>,
    /// Per-operation call counters (1-based indices for `AtCall` rules).
    enter_calls: u64,
    /// Per-operation call counters (see `enter_calls`).
    poll_calls: u64,
    /// Per-operation call counters (see `enter_calls`).
    read_calls: u64,
    /// Per-operation call counters (see `enter_calls`).
    submit_calls: u64,
    /// Per-operation call counters (see `enter_calls`).
    flush_calls: u64,
    /// Per-operation call counters (see `enter_calls`).
    leave_calls: u64,
}

impl Default for TestDoubleBackend {
    /// Returns a double with the conservative snapshot at 80×24, unentered.
    fn default() -> Self {
        Self::new(TerminalCapabilities::conservative(0))
    }
}

impl TestDoubleBackend {
    /// Creates a test double reporting the given capability snapshot at 80×24.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jilva_terminal::{TerminalCapabilities, TestDoubleBackend};
    ///
    /// let backend = TestDoubleBackend::new(TerminalCapabilities::conservative(0));
    /// assert!(!backend.is_entered());
    /// ```
    #[must_use]
    pub fn new(profile: TerminalCapabilities) -> Self {
        let mut snapshot = profile;
        snapshot.snapshot_id = 0;
        Self {
            entered: false,
            profile,
            snapshot,
            modes: SessionModes::default(),
            size: (80, 24),
            log: Vec::new(),
            call_log: Vec::new(),
            script: VecDeque::new(),
            staged_script: VecDeque::new(),
            consumed: Vec::new(),
            buffer: Vec::new(),
            captured: Vec::new(),
            batches: Vec::new(),
            skipped: 0,
            flush_count: 0,
            restorations: 0,
            partial_enter: None,
            rollback_failure: None,
            failures: Vec::new(),
            enter_calls: 0,
            poll_calls: 0,
            read_calls: 0,
            submit_calls: 0,
            flush_calls: 0,
            leave_calls: 0,
        }
    }

    /// Creates a double with an explicit size in cells (columns, rows).
    ///
    /// Degenerate sizes are stored as given and rejected by
    /// [`enter`](Backend::enter) before enabling any mode.
    #[must_use]
    pub fn with_size(profile: TerminalCapabilities, cols: u16, rows: u16) -> Self {
        let mut backend = Self::new(profile);
        backend.size = (cols, rows);
        backend
    }

    /// Creates a double pre-loaded with a scripted input queue (FIFO).
    #[must_use]
    pub fn with_script(profile: TerminalCapabilities, script: Vec<InputEvent>) -> Self {
        let mut backend = Self::new(profile);
        backend.script_all(script);
        backend
    }

    // -- Associated constructors (facade-clean scripting) -----------------

    /// A best-case modern-terminal snapshot for tests: truecolor, mouse,
    /// alternate screen (legacy key encoding, unknown background).
    #[must_use]
    pub fn full_profile() -> TerminalCapabilities {
        TerminalCapabilities::new(
            ColorProfile::new(
                ColorTier::Truecolor,
                AttrSupport::all(),
                BackgroundHint::Unknown,
            ),
            InputProfile::new(KeyEncoding::Legacy, true, false),
            ScreenProfile::new(true, true),
            false,
            1,
        )
    }

    /// Builds the legacy printable-character key event (no modifiers).
    ///
    /// A `char` always converts to a non-empty cluster string, so the
    /// conversion below is infallible by construction.
    #[must_use]
    pub fn key(character: char) -> InputEvent {
        InputEvent::Key(
            KeyEvent::text(
                character.to_string(),
                KeyModifiers::NONE,
                KeyEncoding::Legacy,
            )
            .expect("a char converts to a non-empty cluster string"),
        )
    }

    /// Builds the legacy Escape key event.
    #[must_use]
    pub const fn esc() -> InputEvent {
        InputEvent::Key(KeyEvent::special(
            SpecialKey::Escape,
            KeyModifiers::NONE,
            KeyEncoding::Legacy,
        ))
    }

    /// Builds the legacy Enter key event.
    #[must_use]
    pub const fn enter_key() -> InputEvent {
        InputEvent::Key(KeyEvent::special(
            SpecialKey::Enter,
            KeyModifiers::NONE,
            KeyEncoding::Legacy,
        ))
    }

    /// Builds a resize event, or `None` for degenerate sizes.
    ///
    /// Delegates to [`InputEvent::resize`]: a zero width or height is never
    /// a valid resize event (mirrors the `size` non-degenerate rule); the
    /// backend would drop such a report counted and debug-visible instead
    /// of emitting it.
    #[must_use]
    pub const fn resize_event(cols: u16, rows: u16) -> Option<InputEvent> {
        InputEvent::resize(cols, rows)
    }

    /// Builds a left-button click at the given 0-based cell.
    #[must_use]
    pub const fn mouse_click(x: u16, y: u16) -> InputEvent {
        InputEvent::Mouse(MouseEvent::new(MouseKind::Down, MouseButton::Left, x, y))
    }

    /// Builds a left-button drag at the given 0-based cell.
    ///
    /// Scripted test data (left button is the common case), not a claim
    /// about a real terminal.
    #[must_use]
    pub const fn mouse_drag(x: u16, y: u16) -> InputEvent {
        InputEvent::Mouse(MouseEvent::new(MouseKind::Drag, MouseButton::Left, x, y))
    }

    /// Builds a vertical scroll at the given 0-based cell.
    ///
    /// `up` selects scroll-up (`true`) or scroll-down (`false`).
    #[must_use]
    pub const fn mouse_scroll(x: u16, y: u16, up: bool) -> InputEvent {
        InputEvent::Mouse(MouseEvent::new(
            if up {
                MouseKind::ScrollUp
            } else {
                MouseKind::ScrollDown
            },
            MouseButton::None,
            x,
            y,
        ))
    }

    /// Builds a cursor-move output command.
    #[must_use]
    pub const fn move_cmd(x: u16, y: u16) -> OutputCommand {
        OutputCommand::MoveTo { x, y }
    }

    /// Builds a text-print output command.
    #[must_use]
    pub fn print_cmd(text: &str) -> OutputCommand {
        OutputCommand::PrintSegment {
            text: text.to_string(),
        }
    }

    /// Builds a set-attributes output command.
    #[must_use]
    pub const fn attrs_cmd(attrs: ResolvedAttributes) -> OutputCommand {
        OutputCommand::SetAttributes(attrs)
    }

    /// Builds plain (terminal-default) resolved attributes.
    #[must_use]
    pub const fn plain_attrs() -> ResolvedAttributes {
        ResolvedAttributes::plain()
    }

    /// Builds bold-only resolved attributes.
    #[must_use]
    pub const fn bold_attrs() -> ResolvedAttributes {
        ResolvedAttributes::plain().with_bold()
    }

    /// Builds a reset-attributes output command.
    #[must_use]
    pub const fn reset_cmd() -> OutputCommand {
        OutputCommand::ResetAttributes
    }

    /// Builds a hide-cursor output command.
    #[must_use]
    pub const fn hide_cursor_cmd() -> OutputCommand {
        OutputCommand::HideCursor
    }

    /// Builds a show-cursor output command.
    #[must_use]
    pub const fn show_cursor_cmd() -> OutputCommand {
        OutputCommand::ShowCursor
    }

    /// Builds a clear-region output command over explicit cells.
    #[must_use]
    pub const fn clear_cmd(x: u16, y: u16, width: u16, height: u16) -> OutputCommand {
        OutputCommand::ClearRegion {
            rect: Rect::new(x, y, width, height),
        }
    }

    // -- Scripting ----------------------------------------------------------

    /// Appends one event to the scripted input queue (FIFO).
    ///
    /// While entered, new units remain staged until the next successful
    /// [`poll`](Backend::poll) or [`read`](Backend::read) observation.
    pub fn script(&mut self, event: InputEvent) {
        self.push_unit(ScriptUnit::Event(event));
    }

    /// Appends many events to the scripted input queue, in order.
    pub fn script_all(&mut self, events: Vec<InputEvent>) {
        for event in events {
            self.script(event);
        }
    }

    /// Scripts one undecodable escape sequence.
    ///
    /// [`read`](Backend::read) skips it (counted in
    /// [`skipped_input`](Self::skipped_input)) and continues with the next
    /// unit — one garbage sequence never poisons the stream.
    pub fn push_garbage(&mut self) {
        self.push_unit(ScriptUnit::Garbage);
    }

    fn push_unit(&mut self, unit: ScriptUnit) {
        if self.entered {
            self.staged_script.push_back(unit);
        } else {
            self.script.push_back(unit);
        }
    }

    /// Replaces the configured terminal size (takes effect immediately).
    ///
    /// Degenerate values are stored; [`size`](Backend::size) reports them as
    /// an error rather than returning `(0, _)`.
    pub fn set_size(&mut self, cols: u16, rows: u16) {
        self.size = (cols, rows);
    }

    /// Discards the scripted input queue without consuming it.
    pub fn clear_script(&mut self) {
        self.script.clear();
        self.staged_script.clear();
    }

    // -- Failure injection --------------------------------------------------

    /// Fails every future call of `op` with `kind`.
    pub fn inject(&mut self, op: FailOp, kind: io::ErrorKind) {
        self.failures.push(FailRule {
            op,
            kind,
            mode: FailMode::Always,
        });
    }

    /// Fails the next call of `op` with `kind`, then uninstalls.
    pub fn inject_once(&mut self, op: FailOp, kind: io::ErrorKind) {
        self.failures.push(FailRule {
            op,
            kind,
            mode: FailMode::Once,
        });
    }

    /// Fails the given 1-based call of `op` with `kind`, then uninstalls.
    pub fn inject_at(&mut self, op: FailOp, call: u64, kind: io::ErrorKind) {
        self.failures.push(FailRule {
            op,
            kind,
            mode: FailMode::AtCall(call),
        });
    }

    /// Fails every future `enter` with `kind`.
    pub fn fail_enter(&mut self, kind: io::ErrorKind) {
        self.inject(FailOp::Enter, kind);
    }

    /// Fails the next `enter` with `kind`.
    pub fn fail_enter_once(&mut self, kind: io::ErrorKind) {
        self.inject_once(FailOp::Enter, kind);
    }

    /// Arms a partial-initialisation failure for the next `enter`.
    ///
    /// Models failure after the requested setup stages completed. Only
    /// enabled modes are rolled back, in reverse setup order. No snapshot
    /// identity is minted. A failed rollback remains retryable by `leave`.
    pub fn fail_enter_partial(&mut self, kind: io::ErrorKind) {
        self.partial_enter = Some(kind);
    }

    /// Fails the first active restoration stage of the next partial enter.
    ///
    /// Remaining stages are still attempted. The failed flag remains set;
    /// `leave` and `Drop` retry it, and `enter` refuses a dirty session.
    pub fn fail_rollback_once(&mut self, kind: io::ErrorKind) {
        self.rollback_failure = Some(kind);
    }

    /// Fails every future `poll` with `kind`.
    pub fn fail_poll(&mut self, kind: io::ErrorKind) {
        self.inject(FailOp::Poll, kind);
    }

    /// Fails the next `poll` with `kind`.
    pub fn fail_poll_once(&mut self, kind: io::ErrorKind) {
        self.inject_once(FailOp::Poll, kind);
    }

    /// Fails every future `read` with `kind`.
    pub fn fail_read(&mut self, kind: io::ErrorKind) {
        self.inject(FailOp::Read, kind);
    }

    /// Fails the next `read` with `kind`.
    pub fn fail_read_once(&mut self, kind: io::ErrorKind) {
        self.inject_once(FailOp::Read, kind);
    }

    /// Fails the given 1-based `read` call with `kind`.
    pub fn fail_read_at(&mut self, call: u64, kind: io::ErrorKind) {
        self.inject_at(FailOp::Read, call, kind);
    }

    /// Fails every future `submit` with `kind` (buffer untouched).
    pub fn fail_submit(&mut self, kind: io::ErrorKind) {
        self.inject(FailOp::Submit, kind);
    }

    /// Fails the next `submit` with `kind` (buffer untouched).
    pub fn fail_submit_once(&mut self, kind: io::ErrorKind) {
        self.inject_once(FailOp::Submit, kind);
    }

    /// Fails every future `flush` with `kind` (pending output discarded).
    pub fn fail_flush(&mut self, kind: io::ErrorKind) {
        self.inject(FailOp::Flush, kind);
    }

    /// Fails the next `flush` with `kind` (pending output discarded).
    pub fn fail_flush_once(&mut self, kind: io::ErrorKind) {
        self.inject_once(FailOp::Flush, kind);
    }

    /// Fails the given 1-based `flush` call with `kind` (pending output discarded).
    pub fn fail_flush_at(&mut self, call: u64, kind: io::ErrorKind) {
        self.inject_at(FailOp::Flush, call, kind);
    }

    /// Fails the first active cleanup stage of each future `leave`.
    ///
    /// Other stages are still restored; the failed flag remains retryable.
    pub fn fail_leave(&mut self, kind: io::ErrorKind) {
        self.inject(FailOp::Leave, kind);
    }

    /// Fails the first active cleanup stage of the next `leave`.
    pub fn fail_leave_once(&mut self, kind: io::ErrorKind) {
        self.inject_once(FailOp::Leave, kind);
    }

    /// Removes all installed failure rules and any armed partial-init failure.
    pub fn clear_failures(&mut self) {
        self.failures.clear();
        self.partial_enter = None;
        self.rollback_failure = None;
    }

    /// Removes installed failure rules for one operation.
    pub fn clear_failure(&mut self, op: FailOp) {
        self.failures.retain(|rule| rule.op != op);
        if op == FailOp::Enter {
            self.partial_enter = None;
            self.rollback_failure = None;
        }
    }

    /// Returns the installed failure rules as stable strings for assertions.
    ///
    /// Format: `flush×@3: BrokenPipe` (call-index rule), `read×once:
    /// ConnectionReset` (next-call rule), `flush×always: Other`
    /// (every-call rule), `enter:partial-init: Other` (armed partial mode).
    #[must_use]
    pub fn failures_configured(&self) -> Vec<String> {
        let mut configured: Vec<String> = self
            .failures
            .iter()
            .map(|rule| {
                let schedule = match rule.mode {
                    FailMode::Always => "always".to_string(),
                    FailMode::Once => "once".to_string(),
                    FailMode::AtCall(call) => format!("@{call}"),
                };
                format!("{}×{}: {:?}", rule.op.name(), schedule, rule.kind)
            })
            .collect();
        if let Some(kind) = self.partial_enter {
            configured.push(format!("enter:partial-init: {kind:?}"));
        }
        if let Some(kind) = self.rollback_failure {
            configured.push(format!("enter:rollback-once: {kind:?}"));
        }
        configured
    }

    /// Returns the call count for one operation.
    #[must_use]
    pub const fn op_calls(&self, op: FailOp) -> u64 {
        match op {
            FailOp::Enter => self.enter_calls,
            FailOp::Poll => self.poll_calls,
            FailOp::Read => self.read_calls,
            FailOp::Submit => self.submit_calls,
            FailOp::Flush => self.flush_calls,
            FailOp::Leave => self.leave_calls,
        }
    }

    // -- Inspection -----------------------------------------------------------

    /// Returns `true` while the backend is entered (between `enter` and `leave`).
    #[must_use]
    pub const fn is_entered(&self) -> bool {
        self.entered
    }

    /// Whether the modeled raw-mode setup still needs restoration.
    #[must_use]
    pub const fn raw_mode_active(&self) -> bool {
        self.modes.raw
    }

    /// Whether the modeled alternate screen still needs restoration.
    #[must_use]
    pub const fn alternate_screen_active(&self) -> bool {
        self.modes.alternate_screen
    }

    /// Whether the modeled mouse capture still needs restoration.
    #[must_use]
    pub const fn mouse_capture_active(&self) -> bool {
        self.modes.mouse
    }

    /// Whether the modeled focus reporting still needs restoration.
    #[must_use]
    pub const fn focus_reporting_active(&self) -> bool {
        self.modes.focus
    }

    /// Whether the modeled bracketed paste still needs restoration.
    #[must_use]
    pub const fn bracketed_paste_active(&self) -> bool {
        self.modes.paste
    }

    /// Whether the modeled enhanced keyboard flags still need restoration.
    #[must_use]
    pub const fn keyboard_enhancement_active(&self) -> bool {
        self.modes.enhanced
    }

    /// Returns the capability profile this double reports.
    #[must_use]
    pub const fn profile(&self) -> TerminalCapabilities {
        self.profile
    }

    /// Returns the ordered record of `enter`/`leave` calls.
    #[must_use]
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Returns the ordered record of every mutating call with its outcome.
    ///
    /// Entries look like `enter`, `poll→true`, `read→key("q")`,
    /// `read→skipped:mouse-disabled`, `submit×2`, `flush:committed×2`,
    /// `flush:failed(BrokenPipe)`, `leave`, plus `rollback:*` lines for
    /// partial-initialisation failures.
    #[must_use]
    pub fn call_log(&self) -> Vec<String> {
        self.call_log.clone()
    }

    /// Returns delivered input in consumption order (stable descriptions —
    /// see [`describe_input`]).
    #[must_use]
    pub fn consumed(&self) -> Vec<String> {
        self.consumed.iter().map(describe_input).collect()
    }

    /// Complete delivered events, retaining text, kinds, and modifiers.
    #[must_use]
    pub fn consumed_events(&self) -> &[InputEvent] {
        &self.consumed
    }

    /// Returns the most recently delivered input description, if any.
    #[must_use]
    pub fn last_consumed(&self) -> Option<String> {
        self.consumed.last().map(describe_input)
    }

    /// Returns flushed output in commit order (stable descriptions — see
    /// [`describe_command`]).
    #[must_use]
    pub fn captured(&self) -> Vec<String> {
        self.captured.iter().map(describe_command).collect()
    }

    /// Complete successfully flushed commands, including resolved payloads.
    #[must_use]
    pub fn captured_commands(&self) -> &[OutputCommand] {
        &self.captured
    }

    /// Returns the committed command groups, one entry per successful flush.
    #[must_use]
    pub fn flushed_batches(&self) -> Vec<Vec<String>> {
        self.batches
            .iter()
            .map(|batch| batch.iter().map(describe_command).collect())
            .collect()
    }

    /// Complete command groups, one per successful flush (including empty).
    #[must_use]
    pub fn flushed_command_batches(&self) -> &[Vec<OutputCommand>] {
        &self.batches
    }

    /// Returns the number of scripted input units still queued.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.script.len() + self.staged_script.len()
    }

    /// Returns the number of submitted-but-unflushed commands buffered.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    /// Complete pending output that has not had a flush attempt.
    #[must_use]
    pub fn buffered_commands(&self) -> &[OutputCommand] {
        &self.buffer
    }

    /// Returns the number of successful `flush` commits.
    #[must_use]
    pub const fn flush_count(&self) -> usize {
        self.flush_count
    }

    /// Returns the number of skipped input units (mouse-gated + undecodable).
    #[must_use]
    pub const fn skipped_input(&self) -> usize {
        self.skipped
    }

    /// Returns every `leave` invocation, including best-effort `Drop` attempts.
    #[must_use]
    pub const fn restoration_attempts(&self) -> usize {
        self.restorations
    }

    /// Returns the configured size without the session/degeneracy gate.
    ///
    /// Unlike [`size`](Backend::size) this never fails; use it to assert what
    /// was configured separately from what the gate reports.
    #[must_use]
    pub const fn configured_size(&self) -> (u16, u16) {
        self.size
    }

    /// Returns the colour tier name of the cached snapshot: `truecolor`, `palette256`,
    /// `ansi16`, or `mono`.
    #[must_use]
    pub fn capability_tier_name(&self) -> &'static str {
        match self.snapshot.color.color {
            ColorTier::Truecolor => "truecolor",
            ColorTier::Palette256 => "palette256",
            ColorTier::Ansi16 => "ansi16",
            ColorTier::Mono => "mono",
            _ => "unknown",
        }
    }

    /// Returns whether the derived snapshot offers mouse reporting.
    #[must_use]
    pub fn mouse_offered(&self) -> bool {
        self.snapshot.input.mouse
    }

    /// Returns whether the derived snapshot offers the alternate screen.
    #[must_use]
    pub fn alternate_screen_offered(&self) -> bool {
        self.snapshot.screen.alternate_screen
    }

    /// Cached snapshot identity, advanced by successful enter and changed
    /// resize geometry; `0` before the first successful enter.
    #[must_use]
    pub fn snapshot_id(&self) -> u64 {
        self.snapshot.snapshot_id
    }

    /// Removes the first rule matching `op` at 1-based `call_index`,
    /// returning its error kind when one fires.
    fn take_matching(&mut self, op: FailOp, call_index: u64) -> Option<io::ErrorKind> {
        let position = self.failures.iter().position(|rule| {
            rule.op == op
                && match rule.mode {
                    FailMode::Always => true,
                    FailMode::Once => true,
                    FailMode::AtCall(call) => call == call_index,
                }
        });
        let position = position?;
        let rule = self.failures[position];
        match rule.mode {
            FailMode::Always => {}
            FailMode::Once | FailMode::AtCall(_) => {
                self.failures.remove(position);
            }
        }
        Some(rule.kind)
    }

    fn set_up_modes(&mut self) {
        self.modes = SessionModes {
            alternate_screen: self.profile.screen.alternate_screen,
            raw: true,
            mouse: self.profile.input.mouse,
            focus: self.profile.input.focus_events,
            paste: self.profile.input.bracketed_paste,
            enhanced: self.profile.input.key_encoding == KeyEncoding::Enhanced,
        };
    }

    /// Restores all active modes; an injected failure affects only the first.
    fn restore_modes(
        &mut self,
        prefix: &str,
        failure: Option<io::ErrorKind>,
    ) -> Option<io::ErrorKind> {
        let mut first_error = None;
        for (active, name) in [
            (&mut self.modes.enhanced, "enhancement-flags"),
            (&mut self.modes.paste, "bracketed-paste"),
            (&mut self.modes.focus, "focus-events"),
            (&mut self.modes.mouse, "mouse-capture"),
            (&mut self.modes.alternate_screen, "alternate-screen"),
            (&mut self.modes.raw, "raw-mode"),
        ] {
            if !*active {
                continue;
            }
            self.call_log.push(format!("{prefix}:{name}"));
            if first_error.is_none() && failure.is_some() {
                first_error = failure;
            } else {
                *active = false;
            }
        }
        first_error
    }

    /// Shared teardown, including actual failure injection on the Drop path.
    fn leave_inner(&mut self, from_drop: bool) -> io::Result<()> {
        self.leave_calls += 1;
        self.restorations += 1;
        if from_drop {
            self.call_log.push("leave:drop-guard".to_string());
        }
        if !self.entered && self.modes.is_clear() {
            // Idempotent: leaving a non-entered backend is Ok so Drop
            // guards and double-teardown paths are safe.
            self.call_log.push("leave:idempotent".to_string());
            return Ok(());
        }
        // Cleanup can restore only some modes before an I/O failure. The
        // terminal is no longer usable; dirty flags track retry ownership.
        self.entered = false;
        let failure = self.take_matching(FailOp::Leave, self.leave_calls);
        if let Some(kind) = self.restore_modes("restore", failure) {
            self.call_log.push(format!("leave:failed({kind:?})"));
            return Err(injected("leave", kind));
        }
        self.buffer.clear();
        self.clear_script();
        self.log.push("leave".to_string());
        self.call_log.push("leave".to_string());
        Ok(())
    }
}

impl Backend for TestDoubleBackend {
    /// Returns `"test-double"`.
    fn name(&self) -> &str {
        "test-double"
    }

    /// Enters terminal mode: marks the backend entered and records the call.
    ///
    /// One live session per instance: entering an entered backend is an
    /// error. An armed partial-initialisation failure logs the rollback
    /// steps, stays unentered, and reports the injected kind. An injected
    /// `Enter` failure is atomic: the entered flag is untouched.
    ///
    /// # Errors
    ///
    /// Returns an error when already entered or when an `Enter` failure
    /// (full or partial) is injected.
    fn enter(&mut self) -> io::Result<()> {
        self.enter_calls += 1;
        if self.entered {
            self.call_log.push("enter:already-entered".to_string());
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "test-double: enter: already entered",
            ));
        }
        if !self.modes.is_clear() {
            self.call_log.push("enter:cleanup-pending".to_string());
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "test-double: enter: cleanup pending (leave before re-entering)",
            ));
        }
        if self.size.0 == 0 || self.size.1 == 0 {
            self.call_log
                .push("enter:rejected(degenerate-size)".to_string());
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "test-double: enter: terminal size must have non-zero columns and rows",
            ));
        }
        if let Some(kind) = self.partial_enter.take() {
            self.set_up_modes();
            self.call_log
                .push(format!("enter:partial-failure({kind:?})"));
            let failure = self.rollback_failure.take();
            let cleanup_error = self.restore_modes("rollback", failure);
            if let Some(cleanup_kind) = cleanup_error {
                self.call_log
                    .push(format!("rollback:failed({cleanup_kind:?})"));
            }
            let message = match cleanup_error {
                Some(cleanup_kind) => format!(
                    "test-double: enter: partial initialisation ({kind:?}); rollback failed ({cleanup_kind:?}); retry leave"
                ),
                None => {
                    format!("test-double: enter: partial initialisation ({kind:?}); rolled back")
                }
            };
            return Err(io::Error::new(kind, message));
        }
        if let Some(kind) = self.take_matching(FailOp::Enter, self.enter_calls) {
            self.call_log.push(format!("enter:failed({kind:?})"));
            return Err(injected("enter", kind));
        }
        self.set_up_modes();
        let mut fresh = self.profile;
        if let Some(screen) = fresh.screen.with_size(self.size.0, self.size.1) {
            fresh.screen = screen;
        }
        let id = self.snapshot.snapshot_id.saturating_add(1);
        self.snapshot = if self.snapshot.snapshot_id == 0 {
            fresh.snapshot_id = id;
            fresh
        } else {
            self.snapshot.refreshed(
                fresh.color,
                fresh.input,
                fresh.screen,
                fresh.reduced_motion,
                id,
            )
        };
        self.buffer.clear();
        self.entered = true;
        self.log.push("enter".to_string());
        self.call_log.push("enter".to_string());
        Ok(())
    }

    /// Restores the terminal, undoing everything [`Backend::enter`] did.
    ///
    /// Idempotent: leaving a non-entered backend is `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns an error when a `Leave` failure is injected (the session
    /// stays entered so a retry can still restore it).
    fn leave(&mut self) -> io::Result<()> {
        self.leave_inner(false)
    }

    /// Returns the current terminal size in cells.
    ///
    /// # Errors
    ///
    /// Returns an error with no live session (fail-loud, never operate on a
    /// dead terminal) or when the configured size is degenerate — a zero
    /// width or height is reported as an error, never as a size.
    fn size(&self) -> io::Result<(u16, u16)> {
        if !self.entered {
            return Err(no_session("size"));
        }
        let (cols, rows) = self.size;
        if cols == 0 || rows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "test-double: size: degenerate size (width and height must be non-zero)",
            ));
        }
        Ok((cols, rows))
    }

    /// Returns whether any input source unit is ready after staged promotion.
    ///
    /// Never blocks and never decodes: the headless double reports current
    /// queue state immediately for any `timeout` (including non-zero ones —
    /// there is no TTY to wait on, so waiting would only make tests flaky).
    ///
    /// # Errors
    ///
    /// Returns an error with no live session, or when a `Poll` failure is
    /// injected.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        let _ = timeout;
        self.poll_calls += 1;
        if !self.entered {
            self.call_log.push("poll:rejected(no-session)".to_string());
            return Err(no_session("poll"));
        }
        if let Some(kind) = self.take_matching(FailOp::Poll, self.poll_calls) {
            self.call_log.push(format!("poll:failed({kind:?})"));
            return Err(injected("poll", kind));
        }
        self.script.append(&mut self.staged_script);
        let pending = !self.script.is_empty();
        self.call_log.push(format!("poll→{pending}"));
        Ok(pending)
    }

    /// Drains ONE decoded input event, or `Ok(None)` when the queue is empty.
    ///
    /// `Ok(None)` is normal operation, never an error. Mouse reports are
    /// skipped (counted) when their input mode was not offered;
    /// undecodable sequences are skipped (counted). At most 64 source units
    /// are examined in one call; another poll/read continues the source.
    ///
    /// # Errors
    ///
    /// Returns an error with no live session, or when a `Read` failure is
    /// injected.
    fn read(&mut self) -> io::Result<Option<InputEvent>> {
        self.read_calls += 1;
        if !self.entered {
            self.call_log.push("read:rejected(no-session)".to_string());
            return Err(no_session("read"));
        }
        if let Some(kind) = self.take_matching(FailOp::Read, self.read_calls) {
            self.call_log.push(format!("read:failed({kind:?})"));
            return Err(injected("read", kind));
        }
        self.script.append(&mut self.staged_script);
        for _ in 0..READ_SOURCE_BUDGET {
            let Some(unit) = self.script.pop_front() else {
                break;
            };
            match unit {
                ScriptUnit::Garbage => {
                    self.skipped += 1;
                    self.call_log.push("read→skipped:undecodable".to_string());
                }
                ScriptUnit::Event(InputEvent::Mouse(_)) if !self.snapshot.input.mouse => {
                    self.skipped += 1;
                    self.call_log
                        .push("read→skipped:mouse-disabled".to_string());
                }
                ScriptUnit::Event(InputEvent::Focus(_)) if !self.snapshot.input.focus_events => {
                    self.skipped += 1;
                    self.call_log
                        .push("read→skipped:focus-disabled".to_string());
                }
                ScriptUnit::Event(InputEvent::Paste(_)) if !self.snapshot.input.bracketed_paste => {
                    self.skipped += 1;
                    self.call_log
                        .push("read→skipped:paste-disabled".to_string());
                }
                ScriptUnit::Event(InputEvent::Resize { cols, rows }) if cols == 0 || rows == 0 => {
                    self.skipped += 1;
                    self.call_log
                        .push("read→skipped:degenerate-resize".to_string());
                }
                ScriptUnit::Event(event) => {
                    if let InputEvent::Resize { cols, rows } = &event {
                        self.size = (*cols, *rows);
                        if let Some(snapshot) = self.snapshot.resized(*cols, *rows) {
                            self.snapshot = snapshot;
                        }
                    }
                    let description = describe_input(&event);
                    self.consumed.push(event.clone());
                    self.call_log.push(format!("read→{description}"));
                    return Ok(Some(event));
                }
            }
        }
        self.call_log.push("read→none".to_string());
        Ok(None)
    }

    /// Buffers output commands in order, without writing anywhere yet.
    ///
    /// Buffering is FIFO. A failed batch leaves the prior buffer untouched.
    ///
    /// # Errors
    ///
    /// Returns an error with no live session, or when a `Submit` failure is
    /// injected (the buffer is untouched in both cases).
    fn submit(&mut self, commands: &[OutputCommand]) -> io::Result<()> {
        self.submit_calls += 1;
        if !self.entered {
            self.call_log
                .push("submit:rejected(no-session)".to_string());
            return Err(no_session("submit"));
        }
        if let Some(kind) = self.take_matching(FailOp::Submit, self.submit_calls) {
            self.call_log.push(format!("submit:failed({kind:?})"));
            return Err(injected("submit", kind));
        }
        for command in commands {
            if let Err(error) = command.validate_for_size(self.size.0, self.size.1) {
                self.call_log
                    .push("submit:rejected(invalid-command)".to_string());
                return Err(io::Error::new(
                    error.kind(),
                    format!("test-double: submit: {error}"),
                ));
            }
        }
        self.buffer.extend_from_slice(commands);
        self.call_log.push(format!("submit×{}", commands.len()));
        Ok(())
    }

    /// Commits everything buffered by [`submit`](Backend::submit) to the capture.
    ///
    /// After `Ok(())` the buffer is empty and one batch marker is recorded
    /// (see [`flushed_command_batches`](Self::flushed_command_batches)).
    /// An attempted flush discards pending output, including on failure,
    /// matching the real backend's protection against replaying prefixes.
    /// An empty successful flush records an empty batch.
    ///
    /// # Errors
    ///
    /// Returns an error with no live session, or when a `Flush` failure is
    /// injected. A rejected call outside a session attempts no delivery.
    fn flush(&mut self) -> io::Result<()> {
        self.flush_calls += 1;
        if !self.entered {
            self.call_log.push("flush:rejected(no-session)".to_string());
            return Err(no_session("flush"));
        }
        let batch = std::mem::take(&mut self.buffer);
        if let Some(kind) = self.take_matching(FailOp::Flush, self.flush_calls) {
            self.call_log.push(format!("flush:failed({kind:?})"));
            return Err(injected("flush", kind));
        }
        for command in &batch {
            if let Err(error) = command.validate_for_size(self.size.0, self.size.1) {
                self.call_log
                    .push("flush:rejected(invalid-command)".to_string());
                return Err(io::Error::new(
                    error.kind(),
                    format!("test-double: flush: {error}"),
                ));
            }
        }
        self.call_log
            .push(format!("flush:committed×{}", batch.len()));
        self.captured.extend(batch.iter().cloned());
        self.batches.push(batch);
        self.flush_count += 1;
        Ok(())
    }

    /// Returns the cached capability snapshot taken at `enter`.
    ///
    /// Never re-probes. Valid consumed resizes update only the screen domain.
    fn capabilities(&self) -> TerminalCapabilities {
        self.snapshot
    }
}

impl Drop for TestDoubleBackend {
    /// Best-effort restoration: attempts `leave` on a live session.
    ///
    /// The error (if any) is swallowed and counted — `Drop` impls must
    /// never panic, so a failing best-effort `leave` here is recorded in
    /// the call log, never unwound.
    fn drop(&mut self) {
        if self.entered || !self.modes.is_clear() {
            let _ = self.leave_inner(true);
        }
    }
}

/// Builds the no-session error for `op`: backend name + operation + expectation.
fn no_session(op: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotConnected,
        format!("test-double: {op}: no live session (call enter first)"),
    )
}

/// Builds an injected-failure error for `op` carrying `kind`.
fn injected(op: &str, kind: io::ErrorKind) -> io::Error {
    io::Error::new(
        kind,
        format!("test-double: {op}: injected {kind:?} failure"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_modes() -> TerminalCapabilities {
        let mut caps = TestDoubleBackend::full_profile();
        caps.input = InputProfile::new(KeyEncoding::Enhanced, true, true).with_focus_events(true);
        caps
    }

    #[test]
    fn complete_setup_and_reverse_cleanup_change_mode_state() {
        let mut backend = TestDoubleBackend::new(all_modes());
        backend.enter().expect("enter succeeds");
        assert!(backend.raw_mode_active());
        assert!(backend.alternate_screen_active());
        assert!(backend.mouse_capture_active());
        assert!(backend.focus_reporting_active());
        assert!(backend.bracketed_paste_active());
        assert!(backend.keyboard_enhancement_active());
        backend.leave().expect("leave succeeds");
        assert!(backend.modes.is_clear());
        assert_eq!(backend.log(), ["enter", "leave"]);
        assert_eq!(
            &backend.call_log()[1..7],
            [
                "restore:enhancement-flags",
                "restore:bracketed-paste",
                "restore:focus-events",
                "restore:mouse-capture",
                "restore:alternate-screen",
                "restore:raw-mode",
            ]
        );
    }

    #[test]
    fn partial_setup_restores_only_enabled_modes() {
        let mut backend = TestDoubleBackend::default();
        backend.fail_enter_partial(io::ErrorKind::ConnectionRefused);
        assert_eq!(
            backend.enter().expect_err("partial setup fails").kind(),
            io::ErrorKind::ConnectionRefused
        );
        assert!(!backend.is_entered());
        assert!(backend.modes.is_clear());
        assert_eq!(backend.snapshot_id(), 0);
        assert_eq!(
            backend.call_log(),
            [
                "enter:partial-failure(ConnectionRefused)",
                "rollback:raw-mode"
            ]
        );
        backend.leave().expect("leave after rollback is idempotent");
        backend.enter().expect("setup can recover");
        assert_eq!(backend.snapshot_id(), 1);
    }

    #[test]
    fn rollback_failure_retains_only_the_failed_mode_for_retry() {
        let mut backend = TestDoubleBackend::new(all_modes());
        backend.fail_enter_partial(io::ErrorKind::PermissionDenied);
        backend.fail_rollback_once(io::ErrorKind::BrokenPipe);
        assert_eq!(
            backend
                .enter()
                .expect_err("setup failure is preserved")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(!backend.is_entered());
        assert!(backend.keyboard_enhancement_active());
        assert!(!backend.raw_mode_active());
        assert!(!backend.alternate_screen_active());
        assert!(!backend.mouse_capture_active());
        assert!(!backend.focus_reporting_active());
        assert!(!backend.bracketed_paste_active());
        assert!(backend.enter().is_err(), "dirty modes must block setup");
        backend.leave().expect("cleanup retries the remaining flag");
        assert!(backend.modes.is_clear());
        backend.enter().expect("setup works after cleanup");
    }

    #[test]
    fn drop_cleanup_uses_failure_injection_and_can_retry_dirty_setup() {
        let mut backend = TestDoubleBackend::default();
        backend.fail_enter_partial(io::ErrorKind::Other);
        backend.fail_rollback_once(io::ErrorKind::BrokenPipe);
        backend.enter().expect_err("partial setup fails");
        backend.fail_leave_once(io::ErrorKind::Interrupted);
        assert_eq!(
            backend
                .leave_inner(true)
                .expect_err("drop cleanup can fail")
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert_eq!(backend.restoration_attempts(), 1);
        assert!(
            backend.raw_mode_active(),
            "failed cleanup must remain retryable"
        );
        assert!(
            backend
                .call_log()
                .iter()
                .any(|line| line == "leave:drop-guard")
        );
        backend
            .leave_inner(true)
            .expect("one-shot failure is consumed");
        assert_eq!(backend.restoration_attempts(), 2);
        assert!(backend.modes.is_clear());
    }

    #[test]
    fn drop_never_unwinds_when_restoration_fails() {
        let mut backend = TestDoubleBackend::default();
        backend.enter().expect("enter succeeds");
        backend.fail_leave(io::ErrorKind::Other);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(backend))).is_ok());
    }

    #[test]
    fn indexed_read_failure_consumes_nothing_and_fires_once() {
        let events = vec![TestDoubleBackend::key('a'), TestDoubleBackend::key('b')];
        let mut backend =
            TestDoubleBackend::with_script(TerminalCapabilities::conservative(0), events.clone());
        backend.enter().expect("enter succeeds");
        backend.fail_read_at(2, io::ErrorKind::ConnectionReset);
        assert_eq!(
            backend.read().expect("first read succeeds"),
            Some(events[0].clone())
        );
        assert_eq!(
            backend.read().expect_err("second read fails").kind(),
            io::ErrorKind::ConnectionReset
        );
        assert_eq!(backend.pending(), 1);
        assert_eq!(
            backend.read().expect("third read recovers"),
            Some(events[1].clone())
        );
        assert_eq!(backend.consumed_events(), events);
        assert_eq!(backend.op_calls(FailOp::Read), 3);
        assert!(backend.failures_configured().is_empty());
    }

    #[test]
    fn clear_enter_failure_clears_partial_setup_and_rollback_rules() {
        let mut backend = TestDoubleBackend::default();
        backend.fail_enter(io::ErrorKind::Other);
        backend.fail_enter_partial(io::ErrorKind::PermissionDenied);
        backend.fail_rollback_once(io::ErrorKind::BrokenPipe);
        assert_eq!(backend.failures_configured().len(), 3);
        backend.clear_failure(FailOp::Enter);
        assert!(backend.failures_configured().is_empty());
        backend.enter().expect("all setup rules were cleared");
    }

    #[test]
    fn snapshot_identity_never_regresses_across_resize_and_reenter() {
        let mut backend = TestDoubleBackend::with_script(
            TestDoubleBackend::full_profile(),
            vec![InputEvent::Resize {
                cols: 100,
                rows: 30,
            }],
        );
        backend.enter().expect("enter succeeds");
        assert_eq!(backend.snapshot_id(), 1);
        backend.read().expect("resize succeeds");
        let resized = backend.capabilities();
        assert_eq!(resized.snapshot_id, 2);
        backend.leave().expect("leave succeeds");
        backend.enter().expect("next session succeeds");
        let next = backend.capabilities();
        assert_eq!(next.snapshot_id, 3);
        assert_eq!(next.screen.geometry, Some((100, 30)));
        assert_eq!(next.screen_version, resized.screen_version);
    }
}
