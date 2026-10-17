//! Minimal real terminal demonstration: q, Escape, or Ctrl+C exits.
//!
//! Run `cargo run -p jilva --example backend_demo` in an interactive terminal.
//! It uses the facade and typed commands, without widgets or a renderer.
//! Resize the terminal and type keys to see decoded events.
//! For CI or redirected output, run `headless_demo` instead.

use std::io;
use std::time::Duration;

use jilva::{
    Backend, BackendOptions, CrosstermBackend, InputEvent, KeyCode, OutputCommand, Rect, SpecialKey,
};

fn draw(backend: &mut impl Backend, status: &str) -> io::Result<()> {
    let (cols, rows) = backend.size()?;
    let capabilities = backend.capabilities();
    let lines = [
        "Jilva Phase 2 terminal engine - q / Escape / Ctrl+C exits".to_owned(),
        format!("Terminal: {cols} columns x {rows} rows"),
        format!(
            "Modes: alternate={} mouse={} focus={} paste={}",
            capabilities.screen.alternate_screen,
            capabilities.input.mouse,
            capabilities.input.focus_events,
            capabilities.input.bracketed_paste
        ),
        status.to_owned(),
    ];
    let mut commands = vec![
        OutputCommand::ResetAttributes,
        OutputCommand::clear_region(Rect::new(0, 0, cols, rows.saturating_sub(1))),
    ];
    // Leave the final column and row unused to avoid terminal autowrap/scroll.
    for (row, text) in lines
        .iter()
        .take(usize::from(rows.saturating_sub(1)))
        .enumerate()
    {
        // This diagnostic uses ASCII escapes rather than choosing Unicode
        // display widths before Phase 3 defines the grapheme/width authority.
        let ascii = text.escape_default().to_string();
        let visible = &ascii[..ascii.len().min(usize::from(cols.saturating_sub(1)))];
        commands.push(OutputCommand::move_to(0, row as u16));
        commands.push(OutputCommand::print_segment(visible));
    }
    backend.submit(&commands)?;
    backend.flush()
}

fn interact(backend: &mut impl Backend) -> io::Result<()> {
    draw(backend, "Type a key or resize the terminal.")?;
    loop {
        if !backend.poll(Duration::from_millis(100))? {
            continue;
        }
        let Some(event) = backend.read()? else {
            continue;
        };
        match &event {
            InputEvent::Key(key) => {
                // Releases are observable input, but never activate shortcuts.
                if key.kind == jilva::KeyEventKind::Release {
                    continue;
                }
                let quit = matches!(&key.code, KeyCode::Special(SpecialKey::Escape))
                    || matches!(&key.code, KeyCode::Text(text) if text == "q"
                        || (text.eq_ignore_ascii_case("c") && key.modifiers.ctrl));
                if quit {
                    return Ok(());
                }
            }
            InputEvent::Resize { .. } => {}
            _ => {}
        }
        // draw() escapes Unicode and controls before display.
        draw(backend, &format!("Last input: {event:?}"))?;
    }
}

fn main() -> io::Result<()> {
    let mut options = BackendOptions::default();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--mouse" => options.mouse_capture = true,
            "--focus" => options.focus_events = true,
            "--paste" => options.bracketed_paste = true,
            "--help" | "-h" => {
                println!("backend_demo [--mouse] [--focus] [--paste]; q / Escape / Ctrl+C exits");
                return Ok(());
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown option; use --help",
                ));
            }
        }
    }
    let mut backend = CrosstermBackend::with_options(options);
    backend.enter()?;
    let result = interact(&mut backend);
    let restoration = backend.leave();
    match (result, restoration) {
        (Err(original), Err(cleanup)) => Err(io::Error::new(
            original.kind(),
            format!("{original}; terminal restoration also failed: {cleanup}"),
        )),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}
