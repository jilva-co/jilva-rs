//! Headless input/output behavior through the public facade.

use std::io;
use std::time::Duration;

use jilva::{
    AttrSupport, Backend, BackgroundHint, ColorProfile, ColorTier, InputEvent, InputProfile,
    KeyEncoding, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseKind,
    OutputCommand, Rect, ResolvedAttributes, ResolvedColor, TerminalCapabilities,
    TestDoubleBackend,
};

fn full_capabilities() -> TerminalCapabilities {
    let mut caps = TestDoubleBackend::full_profile();
    caps.input = InputProfile::new(KeyEncoding::Enhanced, true, true).with_focus_events(true);
    caps
}

#[test]
fn scripted_input_preserves_unicode_key_kinds_and_all_modifiers() {
    let modifiers = KeyModifiers::new(true, true, true, true)
        .with_hyper(true)
        .with_meta(true);
    let key = KeyEvent::text("e\u{301}👩‍💻", modifiers, KeyEncoding::Enhanced)
        .expect("cluster text is valid");
    let events = vec![
        InputEvent::Key(key.clone().with_kind(KeyEventKind::Repeat)),
        InputEvent::Key(key.with_kind(KeyEventKind::Release)),
        InputEvent::Mouse(
            MouseEvent::new(MouseKind::Drag, MouseButton::Right, 8, 9).with_modifiers(modifiers),
        ),
        InputEvent::Focus(false),
        InputEvent::Paste("raw\nclipboard\u{1b}[31m".to_string()),
    ];
    let mut backend = TestDoubleBackend::with_script(full_capabilities(), events.clone());
    backend.enter().expect("enter succeeds");
    let mut drained = Vec::new();
    while backend.poll(Duration::ZERO).expect("poll succeeds") {
        drained.push(
            backend
                .read()
                .expect("read succeeds")
                .expect("script has an event"),
        );
    }
    assert_eq!(drained, events);
    assert_eq!(backend.consumed_events(), events);
    assert_eq!(backend.skipped_input(), 0);
    assert_eq!(backend.pending(), 0);
}

#[test]
fn quit_detection_uses_typed_events_and_leaves_later_input_pending() {
    for quit in [TestDoubleBackend::key('q'), TestDoubleBackend::esc()] {
        let mut backend = TestDoubleBackend::with_script(
            TerminalCapabilities::conservative(0),
            vec![
                TestDoubleBackend::key('x'),
                quit.clone(),
                TestDoubleBackend::key('z'),
            ],
        );
        backend.enter().expect("enter succeeds");
        while backend.poll(Duration::ZERO).expect("poll succeeds") {
            if backend.read().expect("read succeeds") == Some(quit.clone()) {
                break;
            }
        }
        assert_eq!(
            backend.consumed_events(),
            [TestDoubleBackend::key('x'), quit]
        );
        assert_eq!(backend.pending(), 1);
    }
}

#[test]
fn source_readiness_can_filter_all_unsupported_units_without_an_error() {
    let mut backend = TestDoubleBackend::with_script(
        TerminalCapabilities::conservative(0),
        vec![
            TestDoubleBackend::mouse_click(3, 4),
            InputEvent::Focus(true),
            InputEvent::Paste("clipboard".to_string()),
            InputEvent::Resize { cols: 0, rows: 24 },
        ],
    );
    backend.push_garbage();
    backend.enter().expect("enter succeeds");
    assert!(
        backend
            .poll(Duration::ZERO)
            .expect("source units are ready")
    );
    assert_eq!(backend.read().expect("filtered source is normal"), None);
    assert_eq!(backend.skipped_input(), 5);
    assert!(backend.consumed_events().is_empty());
    assert!(!backend.poll(Duration::ZERO).expect("source is now empty"));
    backend.script(TestDoubleBackend::key('q'));
    assert!(
        backend
            .poll(Duration::ZERO)
            .expect("later valid input is ready")
    );
    assert_eq!(
        backend.read().expect("stream remains usable"),
        Some(TestDoubleBackend::key('q'))
    );
}

