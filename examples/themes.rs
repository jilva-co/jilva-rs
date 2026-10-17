//! Jilva Studio: an offline, interactive study of the default design language.
//! 1/2/3 or arrows choose a study; Tab moves focus; Enter selects a direction;
//! t switches dark/light; q quits. No files or external services are changed.
//! `--headless` checks all colour tiers; `--snapshots DIR` exports actual frames.

#[path = "support/snapshot.rs"]
mod snapshot;

use jilva::{
    Align, App, AttrSupport, ColorProfile, ColorTier, Component, Constraint as C, Context, Driven,
    Effects, Event, InputEvent, KeyEncoding, KeyEvent, KeyModifiers, Model, Outcome,
    ResolvedAttributes, ResolvedColor, SemanticToken as Token, Span, SpecialKey, StyleState,
    TestDoubleBackend, Theme, View,
};
use std::{path::Path, time::Duration};

struct Study {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    heading: &'static str,
    note: &'static str,
    details: [(&'static str, &'static str); 3],
}

const STUDIES: [Study; 3] = [
    Study {
        name: "Rhythm",
        title: "Make room for good work.",
        description: "A little less noise. A lot more room to think.",
        heading: "Clarity before decoration.",
        note: "Every element earns its place.",
        details: [
            ("One clear focal point", "HIERARCHY"),
            ("Space that follows the content", "RHYTHM"),
            ("Details, only when you need them", "RESTRAINT"),
        ],
    },
    Study {
        name: "Colour",
        title: "Quiet surfaces. Clear intent.",
        description: "Electric accents. Inky surfaces. Colour with a purpose.",
        heading: "A palette that knows its role.",
        note: "The same composition, in light and dark.",
        details: [
            ("Text stays readable on each surface", "CONTRAST"),
            ("Accent points to the next action", "INTENT"),
            ("Status always carries a label", "MEANING"),
        ],
    },
    Study {
        name: "Focus",
        title: "Every next step feels obvious.",
        description: "A visible focus path. Familiar keys. Nothing left to guess.",
        heading: "Designed for your keyboard.",
        note: "Try Tab, arrow keys, and Enter. Then try mono.",
        details: [
            ("Selection and focus stay distinct", "STRUCTURE"),
            ("Shortcuts stay close to the action", "GUIDANCE"),
            ("State survives a change of theme", "CONTINUITY"),
        ],
    },
];

#[derive(Default)]
struct Studio {
    selected: usize,
    saved: [bool; 3],
    light: bool,
}

#[derive(Clone, Copy)]
struct Metrics {
    wide: bool,
    roomy: bool,
    margin_x: u16,
    margin_y: u16,
    width: u16,
    canvas_height: u16,
}

impl Metrics {
    fn new((width, height): (u16, u16)) -> Self {
        let wide = width >= 100;
        let margin_x = if wide {
            4
        } else if width >= 60 {
            2
        } else {
            1
        };
        let margin_y = u16::from(height >= 22);
        let workspace_height = height.saturating_sub(margin_y * 2 + 6);
        Self {
            wide,
            roomy: height >= 30,
            margin_x,
            margin_y,
            width: width.saturating_sub(margin_x * 2).min(112),
            canvas_height: workspace_height.saturating_sub(if wide { 0 } else { 2 }),
        }
    }
}

// A role's foreground on a named surface, without hard-coded RGB in app code.
fn on(cx: &Context, role: Token, surface: Token) -> ResolvedAttributes {
    let mut attrs = cx.style(role);
    attrs.bg = cx.style(surface).bg;
    attrs
}

fn text(cx: &Context, content: &str, role: Token) -> View {
    View::styled_text(content, cx.style(role))
}

fn label(cx: &Context, content: &str) -> View {
    text(cx, content, Token::Muted)
}

