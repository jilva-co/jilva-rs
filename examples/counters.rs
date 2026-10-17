//! Two counters, one component: framework-owned state and focus.
//!
//! Run `cargo run -p jilva --example counters` in an interactive terminal.
//! Tab / Shift+Tab move focus; `+` and `-` change the focused counter; `q`
//! or Ctrl+C quits. Each counter keeps its own count in a slot the runtime
//! owns — the model below stores no counts at all.

use jilva::{
    Align, App, BorderStyle, Component, Constraint, Context, Effects, Event, Model, Outcome,
    ResolvedAttributes, View,
};

/// The component's configuration is just its title.
#[derive(Clone, PartialEq)]
struct Counter {
    title: &'static str,
}

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
        _effects: &mut Effects,
    ) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => *count += 1,
            Some("-") => *count -= 1,
            _ => return Ok(Outcome::Ignored),
        }
        Ok(Outcome::repaint())
    }

    fn view(&self, count: &i64, cx: &Context) -> jilva::Result<View> {
        let (style, attrs) = if cx.is_focused() {
            (BorderStyle::Heavy, ResolvedAttributes::plain().with_bold())
        } else {
            (BorderStyle::Rounded, ResolvedAttributes::plain())
        };
        Ok(View::text(&count.to_string())
            .text_align(Align::Center)
            .bordered(style, attrs)
            .title(self.title))
    }

    fn focusable(&self) -> bool {
        true
    }
}

/// The app only lays the counters out and handles quitting.
struct Screen;

impl Model for Screen {
    type Message = ();

    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        if event.key_text() == Some("q") {
            effects.quit();
            return Ok(Outcome::unchanged());
        }
        Ok(Outcome::Ignored)
    }

    fn view(&self, _cx: &Context) -> jilva::Result<View> {
        let card = |title| (Constraint::Length(16), View::component(Counter { title }));
        let hint = View::text("tab  focus    + / -  change    q  quit").text_align(Align::Center);
        Ok(View::column([
            (
                Constraint::Length(3),
                View::row([card("left"), card("right")])
                    .gap(2)
                    .align(Align::Center),
            ),
            (Constraint::Length(1), hint),
        ])
        .gap(1)
        .align(Align::Center))
    }
}

fn main() -> jilva::Result<()> {
    jilva::run(App::new(Screen))?;
    Ok(())
}