#[test]
fn unsupported_input_filtering_is_bounded_and_eventually_reaches_valid_input() {
    let mut backend = TestDoubleBackend::default();
    for _ in 0..130 {
        backend.push_garbage();
    }
    backend.script(TestDoubleBackend::key('q'));
    backend.enter().expect("enter succeeds");
    for skipped in [64, 128] {
        assert!(backend.poll(Duration::ZERO).expect("source remains ready"));
        assert_eq!(backend.read().expect("bounded filtering is normal"), None);
        assert_eq!(backend.skipped_input(), skipped);
    }
    assert!(
        backend
            .poll(Duration::ZERO)
            .expect("valid input is still reachable")
    );
    assert_eq!(
        backend.read().expect("remaining garbage is skipped"),
        Some(TestDoubleBackend::key('q'))
    );
    assert_eq!(backend.skipped_input(), 130);
    assert_eq!(backend.pending(), 0);
}

#[test]
fn input_added_during_a_session_is_visible_to_a_successful_poll_or_read() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    backend.script(TestDoubleBackend::key('a'));
    assert_eq!(backend.pending(), 1);
    backend.fail_read_once(io::ErrorKind::Interrupted);
    assert_eq!(
        backend.read().expect_err("read failure is injected").kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(backend.pending(), 1, "failed read consumes no staged input");
    assert!(backend.consumed_events().is_empty());
    assert_eq!(
        backend.read().expect("direct read observes new input"),
        Some(TestDoubleBackend::key('a'))
    );
    backend.script(TestDoubleBackend::key('b'));
    backend.fail_poll_once(io::ErrorKind::Interrupted);
    assert_eq!(
        backend
            .poll(Duration::ZERO)
            .expect_err("poll failure is injected")
            .kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(backend.pending(), 1, "failed poll consumes no staged input");
    assert!(
        backend
            .poll(Duration::ZERO)
            .expect("successful poll promotes")
    );
    backend.script(TestDoubleBackend::key('c'));
    assert_eq!(
        backend.read().expect("ready input precedes later arrival"),
        Some(TestDoubleBackend::key('b'))
    );
    assert_eq!(
        backend
            .read()
            .expect("later input is observable without another poll"),
        Some(TestDoubleBackend::key('c'))
    );
    assert_eq!(
        backend.consumed_events(),
        [
            TestDoubleBackend::key('a'),
            TestDoubleBackend::key('b'),
            TestDoubleBackend::key('c')
        ]
    );
}

#[test]
fn empty_queue_reports_no_input_for_zero_and_nonzero_timeouts() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    for timeout in [
        Duration::ZERO,
        Duration::from_millis(50),
        Duration::from_secs(1),
    ] {
        assert!(!backend.poll(timeout).expect("headless poll never sleeps"));
        assert_eq!(backend.read().expect("empty read is normal"), None);
    }
    assert_eq!(backend.pending(), 0);
    assert!(backend.consumed_events().is_empty());
}

#[test]
fn consumed_resize_updates_size_and_only_the_screen_capability_domain() {
    let resize = InputEvent::Resize {
        cols: 100,
        rows: 30,
    };
    let mut backend = TestDoubleBackend::with_script(
        full_capabilities(),
        vec![
            resize.clone(),
            resize.clone(),
            InputEvent::Resize { cols: 80, rows: 0 },
        ],
    );
    backend.enter().expect("enter succeeds");
    let before = backend.capabilities();
    assert_eq!(before.screen.geometry, Some((80, 24)));
    assert!(
        backend
            .poll(Duration::ZERO)
            .expect("resize source is ready")
    );
    assert_eq!(
        backend.size().expect("poll does not consume resize"),
        (80, 24)
    );
    assert_eq!(
        backend.read().expect("resize delivers"),
        Some(resize.clone())
    );
    let after = backend.capabilities();
    assert_eq!(
        backend.size().expect("size follows consumed resize"),
        (100, 30)
    );
    assert_eq!(after.screen.geometry, Some((100, 30)));
    assert_eq!(after.snapshot_id, before.snapshot_id + 1);
    assert_eq!(after.screen_version, before.screen_version + 1);
    assert_eq!(after.color_version, before.color_version);
    assert_eq!(after.input_version, before.input_version);
    assert_eq!(after.color, before.color);
    assert_eq!(after.input, before.input);
    assert_eq!(
        backend.read().expect("duplicate resize still delivers"),
        Some(resize)
    );
    assert_eq!(
        backend.capabilities(),
        after,
        "duplicate geometry has no causality change"
    );
    assert_eq!(backend.read().expect("degenerate report is skipped"), None);
    assert_eq!(backend.capabilities(), after);
    assert_eq!(backend.skipped_input(), 1);
}

