//! Describe a screen as an owned `View`, paint it, and watch only the changes
//! cross the backend seam. No terminal needed.
//!
//! Run `cargo run -p jilva --no-default-features --example view_demo`.

use std::io;

use jilva::{
    Align, Backend, BorderStyle, Constraint, Renderer, ResolvedAttributes, ResolvedColor, Span,
    TestDoubleBackend, View,
};

fn screen(count: u32) -> View {
    let accent = ResolvedAttributes::new(ResolvedColor::Rgb(120, 200, 255), ResolvedColor::Default)
        .with_bold();
    View::column([
        (
            Constraint::Length(1),
            View::line([Span::new("Jilva", accent), Span::plain(" — jilva")]),
        ),
        (
            Constraint::Fill(1),
            View::row([
                (Constraint::Percentage(30), View::text("menu\n日本語")),
                (
                    Constraint::Fill(1),
                    View::text(&format!("count: {count}"))
                        .text_align(Align::Center)
                        .bordered(BorderStyle::Rounded, ResolvedAttributes::plain())
                        .title("counter"),
                ),
            ])
            .gap(1),
        ),
        (
            Constraint::Length(1),
            View::text("q to quit").text_align(Align::End),
        ),
    ])
}

fn main() -> io::Result<()> {
    let caps = TestDoubleBackend::full_profile();
    let mut backend = TestDoubleBackend::with_size(caps, 36, 8);
    backend.enter()?;
    let result = (|| {
        let (cols, rows) = backend.size()?;
        let mut renderer = Renderer::new(cols, rows);
        for count in [1, 2, 2] {
            let frame = renderer.render(&screen(count), &caps);
            backend.submit(&frame)?;
            backend.flush()?;
            println!("count={count}: {} command(s)", frame.len());
            for line in renderer.back_buffer().text_rows() {
                println!("  |{line}|");
            }
        }
        Ok(())
    })();
    let restoration = backend.leave();
    result.and(restoration)
}
