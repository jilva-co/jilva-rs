//! Jilva Studio: a working, offline framework playground.
//! 1/2/3 choose an example; Tab moves focus; +/- and Enter interact with it.
//! r resets the focused example, t switches the palette, q quits.
//! `--headless` checks all colour tiers; `--snapshots DIR` exports real frames.

#[path = "support/snapshot.rs"]
mod snapshot;

use jilva::{
    Align, App, AttrSupport, BorderStyle, ColorProfile, ColorTier, Component, Constraint as C,
    Context, Driven, Effects, Event, InputEvent, KeyEncoding, KeyEvent, KeyModifiers, Model,
    Outcome, ResolvedAttributes, SemanticToken as Token, Span, SpecialKey, StyleState,
    TestDoubleBackend, Theme, View,
};
use std::{path::Path, time::Duration};

struct Example {
    name: &'static str,
    description: &'static str,
    hint: &'static str,
}

const EXAMPLES: [Example; 3] = [
    Example {
        name: "Counter",
        description: "Local component state. One key, one update, one frame.",
        hint: "+ / - change   Enter +1   r reset",
    },
    Example {
        name: "Layout",
        description: "A live flex split. Resize the terminal; the content follows.",
        hint: "+ / - resize   Enter +10%   r reset",
    },
    Example {
        name: "Unicode",
        description: "Real grapheme clusters. Wide glyphs. Optional word wrapping.",
        hint: "Enter toggle wrap   r reset",
    },
];

#[derive(Default)]
struct Studio {
    selected: usize,
    light: bool,
}

fn text(cx: &Context, content: &str, role: Token) -> View {
    View::styled_text(content, cx.style(role))
}

fn on(cx: &Context, role: Token, surface: Token) -> ResolvedAttributes {
    let mut attrs = cx.style(role);
    attrs.bg = cx.style(surface).bg;
    attrs
}

#[derive(Clone, PartialEq)]
struct Navigation {
    selected: usize,
    wide: bool,
}

impl Component for Navigation {
    type State = ();
    type Message = ();

    fn init(&self) -> jilva::Result<()> {
        Ok(())
    }

    fn update(&self, _: &mut (), _: Event, _: &mut Effects) -> jilva::Result<Outcome> {
        Ok(Outcome::Ignored)
    }

    fn view(&self, _: &(), cx: &Context) -> jilva::Result<View> {
        let items = EXAMPLES.iter().enumerate().map(|(index, example)| {
            let selected = index == self.selected;
            let attrs = if selected {
                cx.style_with(Token::Text, StyleState::new().selected(true))
            } else {
                cx.style(Token::Muted)
            };
            let marker = if selected && cx.is_focused() {
                ">"
            } else {
                " "
            };
            let content = if self.wide {
                format!("{marker} {}  {}", index + 1, example.name)
            } else {
                format!("{marker}{} {}", index + 1, example.name)
            };
            let item = View::overlay([
                View::fill(if selected {
                    attrs
                } else {
                    cx.style(Token::Background)
                }),
                View::styled_text(&content, attrs.with_bold()).padding_xy(0, u16::from(self.wide)),
            ]);
            (if self.wide { C::Length(3) } else { C::Fill(1) }, item)
        });
        if !self.wide {
            return Ok(View::row(items).gap(1));
        }
        Ok(View::column([
            (C::Length(2), text(cx, "EXAMPLES", Token::Muted)),
            (C::Length(11), View::column(items).gap(1)),
            (C::Fill(1), View::empty()),
            (
                C::Length(2),
                text(cx, "1-3 choose\nTab to interact", Token::Muted),
            ),
        ]))
    }

    fn focusable(&self) -> bool {
        true
    }
}

#[derive(Clone, PartialEq)]
struct Playground {
    selected: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PlaygroundState {
    count: i64,
    split: u16,
    wrap: bool,
}

impl Default for PlaygroundState {
    fn default() -> Self {
        Self {
            count: 0,
            split: 50,
            wrap: true,
        }
    }
}

// The model does not own these values. The runtime retains this component's
// state slot while its configuration, layout, theme and viewport change.
impl Component for Playground {
    type State = PlaygroundState;
    type Message = ();

