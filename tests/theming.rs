//! Themes through the public facade, runtime, components, and render/backend seam.
use jilva::{
    App, AttrSupport, Backend, Buffer, ColorProfile, ColorTier, Component, Constraint, Context,
    Driven, Effects, Event, InputEvent, KeyEncoding, KeyEvent, KeyModifiers, Model, Outcome,
    OutputCommand, ResolvedAttributes, SemanticToken as Token, SpecialKey, StyleState,
    TestDoubleBackend, Theme, View,
};
use std::time::Duration;

#[derive(Clone, PartialEq)]
struct Counter(&'static str);

impl Component for Counter {
    type State = i64;
    type Message = ();
    fn init(&self) -> jilva::Result<i64> {
        Ok(0)
    }
    fn update(
        &self,
        count: &mut i64,
        event: Event,
        effects: &mut Effects,
    ) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => {
                *count += 1;
                Ok(Outcome::repaint())
            }
            Some("c") => {
                effects.set_theme(Theme::light());
                Ok(Outcome::unchanged())
            }
            Some("r") => {
                effects.set_theme(Theme::dark());
                Ok(Outcome::Ignored)
            }
            _ if matches!(event, Event::Resize { .. }) => {
                effects.set_theme(Theme::light());
                Ok(Outcome::Ignored)
            }
            _ => Ok(Outcome::Ignored),
        }
    }
    fn view(&self, count: &i64, cx: &Context) -> jilva::Result<View> {
        let mark = if cx.is_focused() { ">" } else { " " };
        Ok(View::styled_text(
            &format!("{mark}{}={count}", self.0),
            cx.style_with(Token::Accent, StyleState::new().focused(cx.is_focused())),
        ))
    }
    fn focusable(&self) -> bool {
        true
    }
}

#[derive(Default)]
struct Screen;
impl Model for Screen {
    type Message = ();
    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("l") => effects.set_theme(Theme::light()),
            Some("d") => effects.set_theme(Theme::dark()),
            Some("r") => effects.set_theme(Theme::light()),
            Some("f") => {
                effects.set_theme(Theme::light());
                effects.send(());
            }
            Some("e") => {
                effects.set_theme(Theme::light());
                return Err(jilva::Error::Callback("failed update".into()));
            }
            _ if matches!(event, Event::Message(()) | Event::Resize { .. }) => {
                effects.set_theme(Theme::dark())
            }
            _ => {}
        }
        // The effect must repaint even without a model change.
        Ok(Outcome::Ignored)
    }
    fn view(&self, cx: &Context) -> jilva::Result<View> {
        Ok(View::column([
            (
                Constraint::Length(1),
                View::row([
                    (
                        Constraint::Length(5),
                        View::component(Counter("a")).key("a"),
                    ),
                    (
                        Constraint::Length(5),
                        View::component(Counter("b")).key("b"),
                    ),
                ]),
            ),
            (
                Constraint::Length(1),
                View::styled_text("body", cx.style(Token::Text)),
            ),
        ]))
    }
}

fn key(c: char) -> InputEvent {
    TestDoubleBackend::key(c)
}
fn tab() -> InputEvent {
    InputEvent::Key(KeyEvent::special(
        SpecialKey::Tab,
        KeyModifiers::NONE,
        KeyEncoding::Legacy,
    ))
}
fn apply(screen: &mut Buffer, commands: &[OutputCommand]) {
    let (mut x, mut y) = (0, 0);
    let mut attrs = ResolvedAttributes::plain();
    for command in commands {
        match command {
            OutputCommand::MoveTo { x: col, y: row } => {
                x = *col;
                y = *row;
            }
            OutputCommand::SetAttributes(value) => attrs = *value,
            OutputCommand::ResetAttributes => attrs = ResolvedAttributes::plain(),
            OutputCommand::ClearRegion { rect } => screen.fill(*rect, ResolvedAttributes::plain()),
            OutputCommand::PrintSegment { text } => {
                x += screen.set_string(x, y, text, attrs);
            }
            _ => {}
        }
    }
}

#[test]
fn theme_switch_preserves_text_geometry_focus_and_component_state() {
    let caps = TestDoubleBackend::full_profile();
    let mut app = App::new(Screen);
    let mut screen = Buffer::new(10, 2);
    apply(
        &mut screen,
        &app.start(&caps, (10, 2), Duration::ZERO).unwrap(),
    );
    apply(&mut screen, &app.handle(key('+'), Duration::ZERO).unwrap());
    apply(&mut screen, &app.handle(tab(), Duration::ZERO).unwrap());
    apply(&mut screen, &app.handle(key('+'), Duration::ZERO).unwrap());
    let before = screen.text_rows();
    let old_attrs = *screen.get(0, 1).unwrap().attrs();
    let change = app.handle(key('l'), Duration::ZERO).unwrap();
    assert!(!change.is_empty());
    assert!(
        !change
            .iter()
            .any(|c| matches!(c, OutputCommand::ClearRegion { .. }))
    );
    apply(&mut screen, &change);
    assert_eq!(screen.text_rows(), before);
    assert_ne!(screen.get(0, 1).unwrap().attrs(), &old_attrs);
    assert_eq!(screen.text_rows()[0], " a=1 >b=1 ");
    apply(&mut screen, &app.handle(key('+'), Duration::ZERO).unwrap());
    assert_eq!(screen.text_rows()[0], " a=1 >b=2 ");
    assert_eq!(app.theme().name(), "jilva-light");
    assert!(app.handle(key('l'), Duration::ZERO).unwrap().is_empty());
}

