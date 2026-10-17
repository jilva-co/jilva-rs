//! Synchronous crossterm 0.29.0 backend with one owned terminal session.
//!
//! Production and headless fault tests use the same internal lifecycle state
//! machine. An attempted mode change is dirty before I/O starts: a failed
//! write can have delivered a prefix. Cleanup attempts every dirty mode in
//! reverse order, retains failed flags for retry, and preserves the primary
//! I/O error plus cleanup context. Drop retries pending restoration during
//! ordinary return and panic unwind, not process abort or forced termination.
//!
//! Crossterm owns process-global raw-mode/input machinery. A documented atomic
//! lease prevents two Jilva backends from using it at once; it contains no app
//! state and remains held until restoration completes. Entry rejects external
//! raw mode observable by crossterm, which cannot detect all other libraries'
//! out-of-band mutations. Cursor restoration establishes a visible baseline.
//!
//! Both standard handles must be terminals. Optional modes are explicit
//! requests filtered by a conservative host policy. Environment hints and
//! successful writes do not prove an emulator understands a protocol. Unknown
//! and `TERM=dumb` hosts get no optional setup. No keyboard enhancements are
//! negotiated. Capability snapshots and their identities belong to an instance.
//!
//! Complete output batches are validated before acceptance. A flush consumes
//! its batch on failure, since an unknown delivered prefix must not be replayed.
//! Real crossterm commands use an owned stdout handle, retaining native Windows
//! console behavior and ordering. Tests format ANSI directly without native I/O.

use std::fmt;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event as CrosstermEvent, KeyCode as CrosstermKeyCode,
    KeyEventKind as CrosstermKeyKind, KeyModifiers as CrosstermModifiers,
    MouseButton as CrosstermButton, MouseEventKind,
};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute};
#[cfg(windows)]
use crossterm::style::{Color as CrosstermColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{Command, QueueableCommand, event, execute, terminal};

use crate::backend::{
    Backend, ColorProfile, ColorTier, InputEvent, InputProfile, KeyEncoding, KeyEvent,
    KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseKind, OutputCommand, Rect,
    ResolvedAttributes, ResolvedColor, ScreenProfile, SpecialKey, TerminalCapabilities,
};
use crate::capabilities::{self, DetectionReport, EnvSnapshot};

/// Protects crossterm global terminal/input resources, not application state.
static TERMINAL_LEASE: AtomicBool = AtomicBool::new(false);
const READ_BUDGET: usize = 64;

/// Explicit requests for optional session modes.
///
/// Defaults request an alternate screen and hidden cursor. Mouse, focus, and
/// bracketed-paste reporting require explicit opt-in. A conservative host policy
/// filters requests, and capabilities advertise only established modes. Options
/// cannot change while a session is active. Prior cursor visibility is not saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendOptions {
    /// Request alternate-screen ownership on supported interactive hosts.
    pub alternate_screen: bool,
    /// Request a hidden cursor, restored to visible during cleanup.
    pub hide_cursor: bool,
    /// Request mouse capture and delivery of mouse input events.
    pub mouse_capture: bool,
    /// Request terminal focus reporting and delivery of focus input events.
    pub focus_events: bool,
    /// Request bracketed-paste reporting and delivery of untrusted paste text.
    pub bracketed_paste: bool,
}

impl Default for BackendOptions {
    fn default() -> Self {
        Self {
            alternate_screen: true,
            hide_cursor: true,
            mouse_capture: false,
            focus_events: false,
            bracketed_paste: false,
        }
    }
}

#[derive(Debug)]
struct OperationError {
    operation: &'static str,
    primary: io::Error,
    cleanup: Vec<io::Error>,
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "jilva backend 'crossterm' {}: {}",
            self.operation, self.primary
        )?;
        for error in &self.cleanup {
            write!(f, "; cleanup also failed: {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for OperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}

fn operation_error(
    operation: &'static str,
    primary: io::Error,
    cleanup: Vec<io::Error>,
) -> io::Error {
    io::Error::new(
        primary.kind(),
        OperationError {
            operation,
            primary,
            cleanup,
        },
    )
}

fn contextual(operation: &'static str, source: io::Error) -> io::Error {
    operation_error(operation, source, Vec::new())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Raw,
    Alternate,
    Cursor,
    Mouse,
    Focus,
    Paste,
    Attributes,
}

impl Mode {
    const fn index(self) -> usize {
        self as usize
    }
    const fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw mode",
            Self::Alternate => "alternate screen",
            Self::Cursor => "cursor visibility",
            Self::Mouse => "mouse capture",
            Self::Focus => "focus reporting",
            Self::Paste => "bracketed paste",
            Self::Attributes => "attributes",
        }
    }
}

const SETUP_ORDER: [Mode; 6] = [
    Mode::Raw,
    Mode::Alternate,
    Mode::Cursor,
    Mode::Mouse,
    Mode::Focus,
    Mode::Paste,
];
const RESTORE_ORDER: [Mode; 7] = [
    Mode::Attributes,
    Mode::Paste,
    Mode::Focus,
    Mode::Mouse,
    Mode::Cursor,
    Mode::Alternate,
    Mode::Raw,
];

#[derive(Debug, Clone, Copy)]
struct ModeSupport {
    alternate: bool,
    cursor: bool,
    mouse: bool,
    focus: bool,
    paste: bool,
}

// Compatibility hints, separate from the single shared color detector.
fn mode_support(env: &EnvSnapshot, native_windows: bool) -> ModeSupport {
    let term = env.term.as_deref().unwrap_or_default().to_ascii_lowercase();
    let dumb = term == "dumb";
    let modern = [
        "xterm",
        "screen",
        "tmux",
        "rxvt",
        "alacritty",
        "kitty",
        "wezterm",
        "foot",
        "cygwin",
        "st-",
    ]
    .iter()
    .any(|prefix| term.starts_with(prefix));
    let screen = !dumb && (native_windows || modern);
    ModeSupport {
        alternate: screen,
        cursor: screen || term == "linux",
        mouse: screen,
        focus: screen,
        // Legacy Windows has no paste API; require an ANSI terminal hint.
        paste: !dumb && modern,
    }
}

/// Every effect used by the production lifecycle is injectable in tests.
trait TerminalIo: fmt::Debug {
    fn input_is_tty(&self) -> bool;
    fn output_is_tty(&self) -> bool;
    fn environment(&self) -> EnvSnapshot;
    fn mode_support(&self, env: &EnvSnapshot) -> ModeSupport;
    fn raw_mode_enabled(&mut self) -> io::Result<bool>;
    fn size(&mut self) -> io::Result<(u16, u16)>;
    fn mode(&mut self, mode: Mode, enabled: bool) -> io::Result<()>;
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<CrosstermEvent>;
    fn write_batch(
        &mut self,
        commands: &[OutputCommand],
        geometry: (u16, u16),
        color: ColorProfile,
    ) -> io::Result<()>;
}

#[derive(Debug)]
struct CrosstermIo {
    input: io::Stdin,
    output: io::Stdout,
}

impl Default for CrosstermIo {
    fn default() -> Self {
        Self {
            input: io::stdin(),
            output: io::stdout(),
        }
    }
}

impl TerminalIo for CrosstermIo {
    fn input_is_tty(&self) -> bool {
        self.input.is_terminal()
    }
    fn output_is_tty(&self) -> bool {
        self.output.is_terminal()
    }
    fn environment(&self) -> EnvSnapshot {
        EnvSnapshot::from_process()
    }
    fn mode_support(&self, env: &EnvSnapshot) -> ModeSupport {
        mode_support(env, cfg!(windows))
    }
    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        terminal::is_raw_mode_enabled()
    }
    fn size(&mut self) -> io::Result<(u16, u16)> {
        terminal::size()
    }
    fn mode(&mut self, mode: Mode, enabled: bool) -> io::Result<()> {
        match (mode, enabled) {
            (Mode::Raw, true) => terminal::enable_raw_mode(),
            (Mode::Raw, false) => terminal::disable_raw_mode(),
            (Mode::Alternate, true) => execute!(self.output, EnterAlternateScreen),
            (Mode::Alternate, false) => execute!(self.output, LeaveAlternateScreen),
            (Mode::Cursor, true) => execute!(self.output, Hide),
            (Mode::Cursor, false) => execute!(self.output, Show),
            (Mode::Mouse, true) => execute!(self.output, EnableMouseCapture),
            (Mode::Mouse, false) => execute!(self.output, DisableMouseCapture),
            (Mode::Focus, true) => execute!(self.output, EnableFocusChange),
            (Mode::Focus, false) => execute!(self.output, DisableFocusChange),
            (Mode::Paste, true) => execute!(self.output, EnableBracketedPaste),
            (Mode::Paste, false) => execute!(self.output, DisableBracketedPaste),
            (Mode::Attributes, _) => {
                execute!(self.output, ResetColor, SetAttribute(Attribute::Reset))
            }
        }
    }
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }
    fn read(&mut self) -> io::Result<CrosstermEvent> {
        event::read()
    }
    fn write_batch(
        &mut self,
        commands: &[OutputCommand],
        geometry: (u16, u16),
        color: ColorProfile,
    ) -> io::Result<()> {
        // Use the real writer, not Vec: Windows commands may flush it and
        // call native console APIs immediately to preserve operation order.
        buffered_output(self.output.lock(), |writer| {
            encode_batch(&mut QueueSink(writer), commands, geometry, color)
        })
    }
}

