//! Rendering pipeline through the facade only: paint → present → backend.
//!
//! Golden-output tests (frame in, expected instruction stream out) run against
//! the headless test double: no TTY, no timing, fully deterministic. They
//! import `jilva` exclusively, keeping `docs/README.md#architecture` honest.

use jilva::{
    Backend, Buffer, Damage, DiffOptions, OutputCommand, Rect, Renderer, ResolvedAttributes,
    ResolvedColor, TerminalCapabilities, TestDoubleBackend, cluster_width, diff, text_width,
    truncate_to_width,
};

fn plain() -> ResolvedAttributes {
    ResolvedAttributes::plain()
}

fn accent() -> ResolvedAttributes {
    ResolvedAttributes::new(ResolvedColor::Rgb(120, 200, 255), ResolvedColor::Default)
}

fn caps() -> TerminalCapabilities {
    TestDoubleBackend::full_profile()
}

fn live_backend(cols: u16, rows: u16) -> TestDoubleBackend {
    let mut backend = TestDoubleBackend::with_size(caps(), cols, rows);
    backend.enter().expect("test double enters");
    backend
}

#[test]
fn golden_stream_for_a_styled_frame() {
    let caps = caps();
    let before = Buffer::new(12, 2);
    let mut after = before.clone();
    after.set_string(1, 0, "Jilva", accent());
    after.set_string(7, 0, "ok", plain());
    after.set_string(0, 1, "日本", plain());

    let stream = diff(
        &before,
        &after,
        &Damage::Full,
        &caps,
        &DiffOptions::default(),
    );
    assert_eq!(
        stream,
        [
            OutputCommand::move_to(1, 0),
            OutputCommand::SetAttributes(accent()),
            OutputCommand::print_segment("Jilva"),
            OutputCommand::ResetAttributes,
            // The unchanged cell between "Jilva" and "ok" is cheaper to
            // print through than to skip with a cursor move.
            OutputCommand::print_segment(" ok"),
            OutputCommand::move_to(0, 1),
            OutputCommand::print_segment("日本"),
        ]
    );
}

#[test]
fn renderer_drives_a_backend_frame_by_frame() {
    let caps = caps();
    let mut backend = live_backend(16, 3);
    let (cols, rows) = backend.size().expect("size inside a session");
    let mut renderer = Renderer::new(cols, rows);

    // Frame 1: the screen is unknown, so the engine clears and paints.
    renderer
        .back_buffer_mut()
        .set_string(0, 0, "count: 0", plain());
    let frame = renderer.present(&Damage::Full, &caps);
    backend.submit(&frame).expect("frame 1 validates");
    backend.flush().expect("frame 1 flushes");
    assert_eq!(
        frame,
        [
            OutputCommand::ResetAttributes,
            OutputCommand::clear_region(Rect::new(0, 0, 16, 3)),
            OutputCommand::move_to(0, 0),
            OutputCommand::print_segment("count: 0"),
        ]
    );

    // Frame 2: one digit changes; only that cell crosses the seam.
    renderer.back_buffer_mut().set_string(7, 0, "1", plain());
    let frame = renderer.present(&Damage::from_rects([Rect::new(7, 0, 1, 1)]), &caps);
    backend.submit(&frame).expect("frame 2 validates");
    backend.flush().expect("frame 2 flushes");
    assert_eq!(
        frame,
        [
            OutputCommand::move_to(7, 0),
            OutputCommand::print_segment("1")
        ]
    );

    // Frame 3: nothing changed, nothing is emitted.
    let frame = renderer.present(&Damage::Full, &caps);
    assert!(frame.is_empty());
    backend.submit(&frame).expect("empty batch is valid");
    backend.flush().expect("empty batch flushes");

    // Every emitted stream was accepted whole by the backend's validator.
    assert_eq!(backend.flushed_command_batches().len(), 3);
    backend.leave().expect("test double leaves");
}

#[test]
fn resize_redraws_at_the_new_size_and_the_backend_accepts_it() {
    let caps = caps();
    let mut backend = live_backend(10, 2);
    let mut renderer = Renderer::new(10, 2);
    renderer
        .back_buffer_mut()
        .set_string(0, 0, "hello", plain());
    let frame = renderer.present(&Damage::Full, &caps);
    backend.submit(&frame).expect("initial frame");
    backend.flush().expect("flush");

    backend.set_size(6, 3);
    renderer.resize(6, 3);
    assert!(renderer.is_invalidated());
    renderer.back_buffer_mut().set_string(0, 2, "tiny", plain());
    let frame = renderer.present(&Damage::none(), &caps);
    assert!(matches!(
        frame[1],
        OutputCommand::ClearRegion { rect } if rect == Rect::new(0, 0, 6, 3)
    ));
    backend
        .submit(&frame)
        .expect("resized frame fits the new size");
    backend.flush().expect("flush");
}

#[test]
fn a_frame_larger_than_the_screen_is_refused_by_the_backend_not_truncated() {
    let caps = caps();
    let mut backend = live_backend(4, 1);
    let mut renderer = Renderer::new(8, 1);
    renderer
        .back_buffer_mut()
        .set_string(0, 0, "toolong!", plain());
    let frame = renderer.present(&Damage::Full, &caps);
    assert!(backend.submit(&frame).is_err());
    // The caller's recovery path: declare the screen unknown and resync sizes.
    renderer.resize(4, 1);
    renderer.back_buffer_mut().set_string(0, 0, "ok", plain());
    let frame = renderer.present(&Damage::Full, &caps);
    backend.submit(&frame).expect("matching size is accepted");
}