#[test]
fn injected_capability_tiers_are_cached_without_environment_probes() {
    for tier in [
        ColorTier::Truecolor,
        ColorTier::Palette256,
        ColorTier::Ansi16,
        ColorTier::Mono,
    ] {
        let mut injected = TerminalCapabilities::conservative(900);
        injected.color = ColorProfile::new(tier, AttrSupport::for_tier(tier), BackgroundHint::Dark);
        injected.reduced_motion = true;
        let mut backend = TestDoubleBackend::with_size(injected, 200, 50);
        backend.enter().expect("enter succeeds");
        let first = backend.capabilities();
        assert_eq!(first.color, injected.color);
        assert_eq!(first.input, injected.input);
        assert!(first.reduced_motion);
        assert_eq!(first.screen.geometry, Some((200, 50)));
        assert_eq!(
            first.snapshot_id, 1,
            "injected identity is not ambient state"
        );
        assert_eq!(backend.capabilities(), first);
        assert_eq!(
            backend.profile(),
            injected,
            "configuration remains unchanged"
        );
    }
}

#[test]
fn submitted_commands_preserve_complete_payloads_and_fifo_across_batches() {
    let mut attributes =
        ResolvedAttributes::new(ResolvedColor::Rgb(12, 34, 56), ResolvedColor::Palette(207));
    attributes.bold = true;
    attributes.dim = true;
    attributes.italic = true;
    attributes.underline = true;
    attributes.reverse = true;
    attributes.strikethrough = true;
    let commands = vec![
        OutputCommand::MoveTo { x: 3, y: 4 },
        OutputCommand::SetAttributes(attributes),
        OutputCommand::print_segment("e\u{301}👩‍💻"),
        OutputCommand::ResetAttributes,
        OutputCommand::ClearRegion {
            rect: Rect::new(1, 2, 10, 5),
        },
        OutputCommand::HideCursor,
        OutputCommand::ShowCursor,
    ];
    let mut backend = TestDoubleBackend::new(full_capabilities());
    backend.enter().expect("enter succeeds");
    backend.submit(&commands[..2]).expect("first batch buffers");
    backend
        .submit(&commands[2..])
        .expect("second batch appends");
    assert!(backend.captured_commands().is_empty());
    assert_eq!(backend.buffered_commands(), commands);
    backend.flush().expect("flush commits");
    assert_eq!(backend.captured_commands(), commands);
    assert_eq!(backend.flushed_command_batches(), [commands]);
    assert!(backend.buffered_commands().is_empty());
}

#[test]
fn successful_flushes_record_exact_batches_including_empty_output() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    let first = vec![OutputCommand::print_segment("a")];
    let second = vec![OutputCommand::print_segment("b"), OutputCommand::ShowCursor];
    backend.flush().expect("empty flush succeeds");
    backend.submit(&first).expect("first submit succeeds");
    backend.flush().expect("first flush succeeds");
    backend.submit(&second).expect("second submit succeeds");
    backend.flush().expect("second flush succeeds");
    assert_eq!(
        backend.flushed_command_batches(),
        [Vec::new(), first.clone(), second.clone()]
    );
    assert_eq!(backend.captured_commands(), [first, second].concat());
    assert_eq!(backend.flush_count(), 3);
}