#[test]
fn component_can_request_a_theme_and_fifo_follow_ups_choose_the_final_palette() {
    let caps = TestDoubleBackend::full_profile();
    let mut app = App::new(Screen);
    app.start(&caps, (10, 2), Duration::ZERO).unwrap();
    assert!(!app.handle(key('c'), Duration::ZERO).unwrap().is_empty());
    assert_eq!(app.theme().name(), "jilva-light");
    app.handle(key('f'), Duration::ZERO).unwrap();
    assert_eq!(app.theme().name(), "jilva-dark");
    assert!(app.handle(key('f'), Duration::ZERO).unwrap().is_empty());
}

#[test]
fn failed_updates_do_not_apply_or_leak_theme_effects() {
    let caps = TestDoubleBackend::full_profile();
    let mut app = App::new(Screen);
    app.start(&caps, (10, 2), Duration::ZERO).unwrap();
    assert!(app.handle(key('e'), Duration::ZERO).is_err());
    assert_eq!(app.theme().name(), "jilva-dark");
    assert!(app.handle(key('?'), Duration::ZERO).unwrap().is_empty());
    assert_eq!(app.theme().name(), "jilva-dark");
}

#[test]
fn initial_theme_and_bubbling_broadcast_order_are_consistent() {
    let caps = TestDoubleBackend::full_profile();
    let mut app = App::new(Screen).with_theme(Theme::light());
    let mut screen = Buffer::new(10, 2);
    apply(
        &mut screen,
        &app.start(&caps, (10, 2), Duration::ZERO).unwrap(),
    );
    assert_eq!(
        screen.get(0, 1).unwrap().attrs(),
        &Theme::light().resolve(&caps.color).style(Token::Text)
    );
    // Focused child asks for dark, then model fallback asks for light.
    assert!(app.handle(key('r'), Duration::ZERO).unwrap().is_empty());
    assert_eq!(app.theme().name(), "jilva-light");
    app.handle(key('d'), Duration::ZERO).unwrap();
    // Resize reaches the model first (dark), then children (light).
    app.handle(InputEvent::Resize { cols: 20, rows: 2 }, Duration::ZERO)
        .unwrap();
    assert_eq!(app.theme().name(), "jilva-light");
}

#[test]
fn update_failure_restores_the_backend_without_submitting_another_frame() {
    let caps = TestDoubleBackend::full_profile();
    let mut backend = TestDoubleBackend::with_size(caps, 10, 2);
    backend.script(key('e'));
    backend.fail_poll_when_exhausted();
    assert!(jilva::run_with(&mut backend, App::new(Screen)).is_err());
    assert!(!backend.is_entered());
    assert_eq!(backend.flushed_command_batches().len(), 1);
}

#[test]
fn all_tiers_reach_the_backend_and_replay_identically() {
    for tier in [
        ColorTier::Truecolor,
        ColorTier::Palette256,
        ColorTier::Ansi16,
        ColorTier::Mono,
    ] {
        let run = || {
            let mut caps = TestDoubleBackend::full_profile();
            caps.color =
                ColorProfile::new(tier, AttrSupport::for_tier(tier), caps.color.background);
            let mut backend = TestDoubleBackend::with_size(caps, 10, 2);
            backend.enter().unwrap();
            let mut app = App::new(Screen);
            let mut frames = vec![app.start(&caps, (10, 2), Duration::ZERO).unwrap()];
            for (i, input) in [key('+'), tab(), key('+'), key('l'), key('d')]
                .into_iter()
                .enumerate()
            {
                frames.push(app.handle(input, Duration::from_millis(i as u64)).unwrap());
            }
            let mut screen = Buffer::new(10, 2);
            for frame in frames {
                apply(&mut screen, &frame);
                backend.submit(&frame).unwrap();
                backend.flush().unwrap();
            }
            assert_eq!(screen.text_rows()[0], " a=1 >b=1 ");
            if tier == ColorTier::Mono {
                assert!(screen.get(5, 0).unwrap().attrs().reverse);
                assert!(!screen.get(0, 0).unwrap().attrs().reverse);
            }
            backend.leave().unwrap();
            backend.flushed_command_batches().to_vec()
        };
        assert_eq!(run(), run());
    }
}