fn buffered_output<W: Write>(
    target: W,
    encode: impl FnOnce(&mut BufWriter<W>) -> io::Result<()>,
) -> io::Result<()> {
    let mut writer = BufWriter::new(target);
    let result = encode(&mut writer).and_then(|()| writer.flush());
    // Drop must not retry buffered bytes after an unknown-prefix failure.
    let _discarded = writer.into_parts();
    result
}

#[derive(Debug)]
struct Session<'a, I: TerminalIo> {
    io: I,
    options: BackendOptions,
    lease: &'a AtomicBool,
    lease_held: bool,
    entered: bool,
    dirty: [bool; 7],
    profile: TerminalCapabilities,
    report: Option<DetectionReport>,
    pending: Vec<OutputCommand>,
    skipped_sequences: u64,
}

impl<'a, I: TerminalIo> Session<'a, I> {
    fn new(io: I, options: BackendOptions, lease: &'a AtomicBool) -> Self {
        Self {
            io,
            options,
            lease,
            lease_held: false,
            entered: false,
            dirty: [false; 7],
            profile: TerminalCapabilities::conservative(0),
            report: None,
            pending: Vec::new(),
            skipped_sequences: 0,
        }
    }

    fn require_entered(&self, operation: &'static str) -> io::Result<()> {
        if self.entered {
            Ok(())
        } else {
            Err(contextual(
                operation,
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    "an entered terminal session is required",
                ),
            ))
        }
    }

    fn release_if_clean(&mut self) {
        if self.lease_held && !self.dirty.iter().any(|&dirty| dirty) {
            self.lease.store(false, Ordering::Release);
            self.lease_held = false;
        }
    }

    fn restore(&mut self) -> Vec<io::Error> {
        self.entered = false;
        self.pending.clear();
        let mut failures = Vec::new();
        for mode in RESTORE_ORDER {
            if self.dirty[mode.index()] {
                match self.io.mode(mode, false) {
                    Ok(()) => self.dirty[mode.index()] = false,
                    Err(source) => failures.push(contextual(mode.name(), source)),
                }
            }
        }
        self.release_if_clean();
        failures
    }

    fn enter(&mut self) -> io::Result<()> {
        if self.entered {
            return Err(contextual(
                "enter",
                io::Error::new(io::ErrorKind::AlreadyExists, "session is already entered"),
            ));
        }
        if self.lease_held || self.dirty.iter().any(|&dirty| dirty) {
            return Err(contextual(
                "enter",
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "pending terminal restoration requires leave first",
                ),
            ));
        }
        if !self.io.input_is_tty() || !self.io.output_is_tty() {
            return Err(contextual(
                "enter",
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "both stdin and stdout must be terminals",
                ),
            ));
        }
        self.lease
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                contextual(
                    "enter",
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "another crossterm backend owns the terminal lease",
                    ),
                )
            })?;
        self.lease_held = true;
        let preflight = (|| {
            if self.io.raw_mode_enabled()? {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "raw mode is already active outside this session",
                ));
            }
            let size = self.io.size()?;
            if size.0 == 0 || size.1 == 0 {
                return Err(invalid("terminal size must have nonzero columns and rows"));
            }
            Ok(size)
        })();
        let geometry = match preflight {
            Ok(size) => size,
            Err(source) => {
                self.release_if_clean();
                return Err(contextual("enter preflight", source));
            }
        };
        let env = self.io.environment();
        let support = self.io.mode_support(&env);
        let mut report = capabilities::detect_report_from_env(&env, true);
        for mode in SETUP_ORDER {
            let requested = match mode {
                Mode::Raw => true,
                Mode::Alternate => self.options.alternate_screen && support.alternate,
                Mode::Cursor => self.options.hide_cursor && support.cursor,
                Mode::Mouse => self.options.mouse_capture && support.mouse,
                Mode::Focus => self.options.focus_events && support.focus,
                Mode::Paste => self.options.bracketed_paste && support.paste,
                Mode::Attributes => false,
            };
            if !requested {
                continue;
            }
            // Failed I/O may have changed the terminal. Restore attempted
            // stages, including the stage that returned the primary error.
            self.dirty[mode.index()] = true;
            if let Err(primary) = self.io.mode(mode, true) {
                let cleanup = self.restore();
                return Err(operation_error(mode.name(), primary, cleanup));
            }
        }
        let input = InputProfile::new(
            KeyEncoding::Legacy,
            self.dirty[Mode::Mouse.index()],
            self.dirty[Mode::Paste.index()],
        )
        .with_focus_events(self.dirty[Mode::Focus.index()]);
        // Geometry was validated before any mutation.
        let screen = match ScreenProfile::new(self.dirty[Mode::Alternate.index()], true)
            .with_size(geometry.0, geometry.1)
        {
            Some(screen) => screen,
            None => {
                let cleanup = self.restore();
                return Err(operation_error(
                    "enter geometry",
                    invalid("terminal geometry became invalid after preflight"),
                    cleanup,
                ));
            }
        };
        self.profile = if self.profile.snapshot_id == 0 {
            TerminalCapabilities::new(
                report.capabilities.color,
                input,
                screen,
                report.capabilities.reduced_motion,
                1,
            )
        } else {
            self.profile.refreshed(
                report.capabilities.color,
                input,
                screen,
                report.capabilities.reduced_motion,
                self.profile.snapshot_id.saturating_add(1),
            )
        };
        report.capabilities = self.profile;
        self.report = Some(report);
        self.skipped_sequences = 0;
        self.entered = true;
        Ok(())
    }

    fn leave(&mut self) -> io::Result<()> {
        let mut failures = self.restore().into_iter();
        match failures.next() {
            Some(primary) => Err(operation_error("leave", primary, failures.collect())),
            None => Ok(()),
        }
    }

    fn size(&self) -> io::Result<(u16, u16)> {
        self.require_entered("size")?;
        self.profile
            .screen
            .geometry
            .ok_or_else(|| contextual("size", invalid("entered session has no cached geometry")))
    }

    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        self.require_entered("poll")?;
        self.io
            .poll(timeout)
            .map_err(|source| contextual("poll", source))
    }

    fn read(&mut self) -> io::Result<Option<InputEvent>> {
        self.require_entered("read")?;
        for _ in 0..READ_BUDGET {
            if !self
                .io
                .poll(Duration::ZERO)
                .map_err(|source| contextual("read readiness", source))?
            {
                return Ok(None);
            }
            let raw = self
                .io
                .read()
                .map_err(|source| contextual("read", source))?;
            // Gate before decoding, so disabled paste never clones arbitrary
            // payloads. A delivered paste transfers its owned string directly.
            let enabled = match &raw {
                CrosstermEvent::Mouse(_) => self.profile.input.mouse,
                CrosstermEvent::FocusGained | CrosstermEvent::FocusLost => {
                    self.profile.input.focus_events
                }
                CrosstermEvent::Paste(_) => self.profile.input.bracketed_paste,
                _ => true,
            };
            if enabled {
                let decoded = match raw {
                    CrosstermEvent::Paste(text) => Some(InputEvent::Paste(text)),
                    other => decode_event(&other),
                };
                if let Some(decoded) = decoded {
                    if let InputEvent::Resize { cols, rows } = decoded {
                        if let Some(resized) = self.profile.resized(cols, rows) {
                            self.profile = resized;
                            if let Some(report) = &mut self.report {
                                report.capabilities = resized;
                            }
                        }
                        return Ok(InputEvent::resize(cols, rows));
                    }
                    return Ok(Some(decoded));
                }
            }
            self.skipped_sequences = self.skipped_sequences.saturating_add(1);
        }
        Ok(None)
    }

    fn submit(&mut self, commands: &[OutputCommand]) -> io::Result<()> {
        self.require_entered("submit")?;
        let (cols, rows) = self.size()?;
        for command in commands {
            validate_output(command, (cols, rows))
                .map_err(|source| contextual("submit", source))?;
        }
        self.pending.extend_from_slice(commands);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.require_entered("flush")?;
        let geometry = self.size()?;
        let commands = std::mem::take(&mut self.pending);
        if commands.is_empty() {
            return Ok(());
        }
        // Resize can arrive between submit and flush. Validate the whole
        // batch again before writing; stale batches are consumed on error.
        for command in &commands {
            validate_output(command, geometry)
                .map_err(|source| contextual("flush validation", source))?;
        }
        for command in &commands {
            match command {
                OutputCommand::SetAttributes(_) | OutputCommand::ResetAttributes => {
                    self.dirty[Mode::Attributes.index()] = true;
                }
                OutputCommand::HideCursor | OutputCommand::ShowCursor => {
                    self.dirty[Mode::Cursor.index()] = true;
                }
                _ => {}
            }
        }
        self.io
            .write_batch(&commands, geometry, self.profile.color)
            .map_err(|source| contextual("flush", source))
    }
}