    fn init(&self) -> jilva::Result<PlaygroundState> {
        Ok(PlaygroundState::default())
    }

    fn update(
        &self,
        state: &mut PlaygroundState,
        event: Event,
        _: &mut Effects,
    ) -> jilva::Result<Outcome> {
        let before = *state;
        let special = |key| event.key().is_some_and(|event| event.is_special(key));
        let reset = event.key_text() == Some("r");
        let increase = matches!(event.key_text(), Some("+" | "="))
            || special(SpecialKey::Right)
            || special(SpecialKey::Enter);
        let decrease = event.key_text() == Some("-") || special(SpecialKey::Left);
        if !reset && !increase && !decrease && event.key_text() != Some(" ") {
            return Ok(Outcome::Ignored);
        }
        match self.selected {
            0 => {
                state.count = if reset {
                    0
                } else if decrease {
                    state.count.saturating_sub(1)
                } else {
                    state.count.saturating_add(1)
                };
            }
            1 => {
                state.split = if reset {
                    50
                } else if decrease {
                    state.split.saturating_sub(10).max(10)
                } else if state.split == 90 && special(SpecialKey::Enter) {
                    10
                } else {
                    (state.split + 10).min(90)
                };
            }
            _ => state.wrap = reset || !state.wrap,
        }
        Ok(if *state == before {
            Outcome::unchanged()
        } else {
            Outcome::repaint()
        })
    }

    fn view(&self, state: &PlaygroundState, cx: &Context) -> jilva::Result<View> {
        let (width, height) = cx.size();
        let status = match self.selected {
            0 => format!("Value: {}", state.count),
            1 => format!("Split: {} / {}", state.split, 100 - state.split),
            _ => format!("Wrap: {}", if state.wrap { "ON" } else { "OFF" }),
        };
        let large = height >= 24 && width >= 60;
        let scale = if height >= 34 { 2 } else { 1 };
        let content = match self.selected {
            0 => {
                let number = if large && (-999..=999).contains(&state.count) {
                    big_number(state.count, scale)
                } else {
                    state.count.to_string()
                };
                View::column([
                    (C::Fill(1), View::empty()),
                    (
                        C::Length(if large { 5 * scale } else { 1 }),
                        text(cx, &number, Token::Text).text_align(Align::Center),
                    ),
                    (C::Fill(1), View::empty()),
                ])
            }
            1 => {
                let pane = |name: &str, role| {
                    let attrs = on(cx, Token::Text, role);
                    View::overlay([
                        View::fill(attrs),
                        View::column([
                            (C::Fill(1), View::empty()),
                            (
                                C::Length(1),
                                View::styled_text(name, attrs).text_align(Align::Center),
                            ),
                            (C::Fill(1), View::empty()),
                        ]),
                    ])
                    .bordered(BorderStyle::Plain, cx.style(Token::Border))
                };
                View::row([
                    (C::Percentage(state.split), pane("A", Token::Surface)),
                    (C::Fill(1), pane("B", Token::Overlay)),
                ])
                .gap(1)
            }
            _ => {
                let sample = "日本語 / O‘zbekcha / café\ne\u{301} / 👩‍💻 / 🇺🇿\n\nText wraps at words and keeps grapheme clusters whole, even when the terminal gets narrow.";
                let sample = text(cx, sample, Token::Text);
                if state.wrap { sample.wrap() } else { sample }
            }
        };
        let focused = cx.is_focused();
        let border = if focused {
            BorderStyle::Heavy
        } else {
            BorderStyle::Plain
        };
        let hint = if width >= 42 {
            EXAMPLES[self.selected].hint
        } else {
            match self.selected {
                0 => "+/- change  Enter +1  r reset",
                1 => "+/- resize  Enter +10%  r reset",
                _ => "Enter wrap  r reset",
            }
        };
        // Keep the same component mounted even in a tiny viewport. Resizing
        // must not silently discard an application's local state.
        let body = View::column([
            (
                C::Length(1),
                View::line([
                    Span::new(if focused { "> " } else { "  " }, cx.style(Token::Text)),
                    Span::new(status, cx.style(Token::Text).with_bold()),
                ]),
            ),
            (C::Fill(1), content),
            (
                C::Length(u16::from(height >= 8)),
                text(cx, hint, Token::Muted),
            ),
        ])
        .gap(u16::from(height >= 30))
        .padding_xy(1, u16::from(height >= 30));
        Ok(body.bordered(
            border,
            cx.style(if focused { Token::Text } else { Token::Border }),
        ))
    }

