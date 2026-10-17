//! Animation without a terminal: time is injected, every frame is a pure
//! function of it, and only the cells that changed cross the backend seam.
//!
//! Run `cargo run -p jilva --no-default-features --example motion_demo`.

use std::io;
use std::time::Duration;

use jilva::{
    Align, Backdrop, Backend, BorderStyle, Constraint, Direction, Easing, Renderer,
    ResolvedAttributes, ResolvedColor, TestDoubleBackend, Transition, View,
};

fn main() -> io::Result<()> {
    let caps = TestDoubleBackend::full_profile();
    let mut backend = TestDoubleBackend::with_size(caps, 40, 7);
    backend.enter()?;
    let result = (|| {
        let (cols, rows) = backend.size()?;
        let mut renderer = Renderer::new(cols, rows);
        let ms = Duration::from_millis;

        // A card slides in with a soft overshoot while its title fades up.
        let slide = Transition::new(-18_i32, 4, ms(0), ms(240), Easing::EaseOutBack);
        let fade = Transition::new(
            ResolvedColor::Rgb(24, 24, 24),
            ResolvedColor::Rgb(255, 214, 120),
            ms(60),
            ms(180),
            Easing::EaseOut,
        );

        for step in 0..=6u64 {
            let now = ms(step * 50);
            let x = slide.value_at(now).max(0) as u16;
            let title =
                ResolvedAttributes::new(fade.value_at(now), ResolvedColor::Default).with_bold();
            let card = View::column([
                (Constraint::Length(1), View::styled_text("Jilva", title)),
                (Constraint::Fill(1), View::text("shimmer, glimmer")),
            ])
            .padding(1)
            .bordered(BorderStyle::Rounded, ResolvedAttributes::plain());
            let scene = View::overlay([
                View::gradient((10, 20, 60), (60, 20, 90), Direction::Column),
                View::row([
                    (Constraint::Length(x), View::empty()),
                    (Constraint::Length(24), card),
                    (Constraint::Fill(1), View::empty()),
                ]),
                View::tint((0, 0, 0), 40, Backdrop::dark()),
            ])
            .text_align(Align::Start);

            let frame = renderer.render(&scene, &caps);
            backend.submit(&frame)?;
            backend.flush()?;
            println!(
                "t={:>3} ms  card x={x:<2}  {:>3} command(s)",
                step * 50,
                frame.len()
            );
        }
        for line in renderer.back_buffer().text_rows() {
            println!("  |{line}|");
        }
        Ok(())
    })();
    let restoration = backend.leave();
    result.and(restoration)
}