// The shimmer follows semantic endpoints; resolution stays in the renderer.
fn shimmer(cx: &Context, content: &str, surface: Token) -> View {
    if matches!(
        cx.capabilities().color.color,
        ColorTier::Ansi16 | ColorTier::Mono
    ) {
        return View::styled_text(content, on(cx, Token::Accent, surface).with_bold());
    }
    use jilva::Lerp as _;
    let from = cx.style(Token::Accent).fg.to_rgb((0, 0, 0));
    let to = cx.style(Token::Info).fg.to_rgb(from);
    let clusters: Vec<_> = jilva::graphemes(content).collect();
    let last = clusters.len().saturating_sub(1).max(1) as f32;
    View::line(clusters.iter().enumerate().map(|(index, cluster)| {
        let (r, g, b) = from.lerp(&to, index as f32 / last);
        let mut attrs = on(cx, Token::Accent, surface).with_bold();
        attrs.fg = ResolvedColor::Rgb(r, g, b);
        Span::new(*cluster, attrs)
    }))
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
        // Controlled by the model: bubble the original input to its owner.
        Ok(Outcome::Ignored)
    }

    fn view(&self, _: &(), cx: &Context) -> jilva::Result<View> {
        let items = STUDIES
            .iter()
            .enumerate()
            .map(|(index, study)| {
                let selected = index == self.selected;
                let focused = selected && cx.is_focused();
                let attrs = if selected {
                    cx.style_with(Token::Text, StyleState::new().selected(true))
                } else {
                    cx.style(Token::Muted)
                };
                let marker = if focused {
                    ">"
                } else if selected {
                    "·"
                } else {
                    " "
                };
                let content = format!("{marker} {:02}  {}", index + 1, study.name);
                let item = View::overlay([
                    View::fill(if selected {
                        attrs
                    } else {
                        cx.style(Token::Background)
                    }),
                    View::styled_text(&content, if selected { attrs.with_bold() } else { attrs }),
                ]);
                (if self.wide { C::Length(1) } else { C::Fill(1) }, item)
            })
            .collect::<Vec<_>>();
        if !self.wide {
            return Ok(View::row(items).gap(1));
        }
        Ok(View::column([
            (C::Length(1), label(cx, "THE DESIGN STUDIES")),
            (C::Length(1), View::empty()),
            (C::Length(5), View::column(items).gap(1)),
            (C::Length(2), View::empty()),
            (C::Length(1), label(cx, "APPEARANCE")),
            (C::Length(1), View::empty()),
            (
                C::Length(1),
                View::line([
                    Span::new("◐  ", cx.style(Token::Accent)),
                    Span::new(
                        if cx.theme().name() == "jilva-light" {
                            "Prism paper"
                        } else {
                            "Electric dusk"
                        },
                        cx.style(Token::Text),
                    ),
                ]),
            ),
            (C::Length(1), label(cx, "   t to switch")),
            (C::Fill(1), View::empty()),
            (
                C::Length(3),
                label(cx, "A local design preview.\nYours to explore.").wrap(),
            ),
        ]))
    }

    fn focusable(&self) -> bool {
        true
    }
}

#[derive(Clone, PartialEq)]
struct DirectionAction {
    saved: bool,
}

impl Component for DirectionAction {
    type State = ();
    type Message = ();

    fn init(&self) -> jilva::Result<()> {
        Ok(())
    }

    fn update(&self, _: &mut (), _: Event, _: &mut Effects) -> jilva::Result<Outcome> {
        Ok(Outcome::Ignored)
    }

    fn view(&self, _: &(), cx: &Context) -> jilva::Result<View> {
        let focused = cx.is_focused();
        let role = if self.saved {
            Token::Success
        } else {
            Token::Accent
        };
        let mut action = cx.style(Token::Selection);
        if self.saved && !cx.capabilities().color.color.is_mono() {
            action.bg = cx.style(role).fg;
        }
        let content = View::row([
            (
                C::Fill(1),
                View::line([
                    Span::new(if focused { "> " } else { "  " }, action),
                    Span::new(
                        if self.saved {
                            "Direction selected"
                        } else {
                            "Keep this direction"
                        },
                        action.with_bold(),
                    ),
                ]),
            ),
            (
                C::Length(10),
                View::styled_text(
                    if self.saved {
                        "Enter undo"
                    } else {
                        "[ Enter ] "
                    },
                    action,
                )
                .text_align(Align::End),
            ),
        ])
        .padding_xy(1, 0);
        Ok(View::overlay([
            View::fill(action),
            content.padding_xy(1, 1),
        ]))
    }

    fn focusable(&self) -> bool {
        true
    }
}