    fn focusable(&self) -> bool {
        true
    }
}

fn big_number(value: i64, scale: u16) -> String {
    const DIGITS: [[&str; 5]; 11] = [
        ["███", "█ █", "█ █", "█ █", "███"],
        [" █ ", "██ ", " █ ", " █ ", "███"],
        ["███", "  █", "███", "█  ", "███"],
        ["███", "  █", "███", "  █", "███"],
        ["█ █", "█ █", "███", "  █", "  █"],
        ["███", "█  ", "███", "  █", "███"],
        ["███", "█  ", "███", "█ █", "███"],
        ["███", "  █", "  █", "  █", "  █"],
        ["███", "█ █", "███", "█ █", "███"],
        ["███", "█ █", "███", "  █", "███"],
        ["   ", "   ", "███", "   ", "   "],
    ];
    let digits: Vec<_> = value
        .to_string()
        .chars()
        .map(|c| c.to_digit(10).map_or(10, |n| n as usize))
        .collect();
    (0..5)
        .flat_map(|y| {
            let line = digits
                .iter()
                .map(|&digit| DIGITS[digit][y])
                .collect::<Vec<_>>()
                .join("  ")
                .chars()
                .flat_map(|c| std::iter::repeat_n(c, usize::from(scale)))
                .collect::<String>();
            std::iter::repeat_n(line, usize::from(scale))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Model for Studio {
    type Message = ();

    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        let before = (self.selected, self.light);
        let special = |key| event.key().is_some_and(|event| event.is_special(key));
        match event.key_text() {
            Some("1") => self.selected = 0,
            Some("2") => self.selected = 1,
            Some("3") => self.selected = 2,
            Some("j") => self.selected = (self.selected + 1) % EXAMPLES.len(),
            Some("k") => self.selected = (self.selected + 2) % EXAMPLES.len(),
            _ if special(SpecialKey::Down) || special(SpecialKey::Right) => {
                self.selected = (self.selected + 1) % EXAMPLES.len();
            }
            _ if special(SpecialKey::Up) || special(SpecialKey::Left) => {
                self.selected = (self.selected + 2) % EXAMPLES.len();
            }
            Some("t") => {
                self.light = !self.light;
                effects.set_theme(if self.light {
                    Theme::light()
                } else {
                    Theme::dark()
                });
            }
            Some("q") => effects.quit(),
            _ => return Ok(Outcome::Ignored),
        }
        Ok(if (self.selected, self.light) == before {
            Outcome::unchanged()
        } else {
            Outcome::repaint()
        })
    }

    fn view(&self, cx: &Context) -> jilva::Result<View> {
        let (width, height) = cx.size();
        let wide = width >= 100;
        let small = width < 40 || height < 18;
        let short = height < 14;
        let margin_x = if wide { 4 } else { 1 };
        let margin_y = u16::from(height >= 24);
        let example = &EXAMPLES[self.selected];
        let navigation = View::component(Navigation {
            selected: self.selected,
            wide,
        })
        .key("navigation");
        let canvas = View::column([
            (
                C::Length(1),
                text(cx, example.name, Token::Text).key("title"),
            ),
            (
                C::Length(if small { 1 } else { 2 }),
                text(
                    cx,
                    if small {
                        "Resize to 40 x 18 or larger."
                    } else {
                        example.description
                    },
                    Token::Muted,
                )
                .wrap()
                .key("description"),
            ),
            (
                C::Fill(1),
                View::component(Playground {
                    selected: self.selected,
                })
                .key("playground"),
            ),
        ])
        .gap(u16::from(height >= 24))
        .key("canvas");
        let workspace = if wide {
            View::row([(C::Length(20), navigation), (C::Fill(1), canvas)]).gap(4)
        } else {
            View::column([(C::Length(1), navigation), (C::Fill(1), canvas)]).gap(u16::from(!short))
        };
        let header = View::row([
            (
                C::Length(9),
                View::overlay([
                    View::fill(cx.style(Token::Selection)),
                    View::styled_text(" JILVA ", cx.style(Token::Selection).with_bold())
                        .text_align(Align::Center),
                ]),
            ),
            (
                C::Fill(1),
                text(
                    cx,
                    if width >= 60 {
                        "FRAMEWORK WORKBENCH"
                    } else {
                        "WORKBENCH"
                    },
                    Token::Muted,
                )
                .text_align(Align::End),
            ),
        ]);
        let hints = if width >= 72 {
            "1-3 example   Tab focus   +/- change   r reset   t theme   q quit"
        } else if width >= 40 {
            "1-3 demo  Tab focus  t theme  q quit"
        } else {
            "Tab focus   q quit"
        };
        Ok(View::overlay([
            View::fill(cx.style(Token::Background)),
            View::column([
                (C::Length(u16::from(!short)), header),
                (C::Length(u16::from(!short)), View::empty()),
                (C::Fill(1), workspace),
                (C::Length(u16::from(!short)), View::empty()),
                (C::Length(1), text(cx, hints, Token::Muted)),
            ])
            .padding_xy(margin_x, margin_y),
        ]))
    }
}

fn named_key(key: SpecialKey) -> InputEvent {
    InputEvent::Key(KeyEvent::special(
        key,
        KeyModifiers::NONE,
        KeyEncoding::Legacy,
    ))
}

fn capabilities(tier: ColorTier) -> jilva::TerminalCapabilities {
    let mut caps = TestDoubleBackend::full_profile();
    caps.color = ColorProfile::new(tier, AttrSupport::for_tier(tier), caps.color.background);
    caps
}

fn contains(frame: &snapshot::Snapshot, needle: &str) -> bool {
    frame
        .buffer
        .text_rows()
        .iter()
        .any(|row| row.contains(needle))
}

fn headless(snapshots: Option<&Path>) -> jilva::Result<()> {
    for tier in [
        ColorTier::Truecolor,
        ColorTier::Palette256,
        ColorTier::Ansi16,
        ColorTier::Mono,
    ] {
        for size in [(120, 36), (80, 24), (40, 18)] {
            for example in 0..EXAMPLES.len() {
                let mut app = App::new(Studio::default());
                let mut frame = snapshot::Snapshot::new(size);
                frame.apply(&app.start(&capabilities(tier), size, Duration::ZERO)?);
                frame.apply(&app.handle(
                    TestDoubleBackend::key(char::from(b'1' + example as u8)),
                    Duration::from_millis(1),
                )?);
                let name = format!("{tier:?}-{}x{}-{:02}", size.0, size.1, example + 1);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-dark"), false)?;
                }
                frame.apply(&app.handle(named_key(SpecialKey::Tab), Duration::from_millis(2))?);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-focus"), false)?;
                }
                frame.apply(&app.handle(TestDoubleBackend::enter_key(), Duration::from_millis(3))?);
                assert!(contains(
                    &frame,
                    ["Value: 1", "Split: 60 / 40", "Wrap: OFF"][example]
                ));
                frame.apply(&app.handle(TestDoubleBackend::key('t'), Duration::from_millis(4))?);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-active"), true)?;
                }
                frame.apply(&app.handle(TestDoubleBackend::key('r'), Duration::from_millis(5))?);
                assert!(contains(
                    &frame,
                    ["Value: 0", "Split: 50 / 50", "Wrap: ON"][example]
                ));
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-light"), true)?;
                }
            }
            println!(
                "{tier:?} {}x{}: counter, layout, Unicode, focus, reset, theme OK",
                size.0, size.1
            );
        }
    }
    Ok(())
}