#[test]
fn invalid_print_data_rejects_the_entire_batch_without_losing_prior_output() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    let prior = vec![OutputCommand::print_segment("safe")];
    backend.submit(&prior).expect("prior batch buffers");
    for invalid in [
        "\u{1b}[2J",
        "line\nfeed",
        "\t",
        "\0",
        "\u{7f}",
        "\u{80}",
        "\u{9b}31m",
    ] {
        let batch = [
            OutputCommand::MoveTo { x: 40, y: 10 },
            OutputCommand::PrintSegment {
                text: invalid.to_string(),
            },
            OutputCommand::HideCursor,
        ];
        assert_eq!(
            backend
                .submit(&batch)
                .expect_err("control text must be rejected")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            backend.buffered_commands(),
            prior,
            "valid prefix was not appended"
        );
        assert!(backend.captured_commands().is_empty());
    }
    backend.flush().expect("prior valid batch still flushes");
    assert_eq!(backend.captured_commands(), prior);
}

#[test]
fn injected_submit_failure_keeps_prior_buffer_and_allows_a_fresh_retry() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    let first = OutputCommand::print_segment("first");
    let second = OutputCommand::print_segment("second");
    backend
        .submit(std::slice::from_ref(&first))
        .expect("first submit succeeds");
    backend.fail_submit_once(io::ErrorKind::PermissionDenied);
    assert_eq!(
        backend
            .submit(std::slice::from_ref(&second))
            .expect_err("submit fails")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(backend.buffered_commands(), std::slice::from_ref(&first));
    backend
        .submit(std::slice::from_ref(&second))
        .expect("one-shot submit failure clears");
    backend.flush().expect("only accepted commands commit");
    assert_eq!(backend.captured_commands(), [first, second]);
}

#[test]
fn failed_flush_discards_attempted_output_and_retry_cannot_replay_it() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    let committed = OutputCommand::print_segment("committed");
    let fresh = OutputCommand::print_segment("fresh frame");
    backend
        .submit(std::slice::from_ref(&committed))
        .expect("prior batch buffers");
    backend.flush().expect("prior batch commits");
    backend
        .submit(&[OutputCommand::print_segment("uncertain delivery")])
        .expect("next batch buffers");
    backend.fail_flush_at(2, io::ErrorKind::BrokenPipe);
    assert_eq!(
        backend.flush().expect_err("second flush fails").kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(backend.buffered_commands().is_empty());
    assert_eq!(backend.captured_commands(), std::slice::from_ref(&committed));
    assert_eq!(backend.flush_count(), 1);
    backend
        .flush()
        .expect("retry has no old commands to replay");
    assert_eq!(backend.captured_commands(), std::slice::from_ref(&committed));
    backend
        .submit(std::slice::from_ref(&fresh))
        .expect("fresh frame can be submitted");
    backend.flush().expect("fresh frame commits");
    assert_eq!(backend.captured_commands(), [committed, fresh]);
    assert_eq!(backend.flushed_command_batches().len(), 3);
}