fn review(cx: &Context, study: &Study, selected: usize, height: u16, wide: bool) -> View {
    let graphic = height >= 12 && cx.size().0 >= 120;
    let ink = |role| on(cx, role, Token::Surface);
    let surface_text = |content: &str, role| View::styled_text(content, ink(role));
    let content = if height < 6 {
        surface_text(study.heading, Token::Accent).wrap()
    } else {
        let details = View::column(study.details.iter().map(|(detail, category)| {
            let line = View::line([
                Span::new("✓  ", ink(Token::Success)),
                Span::new(*detail, ink(Token::Text)),
            ])
            .wrap();
            let row = if wide && !graphic {
                View::row([
                    (C::Fill(1), line),
                    (
                        C::Length(11),
                        surface_text(category, Token::Muted).text_align(Align::End),
                    ),
                ])
            } else {
                line
            };
            (C::Length(1), row)
        }))
        .gap(u16::from(height >= 12));
        let mut rows = vec![(
            C::Length(1),
            View::row([
                (C::Fill(1), surface_text("DESIGN REVIEW", Token::Muted)),
                (
                    C::Length(7),
                    surface_text(&format!("{:02} / 03", selected + 1), Token::Accent)
                        .text_align(Align::End),
                ),
            ]),
        )];
        if height >= 12 {
            rows.extend([
                (C::Length(1), View::empty()),
                (
                    C::Length(1),
                    View::styled_text(study.heading, ink(Token::Text).with_bold()).wrap(),
                ),
                (C::Length(1), surface_text(study.note, Token::Muted).wrap()),
                (C::Length(2), View::empty()),
            ]);
        } else {
            rows.push((C::Length(u16::from(height >= 7)), View::empty()));
        }
        rows.push((C::Fill(1), details));
        View::column(rows)
    };
    let content = if graphic {
        let mark = [
            "     ▄██████▄",
            "  ▄████████▀",
            " ▄████▀   ▄▄",
            "███▀   ▄█████",
            " ▀   ▄█████▀",
            "   ▀█████▀",
            "     ▀▀",
        ];
        let illustration = View::column([
            (C::Fill(1), View::empty()),
            (
                C::Length(7),
                View::column(mark.map(|line| {
                    (
                        C::Length(1),
                        shimmer(cx, line, Token::Surface).text_align(Align::Center),
                    )
                })),
            ),
            (C::Length(1), View::empty()),
            (
                C::Length(1),
                surface_text("J I L V A", Token::Muted).text_align(Align::Center),
            ),
            (C::Fill(1), View::empty()),
        ]);
        View::row([(C::Fill(1), content), (C::Length(22), illustration)]).gap(3)
    } else {
        content
    };
    View::overlay([
        View::fill(cx.style(Token::Surface)),
        content.padding_xy(2, 1),
    ])
}