fn main() -> jilva::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(index) = args.iter().position(|arg| arg == "--snapshots") {
        let path = args
            .get(index + 1)
            .ok_or_else(|| jilva::Error::Callback("--snapshots requires a directory".into()))?;
        return headless(Some(Path::new(path)));
    }
    if args.iter().any(|arg| arg == "--headless") {
        return headless(None);
    }
    #[cfg(feature = "crossterm")]
    {
        jilva::run(App::new(Studio::default()))?;
        Ok(())
    }
    #[cfg(not(feature = "crossterm"))]
    {
        Err(jilva::Error::Callback(
            "Use --headless or enable the crossterm feature".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapshot::Snapshot;

    fn start(size: (u16, u16), tier: ColorTier) -> (App<Studio>, Snapshot) {
        let mut app = App::new(Studio::default());
        let mut frame = Snapshot::new(size);
        frame.apply(
            &app.start(&capabilities(tier), size, Duration::ZERO)
                .unwrap(),
        );
        (app, frame)
    }

    fn press(app: &mut App<Studio>, frame: &mut Snapshot, event: InputEvent) {
        let commands = app.handle(event, Duration::from_millis(10)).unwrap();
        for command in &commands {
            command
                .validate_for_size(frame.buffer.width(), frame.buffer.height())
                .unwrap();
        }
        frame.apply(&commands);
    }

    #[test]
    fn each_example_is_usable_at_every_supported_size_and_colour_tier() {
        for tier in [
            ColorTier::Truecolor,
            ColorTier::Palette256,
            ColorTier::Ansi16,
            ColorTier::Mono,
        ] {
            for size in [(120, 36), (100, 18), (80, 24), (60, 18), (40, 18)] {
                let (mut app, mut frame) = start(size, tier);
                press(&mut app, &mut frame, named_key(SpecialKey::Tab));
                for (index, example) in EXAMPLES.iter().enumerate() {
                    press(
                        &mut app,
                        &mut frame,
                        TestDoubleBackend::key(char::from(b'1' + index as u8)),
                    );
                    for required in [example.name, "r reset", "q quit"] {
                        assert!(
                            contains(&frame, required),
                            "{tier:?} {size:?}: missing {required}"
                        );
                    }
                    press(&mut app, &mut frame, TestDoubleBackend::enter_key());
                    let active = ["> Value: 1", "> Split: 60 / 40", "> Wrap: OFF"][index];
                    assert!(
                        contains(&frame, active),
                        "{tier:?} {size:?}: missing {active}"
                    );
                    press(&mut app, &mut frame, TestDoubleBackend::key('r'));
                }
            }
        }
    }

    #[test]
    fn component_state_survives_example_switches_focus_theme_and_resize() {
        let (mut app, mut frame) = start((120, 36), ColorTier::Truecolor);
        assert!(contains(&frame, "> 1  Counter"));
        // Navigation owns focus: Enter must not change a counter behind it.
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        assert!(contains(&frame, "Value: 0"));
        press(&mut app, &mut frame, named_key(SpecialKey::Tab));
        press(&mut app, &mut frame, TestDoubleBackend::key('+'));
        press(&mut app, &mut frame, TestDoubleBackend::key('+'));
        press(&mut app, &mut frame, TestDoubleBackend::key('2'));
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        press(&mut app, &mut frame, TestDoubleBackend::key('3'));
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        press(&mut app, &mut frame, TestDoubleBackend::key('1'));
        assert!(contains(&frame, "> Value: 2"));
        let before = frame.buffer.clone();
        press(&mut app, &mut frame, TestDoubleBackend::key('t'));
        assert_eq!(before.text_rows(), frame.buffer.text_rows());
        assert_ne!(before, frame.buffer);
        for size in [(80, 24), (40, 18), (28, 10), (120, 36)] {
            frame.buffer.resize(size.0, size.1);
            press(
                &mut app,
                &mut frame,
                TestDoubleBackend::resize_event(size.0, size.1).unwrap(),
            );
            assert!(
                contains(&frame, "> Value: 2"),
                "lost state or focus at {size:?}"
            );
            assert!(contains(&frame, "q quit"));
        }
        press(&mut app, &mut frame, TestDoubleBackend::key('2'));
        assert!(contains(&frame, "Split: 60 / 40"));
        press(&mut app, &mut frame, TestDoubleBackend::key('3'));
        assert!(contains(&frame, "Wrap: OFF"));
    }

    #[test]
    fn focus_routes_arrow_keys_to_the_example_or_navigation() {
        let (mut app, mut frame) = start((80, 24), ColorTier::Mono);
        press(&mut app, &mut frame, named_key(SpecialKey::Left));
        assert_eq!(app.model().selected, 2);
        press(&mut app, &mut frame, named_key(SpecialKey::Right));
        assert_eq!(app.model().selected, 0);
        press(&mut app, &mut frame, named_key(SpecialKey::Tab));
        press(&mut app, &mut frame, named_key(SpecialKey::Right));
        assert_eq!(app.model().selected, 0);
        assert!(contains(&frame, "> Value: 1"));
        press(&mut app, &mut frame, named_key(SpecialKey::Left));
        assert!(contains(&frame, "> Value: 0"));
    }

    #[test]
    fn layout_has_bounded_ratios_and_reset_and_unicode_toggles_wrapping() {
        let (mut app, mut frame) = start((40, 18), ColorTier::Mono);
        press(&mut app, &mut frame, named_key(SpecialKey::Tab));
        press(&mut app, &mut frame, TestDoubleBackend::key('2'));
        for _ in 0..20 {
            press(&mut app, &mut frame, TestDoubleBackend::key('-'));
        }
        assert!(contains(&frame, "Split: 10 / 90"));
        for _ in 0..20 {
            press(&mut app, &mut frame, TestDoubleBackend::key('+'));
        }
        assert!(contains(&frame, "Split: 90 / 10"));
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        assert!(contains(&frame, "Split: 10 / 90"));
        press(&mut app, &mut frame, TestDoubleBackend::key('r'));
        assert!(contains(&frame, "Split: 50 / 50"));
        press(&mut app, &mut frame, TestDoubleBackend::key('3'));
        assert!(contains(&frame, "日本語 / O‘zbekcha / café"));
        let wrapped = frame.buffer.text_rows();
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        assert!(contains(&frame, "Wrap: OFF"));
        assert_ne!(wrapped, frame.buffer.text_rows());
    }

    #[test]
    fn exported_frame_matches_real_driver_output_and_restores_the_backend() {
        let script = [
            TestDoubleBackend::key('2'),
            named_key(SpecialKey::Tab),
            TestDoubleBackend::enter_key(),
            TestDoubleBackend::key('t'),
            TestDoubleBackend::key('q'),
        ];
        let caps = capabilities(ColorTier::Palette256);
        let mut backend = TestDoubleBackend::with_size(caps, 80, 24);
        backend.script_all(script.to_vec());
        backend.fail_poll_when_exhausted();
        jilva::run_with(&mut backend, App::new(Studio::default())).unwrap();
        let mut from_backend = Snapshot::new((80, 24));
        for batch in backend.flushed_command_batches() {
            from_backend.apply(batch);
        }
        let (mut direct, mut expected) = start((80, 24), ColorTier::Palette256);
        for event in script {
            press(&mut direct, &mut expected, event);
        }
        assert_eq!(from_backend.buffer, expected.buffer);
        assert!(contains(&from_backend, "> Split: 60 / 40"));
        assert!(!backend.is_entered());
    }

    #[test]
    fn studio_layout_matches_the_reviewed_terminal_frames() {
        let mut actual = String::new();
        for size in [(120, 36), (80, 24), (40, 18)] {
            if !actual.is_empty() {
                actual.push('\n');
            }
            let (_, frame) = start(size, ColorTier::Truecolor);
            actual.push_str(&format!("{}x{}\n", size.0, size.1));
            for row in frame.buffer.text_rows() {
                actual.push_str(row.trim_end());
                actual.push('\n');
            }
        }
        assert_eq!(
            actual,
            include_str!("../tests/snapshots/studio.txt").replace("\r\n", "\n")
        );
    }
}