impl<I: TerminalIo> Drop for Session<'_, I> {
    fn drop(&mut self) {
        // Explicit leave reports errors and retains flags for this retry.
        let _ = self.leave();
    }
}

/// Real synchronous backend owning standard handles and one terminal lease.
///
/// The backend stays on its constructing thread: crossterm requires polling
/// and reading on the same thread. A marker makes it neither Send nor Sync.
///
/// ```compile_fail
/// use jilva_terminal::CrosstermBackend;
/// let backend = CrosstermBackend::new();
/// std::thread::spawn(move || drop(backend));
/// ```
#[derive(Debug)]
pub struct CrosstermBackend {
    session: Session<'static, CrosstermIo>,
    thread_bound: PhantomData<Rc<()>>,
}

impl Default for CrosstermBackend {
    fn default() -> Self {
        Self::with_options(BackendOptions::default())
    }
}

impl CrosstermBackend {
    /// Creates an un-entered backend with default session options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an un-entered backend with explicit optional-mode requests.
    #[must_use]
    pub fn with_options(options: BackendOptions) -> Self {
        Self {
            session: Session::new(CrosstermIo::default(), options, &TERMINAL_LEASE),
            thread_bound: PhantomData,
        }
    }

    /// Returns immutable options requested for this backend instance.
    #[must_use]
    pub const fn options(&self) -> BackendOptions {
        self.session.options
    }

    /// Returns whether input/output operations have a usable live session.
    #[must_use]
    pub const fn is_entered(&self) -> bool {
        self.session.entered
    }

    /// Returns whether failed restoration left modes needing `leave` or Drop.
    /// New session entry is rejected until those modes are restored.
    #[must_use]
    pub fn has_pending_cleanup(&self) -> bool {
        !self.session.entered && self.session.dirty.iter().any(|&dirty| dirty)
    }

    /// Returns counted unsupported, invalid, or capability-gated input units.
    /// I/O failures are propagated and never counted as skipped input.
    #[must_use]
    pub const fn skipped_count(&self) -> u64 {
        self.session.skipped_sequences
    }

    /// Returns commands buffered until the next flush attempt.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.session.pending.len()
    }

    /// Returns this session's cached detector sources and profile, or `None`
    /// before successful entry. This performs no new observation or I/O.
    #[must_use]
    pub const fn detection_report(&self) -> Option<DetectionReport> {
        self.session.report
    }

    /// Delegates live, non-mutating detection to the single shared detector.
    #[must_use]
    pub fn detect() -> TerminalCapabilities {
        capabilities::detect()
    }

    /// Delegates injected detection to the shared detector under an explicit
    /// terminal-output assumption. It enables no modes and emits no bytes.
    #[must_use]
    pub fn detect_with_env(get: impl Fn(&str) -> Option<String>) -> TerminalCapabilities {
        let env = EnvSnapshot::new(
            get("COLORTERM"),
            get("TERM"),
            get("TERM_PROGRAM"),
            get("COLORFGBG"),
            get("NO_COLOR").is_some(),
        );
        capabilities::detect_from_env(&env, true)
    }
}

impl Backend for CrosstermBackend {
    fn name(&self) -> &str {
        "crossterm"
    }
    fn enter(&mut self) -> io::Result<()> {
        self.session.enter()
    }
    fn leave(&mut self) -> io::Result<()> {
        self.session.leave()
    }
    fn size(&self) -> io::Result<(u16, u16)> {
        self.session.size()
    }
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        self.session.poll(timeout)
    }
    fn read(&mut self) -> io::Result<Option<InputEvent>> {
        self.session.read()
    }
    fn submit(&mut self, commands: &[OutputCommand]) -> io::Result<()> {
        self.session.submit(commands)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.session.flush()
    }
    fn capabilities(&self) -> TerminalCapabilities {
        self.session.profile
    }
}

/// Converts one crossterm event to core-owned input without terminal I/O.
///
/// Preserves reported kinds/modifiers but always reports legacy encoding:
/// no enhancement protocol was negotiated. Unsupported keys and zero-size
/// resize reports return `None`. Session read gates mouse, focus, and paste.
#[must_use]
pub fn decode_event(event: &CrosstermEvent) -> Option<InputEvent> {
    match event {
        CrosstermEvent::Key(key) => {
            let modifiers = decode_modifiers(key.modifiers);
            let decoded = match &key.code {
                CrosstermKeyCode::Char(ch) => {
                    KeyEvent::text(ch.to_string(), modifiers, KeyEncoding::Legacy)?
                }
                CrosstermKeyCode::F(number) => {
                    KeyEvent::function(*number, modifiers, KeyEncoding::Legacy)?
                }
                code => KeyEvent::special(decode_special(code)?, modifiers, KeyEncoding::Legacy),
            };
            let kind = match key.kind {
                CrosstermKeyKind::Press => KeyEventKind::Press,
                CrosstermKeyKind::Repeat => KeyEventKind::Repeat,
                CrosstermKeyKind::Release => KeyEventKind::Release,
            };
            Some(InputEvent::Key(decoded.with_kind(kind)))
        }
        CrosstermEvent::Mouse(mouse) => {
            let (kind, button) = match mouse.kind {
                MouseEventKind::Down(button) => (MouseKind::Down, decode_button(button)),
                MouseEventKind::Up(button) => (MouseKind::Up, decode_button(button)),
                MouseEventKind::Drag(button) => (MouseKind::Drag, decode_button(button)),
                MouseEventKind::Moved => (MouseKind::Move, MouseButton::None),
                MouseEventKind::ScrollDown => (MouseKind::ScrollDown, MouseButton::None),
                MouseEventKind::ScrollUp => (MouseKind::ScrollUp, MouseButton::None),
                MouseEventKind::ScrollLeft => (MouseKind::ScrollLeft, MouseButton::None),
                MouseEventKind::ScrollRight => (MouseKind::ScrollRight, MouseButton::None),
            };
            Some(InputEvent::Mouse(
                MouseEvent::new(kind, button, mouse.column, mouse.row)
                    .with_modifiers(decode_modifiers(mouse.modifiers)),
            ))
        }
        CrosstermEvent::Resize(cols, rows) => InputEvent::resize(*cols, *rows),
        CrosstermEvent::FocusGained => Some(InputEvent::Focus(true)),
        CrosstermEvent::FocusLost => Some(InputEvent::Focus(false)),
        CrosstermEvent::Paste(text) => Some(InputEvent::Paste(text.clone())),
    }
}

fn decode_modifiers(modifiers: CrosstermModifiers) -> KeyModifiers {
    KeyModifiers::new(
        modifiers.contains(CrosstermModifiers::SHIFT),
        modifiers.contains(CrosstermModifiers::CONTROL),
        modifiers.contains(CrosstermModifiers::ALT),
        modifiers.contains(CrosstermModifiers::SUPER),
    )
    .with_hyper(modifiers.contains(CrosstermModifiers::HYPER))
    .with_meta(modifiers.contains(CrosstermModifiers::META))
}