#[test]
fn hostile_text_never_reaches_the_backend_as_control_bytes() {
    let caps = caps();
    let mut backend = live_backend(24, 2);
    let mut renderer = Renderer::new(24, 2);
    renderer
        .back_buffer_mut()
        .set_string(0, 0, "hi\u{1b}]52;c;Zm9v\u{7} there\n\u{9b}2J", plain());
    let frame = renderer.present(&Damage::Full, &caps);
    // The engine strips controls itself; the backend validator agrees.
    backend
        .submit(&frame)
        .expect("no control survives painting");
    for command in &frame {
        if let OutputCommand::PrintSegment { text } = command {
            assert!(!text.chars().any(char::is_control), "{text:?}");
        }
    }
}

#[test]
fn width_authorities_are_reachable_and_consistent() {
    assert_eq!(cluster_width("日"), 2);
    assert_eq!(text_width("a日b"), 4);
    assert_eq!(truncate_to_width("日本語", 5), ("日本", 4));

    let mut buffer = Buffer::new(6, 1);
    let end = buffer.set_string(0, 0, "a日本x", plain());
    // "a日本x" is 6 cells and exactly fills the row.
    assert_eq!(end, 6);
    assert_eq!(buffer.text_rows(), ["a日本x"]);
}

#[test]
fn layout_regions_drive_painting_and_damage() {
    use jilva::{Constraint, Layout};

    let caps = caps();
    let screen = Rect::new(0, 0, 30, 6);
    let rows = Layout::column()
        .constraints([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(screen);
    let (header, body, footer) = (rows[0], rows[1], rows[2]);
    let columns = Layout::row()
        .constraints([Constraint::Percentage(30), Constraint::Fill(1)])
        .gap(1)
        .split(body);

    let mut renderer = Renderer::new(screen.width, screen.height);
    let paint = |renderer: &mut Renderer, region: Rect, text: &str| {
        renderer
            .back_buffer_mut()
            .set_string_clipped(region.x, region.y, text, plain(), region);
    };
    paint(&mut renderer, header, "header");
    paint(&mut renderer, columns[0], "nav");
    paint(&mut renderer, columns[1], "content");
    paint(&mut renderer, footer, "footer");
    let first = renderer.present(&Damage::Full, &caps);
    assert!(!first.is_empty());

    // Only the content pane changes, and its layout rectangle *is* the damage.
    paint(&mut renderer, columns[1], "CONTENT");
    let frame = renderer.present(&Damage::from_rects([columns[1]]), &caps);
    assert_eq!(
        frame,
        [
            OutputCommand::move_to(columns[1].x, columns[1].y),
            OutputCommand::print_segment("CONTENT"),
        ]
    );
}

#[test]
fn a_described_view_reaches_the_backend_and_only_changes_follow() {
    use jilva::{BorderStyle, Constraint, View};

    let caps = caps();
    let mut backend = live_backend(20, 5);
    let mut renderer = Renderer::new(20, 5);
    let screen = |n: u32| {
        View::column([
            (Constraint::Length(1), View::text("status")),
            (
                Constraint::Fill(1),
                View::text(&format!("n={n}")).bordered(BorderStyle::Plain, plain()),
            ),
        ])
    };

    let first = renderer.render(&screen(1), &caps);
    backend.submit(&first).expect("first frame validates");
    backend.flush().expect("flush");
    assert_eq!(
        renderer.back_buffer().text_rows()[2],
        "│n=1               │"
    );

    let second = renderer.render(&screen(2), &caps);
    assert_eq!(
        second,
        [
            OutputCommand::move_to(3, 2),
            OutputCommand::print_segment("2")
        ]
    );
    backend.submit(&second).expect("delta validates");
    backend.flush().expect("flush");
    assert!(renderer.render(&screen(2), &caps).is_empty());
}

#[test]
fn a_fade_only_costs_output_on_frames_the_terminal_can_show() {
    use jilva::{ColorProfile, Easing, Transition, View};
    use std::time::Duration;

    let fade = Transition::new(
        ResolvedColor::Rgb(20, 20, 20),
        ResolvedColor::Rgb(30, 120, 220),
        Duration::ZERO,
        Duration::from_millis(200),
        Easing::EaseOut,
    );
    let mut mono = TerminalCapabilities::conservative(1);
    mono.color = ColorProfile::mono_no_color();
    let tiers = [
        ("truecolor", caps()),
        ("ansi16", TerminalCapabilities::conservative(1)),
        ("mono", mono),
    ];

    let mut busy_frames = Vec::new();
    for (name, caps) in tiers {
        let mut renderer = Renderer::new(10, 1);
        let mut busy = 0;
        for step in 0..=20u64 {
            let colour = fade.value_at(Duration::from_millis(step * 10));
            let view = View::fill(ResolvedAttributes::new(ResolvedColor::Default, colour));
            let frame = renderer.render(&view, &caps);
            if step > 0 && !frame.is_empty() {
                busy += 1;
            }
        }
        busy_frames.push((name, busy));
    }
    let count = |name: &str| busy_frames.iter().find(|(n, _)| *n == name).unwrap().1;
    // Truecolor shows (almost) every step; 16 colours collapse most steps into
    // the same palette entry; mono cannot show the fade at all.
    assert!(count("truecolor") >= 15, "{busy_frames:?}");
    assert!(count("ansi16") < count("truecolor"), "{busy_frames:?}");
    assert_eq!(count("mono"), 0, "{busy_frames:?}");
}