impl Model for Studio {
    type Message = ();

    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        let special = |key| event.key().is_some_and(|event| event.is_special(key));
        match event.key_text() {
            Some("1") => self.selected = 0,
            Some("2") => self.selected = 1,
            Some("3") => self.selected = 2,
            Some("j" | "l") => self.selected = (self.selected + 1) % STUDIES.len(),
            Some("k" | "h") => self.selected = (self.selected + 2) % STUDIES.len(),
            _ if special(SpecialKey::Down) || special(SpecialKey::Right) => {
                self.selected = (self.selected + 1) % STUDIES.len()
            }
            _ if special(SpecialKey::Up) || special(SpecialKey::Left) => {
                self.selected = (self.selected + 2) % STUDIES.len()
            }
            Some(" ") => self.saved[self.selected] = !self.saved[self.selected],
            _ if special(SpecialKey::Enter) => {
                self.saved[self.selected] = !self.saved[self.selected]
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
        Ok(Outcome::repaint())
    }

    fn view(&self, cx: &Context) -> jilva::Result<View> {
        let (width, height) = cx.size();
        if width < 40 || height < 18 {
            return Ok(View::overlay([
                View::fill(cx.style(Token::Background)),
                View::column([
                    (C::Length(2), text(cx, "◆ jilva / studio", Token::Accent)),
                    (
                        C::Fill(1),
                        text(
                            cx,
                            "A little more room.\nResize to 40 × 18 or larger.",
                            Token::Text,
                        )
                        .wrap(),
                    ),
                    (C::Length(1), label(cx, "q quit")),
                ])
                .padding_xy(1, 0),
            ]));
        }
        let m = Metrics::new(cx.size());
        let study = &STUDIES[self.selected];
        let header_height = if m.roomy { 6 } else { 3 };
        let note_height = u16::from(m.wide && m.roomy);
        let review_height = m
            .canvas_height
            .saturating_sub(header_height + 4 + note_height)
            .min(15);
        let heading = if m.roomy {
            View::column([
                (
                    C::Length(1),
                    label(
                        cx,
                        &format!(
                            "STUDY {:02}   /   {}",
                            self.selected + 1,
                            study.name.to_uppercase()
                        ),
                    ),
                ),
                (C::Length(1), View::empty()),
                (C::Length(1), shimmer(cx, study.title, Token::Background)),
                (C::Length(2), label(cx, study.description).wrap()),
                (C::Length(1), View::empty()),
            ])
        } else {
            View::column([
                (C::Length(1), shimmer(cx, study.title, Token::Background)),
                (C::Length(2), label(cx, study.description).wrap()),
            ])
        };
        let canvas = View::column([
            (C::Length(header_height), heading),
            (
                C::Fill(1),
                View::column([
                    (
                        C::Length(review_height),
                        review(cx, study, self.selected, review_height, m.wide),
                    ),
                    (C::Fill(1), View::empty()),
                ]),
            ),
            (C::Length(1), View::empty()),
            (
                C::Length(3),
                View::component(DirectionAction {
                    saved: self.saved[self.selected],
                }),
            ),
            (
                C::Length(note_height),
                label(cx, "Selections stay with each study while you explore."),
            ),
        ])
        .key("canvas");
        let navigation = View::component(Navigation {
            selected: self.selected,
            wide: m.wide,
        })
        .key("navigation");
        let workspace = if m.wide {
            View::row([(C::Length(24), navigation), (C::Fill(1), canvas)]).gap(4)
        } else {
            View::column([(C::Length(1), navigation), (C::Fill(1), canvas)]).gap(1)
        };
        let header = View::row([
            (
                C::Length(20),
                View::line([
                    Span::new("◆ ", cx.style(Token::Accent)),
                    Span::new("jilva", cx.style(Token::Text).with_bold()),
                    Span::new("  /  studio", cx.style(Token::Muted)),
                ]),
            ),
            (
                C::Fill(1),
                label(
                    cx,
                    if width >= 60 {
                        "jilva-rs   /   LOCAL PREVIEW"
                    } else {
                        "LOCAL"
                    },
                )
                .text_align(Align::End),
            ),
        ]);
        let footer = if width >= 60 {
            View::row([
                (
                    C::Fill(1),
                    label(
                        cx,
                        if width >= 80 {
                            "1–3 study   ↑↓ choose   Tab focus   Enter select"
                        } else {
                            "1–3 study   ↑↓ choose   Tab focus"
                        },
                    ),
                ),
                (
                    C::Length(18),
                    label(cx, "t theme   q quit").text_align(Align::End),
                ),
            ])
        } else {
            label(cx, "1–3 study  Tab focus  t theme  q quit")
        };
        let content = View::column([
            (C::Length(1), header),
            (C::Length(1), View::empty()),
            (
                C::Length(1),
                shimmer(cx, &"━".repeat(usize::from(m.width)), Token::Background),
            ),
            (C::Length(1), View::empty()),
            (C::Fill(1), workspace),
            (C::Length(1), View::empty()),
            (C::Length(1), footer),
        ]);
        Ok(View::overlay([
            View::fill(cx.style(Token::Background)),
            View::row([(C::Length(m.width), content)])
                .align(Align::Center)
                .padding_xy(m.margin_x, m.margin_y),
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

fn headless(snapshots: Option<&Path>) -> jilva::Result<()> {
    for tier in [
        ColorTier::Truecolor,
        ColorTier::Palette256,
        ColorTier::Ansi16,
        ColorTier::Mono,
    ] {
        for size in [(120, 36), (80, 24), (40, 18)] {
            for study in 0..STUDIES.len() {
                let mut app = App::new(Studio::default());
                let mut frame = snapshot::Snapshot::new(size);
                frame.apply(&app.start(&capabilities(tier), size, Duration::ZERO)?);
                let shortcut = char::from(b'1' + study as u8);
                frame.apply(
                    &app.handle(TestDoubleBackend::key(shortcut), Duration::from_millis(1))?,
                );
                let name = format!("{tier:?}-{}x{}-{:02}", size.0, size.1, study + 1);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-dark"), false)?;
                }
                frame.apply(&app.handle(named_key(SpecialKey::Tab), Duration::from_millis(2))?);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-focus"), false)?;
                }
                frame.apply(&app.handle(TestDoubleBackend::enter_key(), Duration::from_millis(3))?);
                frame.apply(&app.handle(TestDoubleBackend::key('t'), Duration::from_millis(4))?);
                assert_eq!(app.model().selected, study);
                assert!(app.model().saved[study]);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-selected"), true)?;
                }
                frame.apply(&app.handle(TestDoubleBackend::enter_key(), Duration::from_millis(5))?);
                assert!(!app.model().saved[study]);
                if let Some(dir) = snapshots {
                    frame.save(dir, &format!("{name}-light"), true)?;
                }
            }
            println!(
                "{tier:?} {}x{}: 3 studies, focus, selection, theme switch OK",
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

    fn position(frame: &Snapshot, needle: &str) -> Option<(usize, usize)> {
        frame
            .buffer
            .text_rows()
            .iter()
            .enumerate()
            .find_map(|(y, row)| {
                row.find(needle)
                    .map(|offset| (jilva::text_width(&row[..offset]), y))
            })
    }

    #[test]
    fn every_study_keeps_its_title_action_and_exit_at_each_size_and_tier() {
        for tier in [
            ColorTier::Truecolor,
            ColorTier::Palette256,
            ColorTier::Ansi16,
            ColorTier::Mono,
        ] {
            for size in [(120, 36), (100, 18), (80, 24), (60, 18), (40, 18)] {
                let (mut app, mut frame) = start(size, tier);
                for (index, study) in STUDIES.iter().enumerate() {
                    press(
                        &mut app,
                        &mut frame,
                        TestDoubleBackend::key(char::from(b'1' + index as u8)),
                    );
                    for required in [study.title, "Keep this direction", "Enter", "q quit"] {
                        assert!(
                            position(&frame, required).is_some(),
                            "{tier:?} {size:?}: missing {required:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn focus_selection_and_theme_changes_preserve_the_controlled_state_and_geometry() {
        let (mut app, mut frame) = start((80, 24), ColorTier::Truecolor);
        assert!(position(&frame, "> 01  Rhythm").is_some());
        press(&mut app, &mut frame, named_key(SpecialKey::Tab));
        assert!(position(&frame, "> Keep this direction").is_some());
        press(&mut app, &mut frame, named_key(SpecialKey::Left));
        assert_eq!(app.model().selected, 2, "navigation wraps backwards");
        press(&mut app, &mut frame, named_key(SpecialKey::Right));
        assert_eq!(app.model().selected, 0);
        press(&mut app, &mut frame, TestDoubleBackend::key('3'));
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        assert_eq!(app.model().saved, [false, false, true]);
        let before = frame.buffer.clone();
        press(&mut app, &mut frame, TestDoubleBackend::key('t'));
        assert_eq!(before.text_rows(), frame.buffer.text_rows());
        assert_ne!(before, frame.buffer, "the palette actually changes");
        assert!(position(&frame, "> Direction selected").is_some());
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        assert_eq!(app.model().saved, [false; 3], "Enter undoes the choice");
    }

    #[test]
    fn resize_keeps_focus_across_breakpoints_and_state_through_the_small_window_fallback() {
        let (mut app, mut frame) = start((120, 36), ColorTier::Mono);
        press(&mut app, &mut frame, named_key(SpecialKey::Tab));
        press(&mut app, &mut frame, TestDoubleBackend::enter_key());
        for size in [(80, 24), (40, 18), (120, 36)] {
            frame.buffer.resize(size.0, size.1);
            press(
                &mut app,
                &mut frame,
                TestDoubleBackend::resize_event(size.0, size.1).unwrap(),
            );
            assert!(
                position(&frame, "> Direction selected").is_some(),
                "{size:?}: focus must follow the control"
            );
        }
        frame.buffer.resize(28, 8);
        press(
            &mut app,
            &mut frame,
            TestDoubleBackend::resize_event(28, 8).unwrap(),
        );
        assert!(position(&frame, "Resize to 40 × 18").is_some());
        assert!(position(&frame, "q quit").is_some());
        frame.buffer.resize(80, 24);
        press(
            &mut app,
            &mut frame,
            TestDoubleBackend::resize_event(80, 24).unwrap(),
        );
        assert!(position(&frame, "Direction selected").is_some());
        assert!(app.model().saved[0]);
    }

    #[test]
    fn exported_frame_matches_the_driver_and_backend_output() {
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
        let app = jilva::run_with(&mut backend, App::new(Studio::default())).unwrap();
        assert!(app.model().saved[1]);
        let mut from_backend = Snapshot::new((80, 24));
        for batch in backend.flushed_command_batches() {
            from_backend.apply(batch);
        }
        let (mut direct, mut expected) = start((80, 24), ColorTier::Palette256);
        for event in script {
            press(&mut direct, &mut expected, event);
        }
        assert_eq!(from_backend.buffer, expected.buffer);
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
