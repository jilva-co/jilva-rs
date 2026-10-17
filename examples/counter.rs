//! The first whole Jilva app: a counter.
//!
//! Run `cargo run -p jilva --example counter` in an interactive terminal.
//! `+` and `-` change the count; `q` or Ctrl+C quits and restores the
//! terminal. The model never touches the terminal: `update` changes state,
//! `view` describes the screen, and the runtime sends only what changed.

use jilva::{
    Align, App, BorderStyle, Constraint, Context, Effects, Event, Model, Outcome,
    ResolvedAttributes, View,
};

struct Counter {
    count: i64,
}

impl Model for Counter {
    type Message = ();

    fn update(&mut self, event: Event, effects: &mut Effects) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => self.count += 1,
            Some("-") => self.count -= 1,
            Some("q") => effects.quit(),
            _ => return Ok(Outcome::Ignored),
        }
        Ok(Outcome::repaint())
    }

    fn view(&self, _cx: &Context) -> jilva::Result<View> {
        let card = View::text(&format!("count: {}", self.count))
            .text_align(Align::Center)
            .bordered(BorderStyle::Rounded, ResolvedAttributes::plain())
            .title("counter");
        let hint = View::text("+ / -  change    q  quit").text_align(Align::Center);
        Ok(View::column([
            (
                Constraint::Length(3),
                View::row([(Constraint::Length(26), card)]).align(Align::Center),
            ),
            (Constraint::Length(1), hint),
        ])
        .gap(1)
        .align(Align::Center))
    }
}

fn main() -> jilva::Result<()> {
    let app = jilva::run(App::new(Counter { count: 0 }))?;
    println!("final count: {}", app.model().count);
    Ok(())
}