fn decode_special(code: &CrosstermKeyCode) -> Option<SpecialKey> {
    Some(match code {
        CrosstermKeyCode::Enter => SpecialKey::Enter,
        CrosstermKeyCode::Tab => SpecialKey::Tab,
        CrosstermKeyCode::BackTab => SpecialKey::BackTab,
        CrosstermKeyCode::Backspace => SpecialKey::Backspace,
        CrosstermKeyCode::Esc => SpecialKey::Escape,
        CrosstermKeyCode::Left => SpecialKey::Left,
        CrosstermKeyCode::Right => SpecialKey::Right,
        CrosstermKeyCode::Up => SpecialKey::Up,
        CrosstermKeyCode::Down => SpecialKey::Down,
        CrosstermKeyCode::Home => SpecialKey::Home,
        CrosstermKeyCode::End => SpecialKey::End,
        CrosstermKeyCode::PageUp => SpecialKey::PageUp,
        CrosstermKeyCode::PageDown => SpecialKey::PageDown,
        CrosstermKeyCode::Delete => SpecialKey::Delete,
        CrosstermKeyCode::Insert => SpecialKey::Insert,
        _ => return None,
    })
}

fn decode_button(button: CrosstermButton) -> MouseButton {
    match button {
        CrosstermButton::Left => MouseButton::Left,
        CrosstermButton::Right => MouseButton::Right,
        CrosstermButton::Middle => MouseButton::Middle,
    }
}

fn validate_output(command: &OutputCommand, geometry: (u16, u16)) -> io::Result<()> {
    match command {
        OutputCommand::MoveTo { .. }
        | OutputCommand::PrintSegment { .. }
        | OutputCommand::SetAttributes(_)
        | OutputCommand::ResetAttributes
        | OutputCommand::HideCursor
        | OutputCommand::ShowCursor
        | OutputCommand::ClearRegion { .. } => command.validate_for_size(geometry.0, geometry.1),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported output command",
        )),
    }
}

// The encoder is shared between real platform-aware queues and pure ANSI
// formatting tests. A Vec passed to queue! is not a headless sink on Windows.
trait CommandSink {
    fn command<C: Command>(&mut self, command: C) -> io::Result<()>;
}

struct QueueSink<'a, W: Write>(&'a mut W);

impl<W: Write> CommandSink for QueueSink<'_, W> {
    fn command<C: Command>(&mut self, command: C) -> io::Result<()> {
        self.0.queue(command).map(|_| ())
    }
}

// Crossterm named colors emit 256-color SGR too. Use classic ANSI16 codes
// while retaining its named-color mapping for legacy Windows native APIs.
struct ColorCommand {
    color: ResolvedColor,
    background: bool,
}

impl Command for ColorCommand {
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        let prefix = if self.background { 48 } else { 38 };
        match self.color {
            ResolvedColor::Default => {
                write!(output, "\x1b[{}m", if self.background { 49 } else { 39 })
            }
            ResolvedColor::Ansi16(entry) => {
                let index = u16::from(entry.index());
                let base = match (self.background, index >= 8) {
                    (false, false) => 30,
                    (false, true) => 90,
                    (true, false) => 40,
                    (true, true) => 100,
                };
                write!(output, "\x1b[{}m", base + index % 8)
            }
            ResolvedColor::Palette(index) => write!(output, "\x1b[{prefix};5;{index}m"),
            ResolvedColor::Rgb(r, g, b) => write!(output, "\x1b[{prefix};2;{r};{g};{b}m"),
            _ => Err(fmt::Error),
        }
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        let color = match self.color {
            ResolvedColor::Default => CrosstermColor::Reset,
            ResolvedColor::Ansi16(entry) => [
                CrosstermColor::Black,
                CrosstermColor::DarkRed,
                CrosstermColor::DarkGreen,
                CrosstermColor::DarkYellow,
                CrosstermColor::DarkBlue,
                CrosstermColor::DarkMagenta,
                CrosstermColor::DarkCyan,
                CrosstermColor::Grey,
                CrosstermColor::DarkGrey,
                CrosstermColor::Red,
                CrosstermColor::Green,
                CrosstermColor::Yellow,
                CrosstermColor::Blue,
                CrosstermColor::Magenta,
                CrosstermColor::Cyan,
                CrosstermColor::White,
            ][usize::from(entry.index())],
            ResolvedColor::Palette(index) => CrosstermColor::AnsiValue(index),
            ResolvedColor::Rgb(r, g, b) => CrosstermColor::Rgb { r, g, b },
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "unsupported resolved color",
                ));
            }
        };
        if self.background {
            SetBackgroundColor(color).execute_winapi()
        } else {
            SetForegroundColor(color).execute_winapi()
        }
    }
}

fn encode_attributes(
    sink: &mut impl CommandSink,
    attrs: &ResolvedAttributes,
    color: ColorProfile,
) -> io::Result<()> {
    sink.command(ResetColor)?;
    sink.command(SetAttribute(Attribute::Reset))?;
    // Enforce NO_COLOR at the emission boundary even for direct RGB commands.
    // These are resolved attributes; no theme/token interpretation occurs.
    if color.color != ColorTier::Mono {
        sink.command(ColorCommand {
            color: attrs.fg,
            background: false,
        })?;
        sink.command(ColorCommand {
            color: attrs.bg,
            background: true,
        })?;
    }
    for (requested, supported, attribute) in [
        (attrs.bold, color.attrs.bold, Attribute::Bold),
        (attrs.dim, color.attrs.dim, Attribute::Dim),
        (attrs.italic, color.attrs.italic, Attribute::Italic),
        (
            attrs.underline,
            color.attrs.underline,
            Attribute::Underlined,
        ),
        (attrs.reverse, color.attrs.reverse, Attribute::Reverse),
        (
            attrs.strikethrough,
            color.attrs.strikethrough,
            Attribute::CrossedOut,
        ),
    ] {
        if requested && supported {
            sink.command(SetAttribute(attribute))?;
        }
    }
    Ok(())
}

fn encode_clear(sink: &mut impl CommandSink, rect: &Rect, geometry: (u16, u16)) -> io::Result<()> {
    if rect.width == 0 || rect.height == 0 {
        return Ok(());
    }
    let right = u32::from(rect.x) + u32::from(rect.width);
    let bottom = u32::from(rect.y) + u32::from(rect.height);
    let blank = " ".repeat(usize::from(rect.width));
    for row in u32::from(rect.y)..bottom {
        sink.command(MoveTo(rect.x, row as u16))?;
        if right == u32::from(geometry.0) {
            // Erasing to EOL doesn't advance the cursor. Printing the final
            // column could wrap and scroll when touching the bottom row.
            sink.command(Clear(ClearType::UntilNewLine))?;
        } else {
            sink.command(Print(blank.as_str()))?;
        }
    }
    Ok(())
}

