//! Rendering engine tour without a terminal: paint, diff, and watch what
//! crosses the backend seam.
//!
//! Run `cargo run -p jilva --no-default-features --example render_demo`.

use std::io;

use jilva::{
    Backend, Damage, OutputCommand, Rect, Renderer, ResolvedAttributes, ResolvedColor,
    TestDoubleBackend,
};

fn main() -> io::Result<()> {
    let caps = TestDoubleBackend::full_profile();
    let mut backend = TestDoubleBackend::with_size(caps, 24, 3);
    backend.enter()?;
    let result = (|| {
        let (cols, rows) = backend.size()?;
        let mut renderer = Renderer::new(cols, rows);
        let title =
            ResolvedAttributes::new(ResolvedColor::Rgb(120, 200, 255), ResolvedColor::Default)
                .with_bold();

        renderer
            .back_buffer_mut()
            .set_string(0, 0, "Jilva — jilva ✨", title);
        renderer.back_buffer_mut().set_string(
            0,
            1,
            "日本語 · wide cells",
            ResolvedAttributes::plain(),
        );
        show(
            "frame 1 (screen unknown: clear + paint)",
            &mut renderer,
            &mut backend,
            &Damage::Full,
        )?;

        renderer.back_buffer_mut().set_string(15, 0, "★", title);
        let damage = Damage::from_rects([Rect::new(15, 0, 1, 1)]);
        show(
            "frame 2 (one cell changed)",
            &mut renderer,
            &mut backend,
            &damage,
        )?;

        show(
            "frame 3 (nothing changed)",
            &mut renderer,
            &mut backend,
            &Damage::Full,
        )
    })();
    let restoration = backend.leave();
    result.and(restoration)
}

fn show(
    label: &str,
    renderer: &mut Renderer,
    backend: &mut TestDoubleBackend,
    damage: &Damage,
) -> io::Result<()> {
    let caps = TestDoubleBackend::full_profile();
    let frame: Vec<OutputCommand> = renderer.present(damage, &caps);
    backend.submit(&frame)?;
    backend.flush()?;
    println!("{label}: {} command(s)", frame.len());
    for command in &frame {
        println!("  {command:?}");
    }
    Ok(())
}
