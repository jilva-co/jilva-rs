//! Semantic theme gallery. `t` switches theme; Tab focuses; +/- changes counters.
//! Run with `--headless` to exercise all tiers without a terminal.
use jilva::{
    Align, App, AttrSupport, BorderStyle, ColorProfile, ColorTier, Component, Constraint, Context,
    Driven, Effects, Event, Model, Outcome, SemanticToken as Token, Span, StyleState,
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
        value: &mut i64,
        event: Event,
        _effects: &mut Effects,
    ) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => *value += 1,
            Some("-") => *value -= 1,
            _ => return Ok(Outcome::Ignored),
        }
        Ok(Outcome::repaint())
    }
    fn view(&self, value: &i64, cx: &Context) -> jilva::Result<View> {
        let focused = cx.is_focused();
        let border = if focused {
            BorderStyle::Heavy
        } else {
            BorderStyle::Rounded
        };
        let mark = if focused { ">" } else { " " };
        Ok(
            View::styled_text(&format!("{mark} {value}"), cx.style(Token::Surface))
                .text_align(Align::Center)
                .bordered(
                    border,
                    cx.style_with(Token::Border, StyleState::new().focused(focused)),
                )
                .title(self.0),
        )
    }
    fn focusable(&self) -> bool {
        true
    }
}

#[derive(Default)]
struct Gallery {
    light: bool,
}
impl Model for Gallery {
    type Message = ();
    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("t") => {
                self.light = !self.light;
                effects.set_theme(if self.light {
                    Theme::light()
                } else {
                    Theme::dark()
                });
                Ok(Outcome::repaint())
            }
            Some("q") => {
                effects.quit();
                Ok(Outcome::unchanged())
            }
            _ => Ok(Outcome::Ignored),
        }
    }
    fn view(&self, cx: &Context) -> jilva::Result<View> {
        let heading = format!(
            "Jilva / {} / {:?}",
            cx.theme().name(),
            cx.capabilities().color.color
        );
        let roles = View::column(Token::ALL.map(|token| {
            let line = View::line([
                Span::new(format!(" {:12} ", token.as_str()), cx.style(Token::Muted)),
                Span::new(" Aa 0123 ", cx.style(token)),
                Span::new(
                    " selected ",
                    cx.style_with(token, StyleState::new().selected(true)),
                ),
                Span::new(
                    " focused ",
                    cx.style_with(token, StyleState::new().focused(true)),
                ),
                Span::new(
                    " disabled ",
                    cx.style_with(token, StyleState::new().disabled(true)),
                ),
            ]);
            (Constraint::Length(1), line)
        }));
        let counters = View::row([
            (
                Constraint::Fill(1),
                View::component(Counter("left")).key("left"),
            ),
            (
                Constraint::Fill(1),
                View::component(Counter("right")).key("right"),
            ),
        ])
        .gap(2);
        Ok(View::overlay([
            View::fill(cx.style(Token::Background)),
            View::column([
                (
                    Constraint::Length(1),
                    View::styled_text(&heading, cx.style(Token::Accent)),
                ),
                (Constraint::Length(14), roles),
                (Constraint::Length(3), counters),
                (
                    Constraint::Length(1),
                    View::styled_text(
                        "t theme   Tab focus   +/- count   q quit",
                        cx.style(Token::Muted),
                    ),
                ),
            ])
            .gap(1)
            .padding(1),
        ]))
    }
}

fn headless() -> jilva::Result<()> {
    for tier in [
        ColorTier::Truecolor,
        ColorTier::Palette256,
        ColorTier::Ansi16,
        ColorTier::Mono,
    ] {
        let mut caps = TestDoubleBackend::full_profile();
        caps.color = ColorProfile::new(tier, AttrSupport::for_tier(tier), caps.color.background);
        let mut app = App::new(Gallery::default());
        let first = app.start(&caps, (80, 24), Duration::ZERO)?;
        let switched = app.handle(TestDoubleBackend::key('t'), Duration::from_millis(10))?;
        println!(
            "{tier:?}: dark {} commands -> light {} commands; {}",
            first.len(),
            switched.len(),
            app.theme().name()
        );
    }
    Ok(())
}

fn main() -> jilva::Result<()> {
    if std::env::args().any(|arg| arg == "--headless") {
        return headless();
    }
    #[cfg(feature = "crossterm")]
    {
        jilva::run(App::new(Gallery::default()))?;
        Ok(())
    }
    #[cfg(not(feature = "crossterm"))]
    {
        Err(jilva::Error::Callback(
            "Use --headless or enable the crossterm feature".into(),
        ))
    }
}