fn encode_batch(
    sink: &mut impl CommandSink,
    commands: &[OutputCommand],
    geometry: (u16, u16),
    color: ColorProfile,
) -> io::Result<()> {
    for command in commands {
        match command {
            OutputCommand::MoveTo { x, y } => sink.command(MoveTo(*x, *y))?,
            OutputCommand::PrintSegment { text } => sink.command(Print(text.as_str()))?,
            OutputCommand::SetAttributes(attrs) => encode_attributes(sink, attrs, color)?,
            OutputCommand::ResetAttributes => {
                sink.command(ResetColor)?;
                sink.command(SetAttribute(Attribute::Reset))?;
            }
            OutputCommand::HideCursor => sink.command(Hide)?,
            OutputCommand::ShowCursor => sink.command(Show)?,
            OutputCommand::ClearRegion { rect } => encode_clear(sink, rect, geometry)?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "unsupported output command",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Ansi16Color, AttrSupport, BackgroundHint, KeyCode};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        RawQuery,
        Size,
        Mode(Mode, bool),
        Poll(Duration),
        Read,
        Write(usize),
    }

    #[derive(Debug)]
    struct Failure {
        call: Call,
        kind: io::ErrorKind,
    }

    #[derive(Debug)]
    struct FakeIo {
        input_tty: bool,
        output_tty: bool,
        raw_active: bool,
        geometry: (u16, u16),
        env: EnvSnapshot,
        failures: VecDeque<Failure>,
        input: VecDeque<CrosstermEvent>,
        calls: Rc<RefCell<Vec<Call>>>,
        wire: String,
    }

    impl Default for FakeIo {
        fn default() -> Self {
            Self {
                input_tty: true,
                output_tty: true,
                raw_active: false,
                geometry: (80, 24),
                env: EnvSnapshot::new(
                    Some("truecolor".into()),
                    Some("xterm-256color".into()),
                    None,
                    None,
                    false,
                ),
                failures: VecDeque::new(),
                input: VecDeque::new(),
                calls: Rc::new(RefCell::new(Vec::new())),
                wire: String::new(),
            }
        }
    }

    impl FakeIo {
        fn note(&mut self, call: Call) -> io::Result<()> {
            self.calls.borrow_mut().push(call.clone());
            if self
                .failures
                .front()
                .is_some_and(|failure| failure.call == call)
            {
                let failure = self.failures.pop_front().expect("matched failure");
                Err(io::Error::new(failure.kind, format!("injected {call:?}")))
            } else {
                Ok(())
            }
        }

        fn fail(&mut self, call: Call, kind: io::ErrorKind) {
            self.failures.push_back(Failure { call, kind });
        }
    }

    struct AnsiSink<'a>(&'a mut String);

    impl CommandSink for AnsiSink<'_> {
        fn command<C: Command>(&mut self, command: C) -> io::Result<()> {
            // Direct pure formatting. queue/Display could invoke Windows
            // native APIs even with a memory writer; tests must never do so.
            command
                .write_ansi(self.0)
                .map_err(|_| io::Error::other("ANSI formatting failed"))
        }
    }

    impl TerminalIo for FakeIo {
        fn input_is_tty(&self) -> bool {
            self.input_tty
        }
        fn output_is_tty(&self) -> bool {
            self.output_tty
        }
        fn environment(&self) -> EnvSnapshot {
            self.env.clone()
        }
        fn mode_support(&self, env: &EnvSnapshot) -> ModeSupport {
            mode_support(env, false)
        }
        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            self.note(Call::RawQuery)?;
            Ok(self.raw_active)
        }
        fn size(&mut self) -> io::Result<(u16, u16)> {
            self.note(Call::Size)?;
            Ok(self.geometry)
        }
        fn mode(&mut self, mode: Mode, enabled: bool) -> io::Result<()> {
            self.note(Call::Mode(mode, enabled))?;
            if mode == Mode::Raw {
                self.raw_active = enabled;
            }
            Ok(())
        }
        fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
            self.note(Call::Poll(timeout))?;
            Ok(!self.input.is_empty())
        }
        fn read(&mut self) -> io::Result<CrosstermEvent> {
            self.note(Call::Read)?;
            self.input
                .pop_front()
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "fake input empty"))
        }
        fn write_batch(
            &mut self,
            commands: &[OutputCommand],
            geometry: (u16, u16),
            color: ColorProfile,
        ) -> io::Result<()> {
            self.note(Call::Write(commands.len()))?;
            encode_batch(&mut AnsiSink(&mut self.wire), commands, geometry, color)
        }
    }

    fn all_modes() -> BackendOptions {
        BackendOptions {
            alternate_screen: true,
            hide_cursor: true,
            mouse_capture: true,
            focus_events: true,
            bracketed_paste: true,
        }
    }

    fn key() -> CrosstermEvent {
        CrosstermEvent::Key(crossterm::event::KeyEvent::new(
            CrosstermKeyCode::Char('q'),
            CrosstermModifiers::NONE,
        ))
    }

    #[test]
    fn fresh_backend_is_observational_and_options_are_explicit() {
        let mut backend = CrosstermBackend::new();
        assert_eq!(backend.name(), "crossterm");
        assert!(!backend.is_entered());
        assert!(!backend.has_pending_cleanup());
        assert_eq!(backend.pending_len(), 0);
        assert_eq!(backend.skipped_count(), 0);
        assert!(backend.detection_report().is_none());
        assert_eq!(backend.options(), BackendOptions::default());
        assert!(!backend.options().mouse_capture);
        assert!(!backend.options().focus_events);
        assert!(!backend.options().bracketed_paste);
        assert!(backend.size().is_err());
        assert!(backend.poll(Duration::ZERO).is_err());
        assert!(backend.read().is_err());
        assert!(backend.submit(&[]).is_err());
        assert!(backend.flush().is_err());
        backend.leave().unwrap();
    }

    #[test]
    fn rejected_redirected_input_or_output_performs_no_terminal_calls() {
        for input in [true, false] {
            let lease = AtomicBool::new(false);
            let io = FakeIo {
                input_tty: input,
                output_tty: !input,
                ..FakeIo::default()
            };
            let mut session = Session::new(io, all_modes(), &lease);
            assert_eq!(
                session.enter().unwrap_err().kind(),
                io::ErrorKind::Unsupported
            );
            assert!(session.io.calls.borrow().is_empty());
            assert!(!lease.load(Ordering::Acquire));
        }
    }

    #[test]
    fn external_raw_mode_and_preflight_failures_never_mutate_terminal() {
        for failure in [None, Some(Call::RawQuery), Some(Call::Size)] {
            let lease = AtomicBool::new(false);
            let mut io = FakeIo::default();
            if let Some(call) = failure {
                io.fail(call, io::ErrorKind::PermissionDenied);
            } else {
                io.raw_active = true;
            }
            let mut session = Session::new(io, all_modes(), &lease);
            assert!(session.enter().is_err());
            assert!(
                !session
                    .io
                    .calls
                    .borrow()
                    .iter()
                    .any(|call| matches!(call, Call::Mode(..)))
            );
            assert!(!lease.load(Ordering::Acquire));
        }
        let lease = AtomicBool::new(false);
        let io = FakeIo {
            geometry: (0, 24),
            ..FakeIo::default()
        };
        let mut session = Session::new(io, all_modes(), &lease);
        assert_eq!(
            session.enter().unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(!lease.load(Ordering::Acquire));
    }

    #[test]
    fn every_setup_failure_restores_attempted_modes_in_reverse_order() {
        for (stage, failing_mode) in SETUP_ORDER.into_iter().enumerate() {
            let lease = AtomicBool::new(false);
            let mut io = FakeIo::default();
            io.fail(
                Call::Mode(failing_mode, true),
                io::ErrorKind::PermissionDenied,
            );
            let mut session = Session::new(io, all_modes(), &lease);
            let error = session.enter().unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(std::error::Error::source(&error).is_some());
            let actual: Vec<_> = session
                .io
                .calls
                .borrow()
                .iter()
                .filter_map(|call| {
                    if let Call::Mode(mode, enabled) = call {
                        Some((*mode, *enabled))
                    } else {
                        None
                    }
                })
                .collect();
            let expected: Vec<_> = SETUP_ORDER[..=stage]
                .iter()
                .map(|mode| (*mode, true))
                .chain(
                    SETUP_ORDER[..=stage]
                        .iter()
                        .rev()
                        .map(|mode| (*mode, false)),
                )
                .collect();
            assert_eq!(actual, expected, "failure at {failing_mode:?}");
            assert!(!session.entered);
            assert!(!session.dirty.iter().any(|&dirty| dirty));
            assert!(!lease.load(Ordering::Acquire));
        }
    }

    #[test]
    fn failed_rollback_preserves_primary_kind_cleanup_sources_and_lease_until_retry() {
        let lease = AtomicBool::new(false);
        let mut io = FakeIo::default();
        io.fail(
            Call::Mode(Mode::Paste, true),
            io::ErrorKind::PermissionDenied,
        );
        io.fail(Call::Mode(Mode::Paste, false), io::ErrorKind::BrokenPipe);
        io.fail(Call::Mode(Mode::Mouse, false), io::ErrorKind::TimedOut);
        let mut session = Session::new(io, all_modes(), &lease);
        let error = session.enter().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("cleanup also failed"));
        assert!(error.to_string().contains("bracketed paste"));
        assert!(error.to_string().contains("mouse capture"));
        let detail = error
            .get_ref()
            .unwrap()
            .downcast_ref::<OperationError>()
            .unwrap();
        assert_eq!(detail.primary.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(detail.cleanup.len(), 2);
        assert_eq!(detail.cleanup[0].kind(), io::ErrorKind::BrokenPipe);
        assert!(session.dirty[Mode::Paste.index()]);
        assert!(session.dirty[Mode::Mouse.index()]);
        assert!(lease.load(Ordering::Acquire));
        assert_eq!(
            session.enter().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(session.size().is_err());
        assert!(session.poll(Duration::ZERO).is_err());
        assert!(session.read().is_err());
        assert!(session.submit(&[]).is_err());
        assert!(session.flush().is_err());
        session.io.calls.borrow_mut().clear();
        session.leave().unwrap();
        assert_eq!(
            *session.io.calls.borrow(),
            vec![
                Call::Mode(Mode::Paste, false),
                Call::Mode(Mode::Mouse, false)
            ]
        );
        assert!(!lease.load(Ordering::Acquire));
        session.enter().unwrap();
        session.leave().unwrap();
    }

    #[test]
    fn leave_retries_only_failed_modes_and_attempts_all_cleanup() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), all_modes(), &lease);
        session.enter().unwrap();
        session
            .submit(&[OutputCommand::SetAttributes(ResolvedAttributes::plain())])
            .unwrap();
        session.flush().unwrap();
        session.io.calls.borrow_mut().clear();
        session.io.fail(
            Call::Mode(Mode::Attributes, false),
            io::ErrorKind::BrokenPipe,
        );
        session
            .io
            .fail(Call::Mode(Mode::Focus, false), io::ErrorKind::TimedOut);
        let error = session.leave().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        let expected: Vec<_> = RESTORE_ORDER
            .iter()
            .map(|mode| Call::Mode(*mode, false))
            .collect();
        assert_eq!(*session.io.calls.borrow(), expected);
        assert!(!session.entered);
        assert!(lease.load(Ordering::Acquire));
        session.io.calls.borrow_mut().clear();
        session.leave().unwrap();
        assert_eq!(
            *session.io.calls.borrow(),
            vec![
                Call::Mode(Mode::Attributes, false),
                Call::Mode(Mode::Focus, false)
            ]
        );
        assert!(!lease.load(Ordering::Acquire));
        session.io.calls.borrow_mut().clear();
        session.leave().unwrap();
        assert!(session.io.calls.borrow().is_empty());
    }

    #[test]
    fn lease_blocks_second_owner_until_failed_cleanup_finishes() {
        let lease = AtomicBool::new(false);
        let mut first = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        let mut second = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        first.enter().unwrap();
        assert_eq!(
            first.enter().unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            second.enter().unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(second.io.calls.borrow().is_empty());
        first
            .io
            .fail(Call::Mode(Mode::Raw, false), io::ErrorKind::BrokenPipe);
        assert!(first.leave().is_err());
        assert_eq!(
            second.enter().unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        first.leave().unwrap();
        second.enter().unwrap();
        second.leave().unwrap();
    }

    #[test]
    fn drop_retries_explicit_cleanup_failure_without_host_io() {
        let lease = AtomicBool::new(false);
        let calls;
        {
            let mut session = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
            session.enter().unwrap();
            calls = session.io.calls.clone();
            session
                .io
                .fail(Call::Mode(Mode::Cursor, false), io::ErrorKind::BrokenPipe);
            assert!(session.leave().is_err());
            assert!(lease.load(Ordering::Acquire));
        }
        assert!(!lease.load(Ordering::Acquire));
        assert_eq!(
            calls
                .borrow()
                .iter()
                .filter(|call| **call == Call::Mode(Mode::Cursor, false))
                .count(),
            2
        );
    }

    #[test]
    fn panic_unwind_runs_the_production_restoration_state_machine() {
        let lease = AtomicBool::new(false);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let saved_calls = calls.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let io = FakeIo {
                calls,
                ..FakeIo::default()
            };
            let mut session = Session::new(io, all_modes(), &lease);
            session.enter().unwrap();
            panic!("injected application panic");
        }));
        assert!(outcome.is_err());
        assert!(!lease.load(Ordering::Acquire));
        let restoration: Vec<_> = saved_calls
            .borrow()
            .iter()
            .filter_map(|call| {
                if let Call::Mode(mode, false) = call {
                    Some(*mode)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            restoration,
            SETUP_ORDER.into_iter().rev().collect::<Vec<_>>()
        );
    }

    #[test]
    fn unknown_and_dumb_hosts_only_enter_required_raw_mode() {
        for term in [None, Some("unknown-terminal"), Some("dumb")] {
            let lease = AtomicBool::new(false);
            let mut io = FakeIo::default();
            io.env.term = term.map(str::to_owned);
            let mut session = Session::new(io, all_modes(), &lease);
            session.enter().unwrap();
            let modes: Vec<_> = session
                .io
                .calls
                .borrow()
                .iter()
                .filter(|call| matches!(call, Call::Mode(_, true)))
                .cloned()
                .collect();
            assert_eq!(modes, vec![Call::Mode(Mode::Raw, true)]);
            assert!(!session.profile.screen.alternate_screen);
            assert!(!session.profile.input.mouse);
            assert!(!session.profile.input.focus_events);
            assert!(!session.profile.input.bracketed_paste);
            session.leave().unwrap();
        }
        assert!(mode_support(&EnvSnapshot::empty(), true).alternate);
        assert!(!mode_support(&EnvSnapshot::empty(), true).paste);
    }

    #[test]
    fn snapshots_are_per_instance_cached_and_advance_only_for_session_or_resize() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), all_modes(), &lease);
        session.enter().unwrap();
        let first = session.profile;
        assert_eq!(first.snapshot_id, 1);
        assert_eq!(
            (
                first.color_version,
                first.input_version,
                first.screen_version
            ),
            (1, 1, 1)
        );
        assert!(first.input.mouse && first.input.focus_events && first.input.bracketed_paste);
        assert_eq!(first.input.key_encoding, KeyEncoding::Legacy);
        let calls = session.io.calls.borrow().len();
        assert_eq!(session.size().unwrap(), (80, 24));
        assert_eq!(session.profile, first);
        assert_eq!(session.io.calls.borrow().len(), calls);
        session.io.input.push_back(CrosstermEvent::Resize(100, 30));
        assert_eq!(session.read().unwrap(), InputEvent::resize(100, 30));
        assert_eq!(session.size().unwrap(), (100, 30));
        assert_eq!(session.profile.snapshot_id, first.snapshot_id + 1);
        assert_eq!(session.profile.screen_version, first.screen_version + 1);
        assert_eq!(session.profile.input_version, first.input_version);
        assert_eq!(session.profile.color_version, first.color_version);
        assert_eq!(session.report.unwrap().capabilities, session.profile);
        session.io.input.push_back(CrosstermEvent::Resize(100, 30));
        session.read().unwrap();
        assert_eq!(session.profile.snapshot_id, 2);
        session.leave().unwrap();
        session.enter().unwrap();
        assert_eq!(session.profile.snapshot_id, 3);
        session.leave().unwrap();
        let mut independent = Session::new(FakeIo::default(), all_modes(), &lease);
        independent.enter().unwrap();
        assert_eq!(independent.profile.snapshot_id, 1);
    }

    #[test]
    fn poll_probes_readiness_and_read_is_bounded_and_counts_filtered_units() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        session.enter().unwrap();
        for _ in 0..(READ_BUDGET + 1) {
            session.io.input.push_back(CrosstermEvent::FocusGained);
        }
        session.io.input.push_back(key());
        assert!(session.poll(Duration::from_millis(20)).unwrap());
        assert_eq!(session.read().unwrap(), None);
        assert_eq!(session.skipped_sequences, READ_BUDGET as u64);
        assert!(matches!(session.read().unwrap(), Some(InputEvent::Key(_))));
        assert_eq!(session.skipped_sequences, (READ_BUDGET + 1) as u64);
        assert!(!session.poll(Duration::ZERO).unwrap());
        assert_eq!(session.read().unwrap(), None);
        assert!(
            session
                .io
                .calls
                .borrow()
                .contains(&Call::Poll(Duration::from_millis(20)))
        );
    }

    #[test]
    fn input_io_errors_keep_kind_and_source_and_are_not_garbage() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        session.enter().unwrap();
        session
            .io
            .fail(Call::Poll(Duration::ZERO), io::ErrorKind::TimedOut);
        assert_eq!(
            session.poll(Duration::ZERO).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        session
            .io
            .fail(Call::Poll(Duration::ZERO), io::ErrorKind::Interrupted);
        assert_eq!(
            session.read().unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        session.io.input.push_back(key());
        session.io.fail(Call::Read, io::ErrorKind::UnexpectedEof);
        let error = session.read().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(session.skipped_sequences, 0);
        assert!(session.entered);
        assert!(matches!(session.read().unwrap(), Some(InputEvent::Key(_))));
    }

    #[test]
    fn optional_input_is_gated_but_opt_in_events_are_delivered() {
        for enabled in [false, true] {
            let lease = AtomicBool::new(false);
            let options = if enabled {
                all_modes()
            } else {
                BackendOptions::default()
            };
            let mut session = Session::new(FakeIo::default(), options, &lease);
            session.enter().unwrap();
            session.io.input.extend([
                CrosstermEvent::Mouse(crossterm::event::MouseEvent {
                    kind: MouseEventKind::Moved,
                    column: 1,
                    row: 2,
                    modifiers: CrosstermModifiers::SHIFT,
                }),
                CrosstermEvent::FocusLost,
                CrosstermEvent::Paste("untrusted\n\x1b[31m".into()),
                CrosstermEvent::Resize(0, 24),
                key(),
            ]);
            if enabled {
                assert!(matches!(
                    session.read().unwrap(),
                    Some(InputEvent::Mouse(_))
                ));
                assert_eq!(session.read().unwrap(), Some(InputEvent::Focus(false)));
                assert_eq!(
                    session.read().unwrap(),
                    Some(InputEvent::Paste("untrusted\n\x1b[31m".into()))
                );
                assert!(matches!(session.read().unwrap(), Some(InputEvent::Key(_))));
                assert_eq!(session.skipped_sequences, 1);
            } else {
                assert!(matches!(session.read().unwrap(), Some(InputEvent::Key(_))));
                assert_eq!(session.skipped_sequences, 4);
            }
        }
    }

    #[test]
    fn full_output_validation_is_atomic_including_direct_controls_and_geometry() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        session.enter().unwrap();
        session
            .submit(&[OutputCommand::print_segment("already queued")])
            .unwrap();
        for invalid_command in [
            OutputCommand::PrintSegment {
                text: "bad\x1b[31m".into(),
            },
            OutputCommand::PrintSegment {
                text: "line\n".into(),
            },
            OutputCommand::PrintSegment {
                text: "\u{009b}31m".into(),
            },
            OutputCommand::move_to(80, 0),
            OutputCommand::move_to(0, 24),
            OutputCommand::clear_region(Rect::new(79, 0, 2, 1)),
            OutputCommand::clear_region(Rect::new(0, 23, 1, 2)),
            OutputCommand::clear_region(Rect::new(u16::MAX, 0, 2, 1)),
            OutputCommand::clear_region(Rect::new(0, u16::MAX, 1, 2)),
        ] {
            let error = session
                .submit(&[
                    OutputCommand::print_segment("valid prefix"),
                    invalid_command,
                ])
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(session.pending.len(), 1);
            assert!(session.io.wire.is_empty());
        }
        session.flush().unwrap();
        assert_eq!(session.io.wire, "already queued");
        assert_eq!(session.pending.len(), 0);
    }

    #[test]
    fn failed_flush_discards_attempted_batch_and_never_replays_it() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        session.enter().unwrap();
        session
            .submit(&[
                OutputCommand::print_segment("first"),
                OutputCommand::HideCursor,
            ])
            .unwrap();
        session.io.fail(Call::Write(2), io::ErrorKind::BrokenPipe);
        let error = session.flush().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(session.pending.len(), 0);
        session.flush().unwrap();
        assert!(session.io.wire.is_empty());
        session
            .submit(&[OutputCommand::print_segment("second")])
            .unwrap();
        session.flush().unwrap();
        assert_eq!(session.io.wire, "second");
        assert_eq!(
            session
                .io
                .calls
                .borrow()
                .iter()
                .filter(|call| **call == Call::Write(2))
                .count(),
            1
        );
    }

    #[test]
    fn resize_invalidates_pending_geometry_before_emission() {
        let lease = AtomicBool::new(false);
        let mut session = Session::new(FakeIo::default(), BackendOptions::default(), &lease);
        session.enter().unwrap();
        session
            .submit(&[
                OutputCommand::move_to(79, 23),
                OutputCommand::print_segment("x"),
            ])
            .unwrap();
        session.io.input.push_back(CrosstermEvent::Resize(40, 10));
        session.read().unwrap();
        assert_eq!(
            session.flush().unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(session.pending.len(), 0);
        assert!(session.io.wire.is_empty());
        assert!(
            !session
                .io
                .calls
                .borrow()
                .iter()
                .any(|call| matches!(call, Call::Write(_)))
        );
    }

    #[test]
    fn output_cursor_and_attributes_restore_even_without_entry_requests() {
        let lease = AtomicBool::new(false);
        let options = BackendOptions {
            alternate_screen: false,
            hide_cursor: false,
            ..BackendOptions::default()
        };
        let mut session = Session::new(FakeIo::default(), options, &lease);
        session.enter().unwrap();
        session
            .submit(&[
                OutputCommand::HideCursor,
                OutputCommand::SetAttributes(ResolvedAttributes::plain()),
            ])
            .unwrap();
        session.flush().unwrap();
        session.io.calls.borrow_mut().clear();
        session.leave().unwrap();
        assert_eq!(
            *session.io.calls.borrow(),
            vec![
                Call::Mode(Mode::Attributes, false),
                Call::Mode(Mode::Cursor, false),
                Call::Mode(Mode::Raw, false)
            ]
        );
    }

    #[derive(Debug, Default)]
    struct WriteState {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
        fail_write: bool,
        fail_flush: bool,
    }

    struct FaultWriter(Rc<RefCell<WriteState>>);

    impl Write for FaultWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let mut state = self.0.borrow_mut();
            state.writes += 1;
            if state.fail_write && state.writes > 1 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected partial write",
                ));
            }
            let count = if state.fail_write {
                2.min(bytes.len())
            } else {
                bytes.len()
            };
            state.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            let mut state = self.0.borrow_mut();
            state.flushes += 1;
            if state.fail_flush {
                Err(io::Error::new(io::ErrorKind::TimedOut, "injected flush"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn production_buffering_never_retries_partial_write_or_flush_failure_on_drop() {
        for fail_write in [true, false] {
            let state = Rc::new(RefCell::new(WriteState {
                fail_write,
                fail_flush: !fail_write,
                ..WriteState::default()
            }));
            let error = buffered_output(FaultWriter(state.clone()), |writer| {
                writer.write_all(b"abcdef")
            })
            .unwrap_err();
            let observed = state.borrow();
            if fail_write {
                assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
                assert_eq!(observed.bytes, b"ab");
                assert_eq!(observed.writes, 2, "Drop must not retry retained cdef");
                assert_eq!(observed.flushes, 0);
            } else {
                assert_eq!(error.kind(), io::ErrorKind::TimedOut);
                assert_eq!(observed.bytes, b"abcdef");
                assert_eq!(observed.writes, 1);
                assert_eq!(observed.flushes, 1);
            }
        }
    }

    #[test]
    fn production_buffering_discards_encoded_prefix_when_encoder_fails() {
        let state = Rc::new(RefCell::new(WriteState::default()));
        let error = buffered_output(FaultWriter(state.clone()), |writer| {
            writer.write_all(b"not yet committed")?;
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "injected encoder error",
            ))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(state.borrow().bytes.is_empty());
        assert_eq!(state.borrow().writes, 0);
        assert_eq!(state.borrow().flushes, 0);
    }

    #[test]
    fn ansi_golden_covers_unicode_order_color_and_every_attribute() {
        let mut attrs =
            ResolvedAttributes::new(ResolvedColor::Rgb(1, 2, 3), ResolvedColor::Palette(42));
        attrs.bold = true;
        attrs.dim = true;
        attrs.italic = true;
        attrs.underline = true;
        attrs.reverse = true;
        attrs.strikethrough = true;
        let mut wire = String::new();
        let color = ColorProfile::new(
            ColorTier::Truecolor,
            AttrSupport::all(),
            BackgroundHint::Unknown,
        );
        encode_batch(
            &mut AnsiSink(&mut wire),
            &[
                OutputCommand::move_to(1, 2),
                OutputCommand::SetAttributes(attrs),
                OutputCommand::print_segment("héllo 👩‍💻"),
                OutputCommand::ResetAttributes,
                OutputCommand::HideCursor,
                OutputCommand::ShowCursor,
            ],
            (80, 24),
            color,
        )
        .unwrap();
        assert_eq!(
            wire,
            concat!(
                "\x1b[3;2H\x1b[0m\x1b[0m\x1b[38;2;1;2;3m\x1b[48;5;42m",
                "\x1b[1m\x1b[2m\x1b[3m\x1b[4m\x1b[7m\x1b[9m",
                "héllo 👩‍💻\x1b[0m\x1b[0m\x1b[?25l\x1b[?25h",
            )
        );
    }

    #[test]
    fn ansi16_uses_classic_codes_for_all_16_foreground_and_background_entries() {
        for index in 0_u8..16 {
            let color = ResolvedColor::Ansi16(Ansi16Color::from_index(index).unwrap());
            for background in [false, true] {
                let mut wire = String::new();
                ColorCommand { color, background }
                    .write_ansi(&mut wire)
                    .unwrap();
                let base = if background {
                    if index < 8 { 40 } else { 100 }
                } else if index < 8 {
                    30
                } else {
                    90
                };
                assert_eq!(wire, format!("\x1b[{}m", base + index % 8));
                assert!(!wire.contains(";5;"));
            }
        }
    }

    #[test]
    fn mono_emission_suppresses_direct_rgb_and_decorations_but_keeps_reverse() {
        let lease = AtomicBool::new(false);
        let mut io = FakeIo::default();
        io.env.no_color_present = true;
        let mut session = Session::new(io, BackendOptions::default(), &lease);
        session.enter().unwrap();
        let mut attrs =
            ResolvedAttributes::new(ResolvedColor::Rgb(1, 2, 3), ResolvedColor::Palette(42));
        attrs.bold = true;
        attrs.dim = true;
        attrs.italic = true;
        attrs.underline = true;
        attrs.reverse = true;
        attrs.strikethrough = true;
        session
            .submit(&[
                OutputCommand::SetAttributes(attrs),
                OutputCommand::print_segment("mono"),
            ])
            .unwrap();
        session.flush().unwrap();
        assert_eq!(session.io.wire, "\x1b[0m\x1b[0m\x1b[7mmono");
    }

    #[test]
    fn clear_region_blanks_exact_bounds_without_bottom_right_print_or_scroll() {
        let mut wire = String::new();
        encode_batch(
            &mut AnsiSink(&mut wire),
            &[
                OutputCommand::clear_region(Rect::new(2, 1, 3, 2)),
                OutputCommand::clear_region(Rect::new(78, 23, 2, 1)),
                OutputCommand::clear_region(Rect::new(u16::MAX, u16::MAX, 0, 24)),
            ],
            (80, 24),
            ColorProfile::ansi16_baseline(),
        )
        .unwrap();
        assert_eq!(wire, "\x1b[2;3H   \x1b[3;3H   \x1b[24;79H\x1b[K");
    }

    #[test]
    fn key_decoder_preserves_press_repeat_release_and_all_modifiers() {
        for (raw_kind, expected) in [
            (CrosstermKeyKind::Press, KeyEventKind::Press),
            (CrosstermKeyKind::Repeat, KeyEventKind::Repeat),
            (CrosstermKeyKind::Release, KeyEventKind::Release),
        ] {
            let mut raw = crossterm::event::KeyEvent::new(
                CrosstermKeyCode::Char('é'),
                CrosstermModifiers::all(),
            );
            raw.kind = raw_kind;
            let Some(InputEvent::Key(key)) = decode_event(&CrosstermEvent::Key(raw)) else {
                panic!("key expected")
            };
            assert_eq!(key.code, KeyCode::Text("é".into()));
            assert_eq!(key.kind, expected);
            assert_eq!(key.encoding, KeyEncoding::Legacy);
            assert!(key.modifiers.shift && key.modifiers.ctrl && key.modifiers.alt);
            assert!(key.modifiers.super_key && key.modifiers.hyper && key.modifiers.meta);
        }
    }

    #[test]
    fn key_decoder_maps_supported_special_and_function_keys_and_skips_unknown() {
        for (raw, expected) in [
            (CrosstermKeyCode::Enter, SpecialKey::Enter),
            (CrosstermKeyCode::Tab, SpecialKey::Tab),
            (CrosstermKeyCode::BackTab, SpecialKey::BackTab),
            (CrosstermKeyCode::Backspace, SpecialKey::Backspace),
            (CrosstermKeyCode::Esc, SpecialKey::Escape),
            (CrosstermKeyCode::Left, SpecialKey::Left),
            (CrosstermKeyCode::Right, SpecialKey::Right),
            (CrosstermKeyCode::Up, SpecialKey::Up),
            (CrosstermKeyCode::Down, SpecialKey::Down),
            (CrosstermKeyCode::Home, SpecialKey::Home),
            (CrosstermKeyCode::End, SpecialKey::End),
            (CrosstermKeyCode::PageUp, SpecialKey::PageUp),
            (CrosstermKeyCode::PageDown, SpecialKey::PageDown),
            (CrosstermKeyCode::Delete, SpecialKey::Delete),
            (CrosstermKeyCode::Insert, SpecialKey::Insert),
        ] {
            let event = CrosstermEvent::Key(crossterm::event::KeyEvent::new(
                raw,
                CrosstermModifiers::NONE,
            ));
            let Some(InputEvent::Key(key)) = decode_event(&event) else {
                panic!("key expected")
            };
            assert_eq!(key.code, KeyCode::Special(expected));
        }
        for number in 1..=24 {
            let raw = CrosstermEvent::Key(crossterm::event::KeyEvent::new(
                CrosstermKeyCode::F(number),
                CrosstermModifiers::NONE,
            ));
            let Some(InputEvent::Key(key)) = decode_event(&raw) else {
                panic!("function expected")
            };
            assert_eq!(key.code, KeyCode::Function(number));
        }
        let raw = CrosstermEvent::Key(crossterm::event::KeyEvent::new(
            CrosstermKeyCode::Null,
            CrosstermModifiers::NONE,
        ));
        assert!(decode_event(&raw).is_none());
        assert!(decode_event(&CrosstermEvent::Resize(0, 24)).is_none());
    }

    #[test]
    fn mouse_decoder_preserves_all_kinds_buttons_position_and_modifiers() {
        for (raw_kind, expected_kind, expected_button) in [
            (
                MouseEventKind::Down(CrosstermButton::Left),
                MouseKind::Down,
                MouseButton::Left,
            ),
            (
                MouseEventKind::Up(CrosstermButton::Right),
                MouseKind::Up,
                MouseButton::Right,
            ),
            (
                MouseEventKind::Drag(CrosstermButton::Middle),
                MouseKind::Drag,
                MouseButton::Middle,
            ),
            (MouseEventKind::Moved, MouseKind::Move, MouseButton::None),
            (
                MouseEventKind::ScrollUp,
                MouseKind::ScrollUp,
                MouseButton::None,
            ),
            (
                MouseEventKind::ScrollDown,
                MouseKind::ScrollDown,
                MouseButton::None,
            ),
            (
                MouseEventKind::ScrollLeft,
                MouseKind::ScrollLeft,
                MouseButton::None,
            ),
            (
                MouseEventKind::ScrollRight,
                MouseKind::ScrollRight,
                MouseButton::None,
            ),
        ] {
            let raw = CrosstermEvent::Mouse(crossterm::event::MouseEvent {
                kind: raw_kind,
                column: 3,
                row: 4,
                modifiers: CrosstermModifiers::all(),
            });
            let Some(InputEvent::Mouse(mouse)) = decode_event(&raw) else {
                panic!("mouse expected")
            };
            assert_eq!((mouse.kind, mouse.button), (expected_kind, expected_button));
            assert_eq!((mouse.cols, mouse.rows), (3, 4));
            assert_eq!(mouse.modifiers, decode_modifiers(CrosstermModifiers::all()));
        }
        assert_eq!(
            decode_event(&CrosstermEvent::FocusGained),
            Some(InputEvent::Focus(true))
        );
        assert_eq!(
            decode_event(&CrosstermEvent::FocusLost),
            Some(InputEvent::Focus(false))
        );
        assert_eq!(
            decode_event(&CrosstermEvent::Paste("\n\x1b".into())),
            Some(InputEvent::Paste("\n\x1b".into()))
        );
    }

    #[test]
    #[ignore = "requires exclusive real terminal; opt in manually"]
    fn real_terminal_enter_leave_and_panic_unwind_smoke() {
        assert!(
            !terminal::is_raw_mode_enabled().unwrap(),
            "raw mode must start disabled"
        );
        let mut backend = CrosstermBackend::new();
        backend
            .enter()
            .expect("exclusive interactive terminal required");
        assert!(backend.is_entered());
        assert!(terminal::is_raw_mode_enabled().unwrap());
        let size = backend.size().unwrap();
        assert!(size.0 > 0 && size.1 > 0);
        backend.leave().unwrap();
        assert!(!backend.has_pending_cleanup());
        assert!(!terminal::is_raw_mode_enabled().unwrap());
        let unwound = std::panic::catch_unwind(|| {
            let mut guard = CrosstermBackend::new();
            guard.enter().unwrap();
            assert!(terminal::is_raw_mode_enabled().unwrap());
            panic!("live terminal unwind test");
        });
        assert!(unwound.is_err());
        assert!(!terminal::is_raw_mode_enabled().unwrap());
        backend.enter().expect("unwind must release the lease");
        backend.leave().unwrap();
    }
}