#[test]
fn degenerate_sizes_fail_and_explicit_reconfiguration_recovers() {
    for (cols, rows) in [(0, 24), (80, 0), (0, 0)] {
        let mut backend =
            TestDoubleBackend::with_size(TerminalCapabilities::conservative(0), cols, rows);
        assert_eq!(
            backend
                .enter()
                .expect_err("invalid geometry blocks setup")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(!backend.is_entered());
        assert!(!backend.raw_mode_active());
        assert_eq!(backend.snapshot_id(), 0);
        backend.set_size(80, 24);
        backend.enter().expect("valid geometry allows setup");
        backend.set_size(cols, rows);
        assert_eq!(
            backend
                .size()
                .expect_err("degenerate size must fail")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        backend.set_size(120, 40);
        assert_eq!(backend.size().expect("configured size recovers"), (120, 40));
    }
    assert_eq!(TestDoubleBackend::resize_event(0, 24), None);
    assert_eq!(TestDoubleBackend::resize_event(80, 0), None);
}

#[test]
fn out_of_bounds_commands_reject_the_entire_submit_before_buffering() {
    let mut backend = TestDoubleBackend::with_size(TerminalCapabilities::conservative(0), 10, 5);
    backend.enter().expect("enter succeeds");
    let prior = vec![OutputCommand::print_segment("valid")];
    backend.submit(&prior).expect("prior batch buffers");
    for invalid in [
        OutputCommand::MoveTo { x: 10, y: 0 },
        OutputCommand::MoveTo { x: 0, y: 5 },
        OutputCommand::ClearRegion {
            rect: Rect::new(9, 4, 2, 1),
        },
        OutputCommand::ClearRegion {
            rect: Rect::new(u16::MAX, 0, 2, 1),
        },
    ] {
        let batch = [
            OutputCommand::HideCursor,
            invalid,
            OutputCommand::ShowCursor,
        ];
        assert_eq!(
            backend
                .submit(&batch)
                .expect_err("invalid geometry is rejected")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(backend.buffered_commands(), prior);
    }
    backend.flush().expect("prior valid batch survives");
    assert_eq!(backend.captured_commands(), prior);
}

#[test]
fn resize_before_flush_revalidates_and_discards_the_entire_stale_batch() {
    let mut backend = TestDoubleBackend::with_size(TerminalCapabilities::conservative(0), 80, 24);
    backend.enter().expect("enter succeeds");
    backend
        .submit(&[
            OutputCommand::HideCursor,
            OutputCommand::MoveTo { x: 79, y: 23 },
            OutputCommand::print_segment("old geometry"),
        ])
        .expect("batch fits original terminal");
    backend.script(InputEvent::Resize { cols: 40, rows: 12 });
    assert!(backend.poll(Duration::ZERO).expect("resize becomes ready"));
    backend.read().expect("resize updates geometry");
    assert_eq!(
        backend
            .flush()
            .expect_err("stale batch no longer fits")
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(backend.buffered_commands().is_empty());
    assert!(
        backend.captured_commands().is_empty(),
        "even the valid prefix was not committed"
    );
    assert!(backend.flushed_command_batches().is_empty());
    backend
        .submit(&[OutputCommand::MoveTo { x: 39, y: 11 }])
        .expect("fresh geometry is valid");
    backend.flush().expect("fresh batch commits");
    assert_eq!(
        backend.captured_commands(),
        [OutputCommand::MoveTo { x: 39, y: 11 }]
    );
}

#[test]
fn replay_records_identical_typed_streams_and_batch_boundaries() {
    let replay = || {
        let events = vec![
            TestDoubleBackend::key('x'),
            InputEvent::Resize { cols: 90, rows: 20 },
            TestDoubleBackend::mouse_click(1, 2),
        ];
        let mut backend = TestDoubleBackend::with_script(full_capabilities(), events);
        backend.enter().expect("enter succeeds");
        while backend.poll(Duration::ZERO).expect("poll succeeds") {
            backend.read().expect("read succeeds");
        }
        backend
            .submit(&[
                OutputCommand::MoveTo { x: 0, y: 0 },
                OutputCommand::print_segment("frame"),
            ])
            .expect("submit succeeds");
        backend.flush().expect("flush succeeds");
        backend.leave().expect("leave succeeds");
        (
            backend.consumed_events().to_vec(),
            backend.captured_commands().to_vec(),
            backend.flushed_command_batches().to_vec(),
            backend.call_log(),
            backend.capabilities(),
        )
    };
    let first = replay();
    let second = replay();
    assert_eq!(first, second);
    assert_eq!(
        first.0,
        [
            TestDoubleBackend::key('x'),
            InputEvent::Resize { cols: 90, rows: 20 },
            TestDoubleBackend::mouse_click(1, 2)
        ]
    );
}
