//! NEGATIVE CASE — this file is intentionally NOT part of the build
//! (no `mod` refers to it). Verify the failures with:
//!
//! ```text
//! rustc --edition 2024 --crate-type=lib src/negative_not_dyn_compatible.rs
//! ```
//!
//! Expected: compilation FAILS (that failure is the evidence).
//! See README.md for the captured compiler output.

/// Full RFC-0002-style component trait: associated State/Message plus
/// receiver-less `init` / `update` with per-widget concrete types.
pub trait Component {
    type State;
    type Message;
    fn init() -> Self::State;
    fn update(slot: &mut Self::State, msg: Self::Message) -> &'static str;
}

pub struct Header;
pub struct TodoList;

impl Component for Header {
    type State = String;
    type Message = char;
    fn init() -> String {
        String::new()
    }
    fn update(slot: &mut String, msg: char) -> &'static str {
        slot.push(msg);
        "repaint"
    }
}

impl Component for TodoList {
    type State = Vec<String>;
    type Message = usize;
    fn init() -> Vec<String> {
        Vec::new()
    }
    fn update(slot: &mut Vec<String>, msg: usize) -> &'static str {
        slot.push(format!("row{msg}"));
        "layout"
    }
}

// FAIL 1 — the RFC-0002 sketch shape: heterogeneous array literal.
// There is no common element type, so even the *syntax* is rejected.
pub fn sketch_column() {
    let _kids = [Header, TodoList];
}

// FAIL 2 — the full trait cannot be a `dyn` object: receiver-less methods
// plus unspecified associated types.
pub fn erased(_: &dyn Component) {}
