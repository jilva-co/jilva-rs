//! NEGATIVE CASE — this file is intentionally NOT part of the build
//! (no `mod` refers to it). Verify the failures with:
//!
//! ```text
//! rustc --edition 2024 --crate-type=lib src/negative_send_bound.rs
//! ```
//!
//! Expected: compilation FAILS (that failure is the evidence).
//! See README.md for the captured compiler output.

use std::cell::RefCell;
use std::rc::Rc;

fn require_send<T: Send>(_: T) {}

/// Single-threaded model with shared-ownership interiors.
pub struct SharedModel {
    pub rows: Rc<RefCell<Vec<String>>>,
}

/// FAIL 1 — a universal `Model: Send` bound rejects the Rc/RefCell model.
pub fn loop_entry_with_send_bound(m: SharedModel) {
    require_send(m);
}

/// FAIL 2 — same for an `Rc`-cached slot memo.
pub fn slot_entry_with_send_bound(memo: Rc<String>) {
    require_send(memo);
}
