# RFC-0008: App Runtime and the First Component Slice

- Status: **Draft — slice A merged (PR #14); slice B implemented on branch
  `phase4-components`; awaiting a maintainer decision** (`docs/RFC/README.md`).
  Nothing here is Accepted until the maintainer records it in
  `docs/reviews/RFC_DECISION_TRACKER.md`.
- Author(s): Claude, for @hamroqulovv
- Created: 2026-10-17
- Related: `docs/RFC/0002-component-model.md` (accepted contracts this RFC
  implements), `docs/RFC/0001-framework-scope.md` (Q1 framework owns the loop),
  `docs/RFC/0004-rendering-engine.md`, `docs/RFC/0006-view-description.md`,
  `docs/adr/0004-product-direction.md` (order of work, step 1),
  `docs/ARCHITECTURE.md` §3.2, §3.4, §6

## Summary

The first runnable Jilva application: a counter you start with one call, drive
with the keyboard, and quit with the terminal restored. It ships in two slices:

- **Slice A — app runtime.** A `Model` trait (Elm-shaped `update` + pure
  `view`), a *sans-IO* `App` runtime in `jilva-core` that turns
  (input, time) into output commands, and a small driver in `jilva-terminal`
  that pumps a real or headless backend. Deliverable: `examples/counter.rs`.
- **Slice B — components.** The RFC-2 `Component` trait with framework-owned
  state slots, identity, a single focus path and bubbling dispatch.
  Deliverable: `examples/counters.rs`, two independent counters with Tab focus.

This RFC fixes the details RFC-2 delegated to "the implementation RFC": the
`Outcome`/`Change` carrier, the effect queue, the clock source, where the loop
lives, and the misuse behaviour of each.

## Motivation

Twelve thousand lines in, no one can write a Jilva app: there is a terminal
engine, a renderer, layout and `View`, but no loop. Every later feature in
ADR-0004 (streaming transcript, prompt input, inline mode, record/replay)
needs one. The runtime is also where reproducibility is won or lost: if the
loop reads clocks or performs I/O in the middle of dispatch, replay is
impossible later. Designing it sans-IO now makes record/replay a file format,
not a rewrite.

## Proposal

### 1. What the user writes (slice A)

```rust
// PROPOSED API — not final, not compiling
use jilva::{Align, App, Context, Effects, Event, Model, Outcome, View};

struct Counter {
    count: i64,
}

impl Model for Counter {
    type Message = ();

    fn update(&mut self, event: Event<()>, effects: &mut Effects<()>) -> jilva::Result<Outcome> {
        match event.key_text() {
            Some("+") => self.count += 1,
            Some("-") => self.count -= 1,
            Some("q") => effects.quit(),
            _ => return Ok(Outcome::Ignored),
        }
        Ok(Outcome::repaint())
    }

    fn view(&self, _cx: &Context) -> jilva::Result<View> {
        Ok(View::text(&format!("count: {}", self.count)).text_align(Align::Center))
    }
}

fn main() -> jilva::Result<()> {
    let app = jilva::run(App::new(Counter { count: 0 }))?;
    println!("final count: {}", app.model().count);
    Ok(())
}
```

About 25 lines, no cell math, no terminal setup, no focus or identity
boilerplate — within the RFC-2 brevity bar (counter ~15 lines of logic).

### 2. Core vocabulary (`jilva-core`)

- **`Event<M>`** replaces the Phase-1 placeholder `Event { Tick, Resized,
  Custom(String) }` (unreleased `0.0.0`; deliberate break recorded in the
  changelog). `#[non_exhaustive]`, RFC-2 §4 taxonomy:
  `Key(KeyEvent)`, `Mouse(MouseEvent)`, `Paste(String)`,
  `Resize { cols, rows }`, `TerminalFocus(bool)`, `Message(M)`.
  `Tick(Duration)` and component `FocusGained`/`FocusLost` arrive with the
  slices that produce them. Helper: `Event::key_text() -> Option<&str>` —
  printable text of a key press with no Ctrl/Alt/Super held.
- **`KeyEvent` matchers:** `is_text(&str)`, `is_special(SpecialKey)`,
  `is_ctrl(&str)`, so apps never compare modifier structs by hand.
- **`Change`** — `None < Repaint < Layout` (RFC-2 §5 names), `Ord`, `max`.
- **`Outcome`** — the RFC-2 §5 carrier, both axes in one value:
  `Ignored` (not consumed, nothing changed) or `Consumed(Change)`. Helpers
  `Outcome::repaint()`, `Outcome::layout()`, `Outcome::unchanged()`.
  Over-reporting is always correct; when unsure, return `Outcome::layout()`.
- **`Effects<M>`** — the bounded effect queue RFC-2 §1 rule 1 gives `update`:
  `quit()` and `send(msg)` (FIFO follow-up, processed after the current update
  — never reentrant). `after(duration, msg)`, `focus(id)` and `emit(msg)`
  arrive with later slices.
- **`Context`** — read-only input to `view`: `size()`, `capabilities()`,
  `now()` (injected time since start). Theme tokens join it with RFC-3.
- **`Model`** — the app root:

  ```rust
  // PROPOSED API
  pub trait Model {
      type Message;
      fn update(&mut self, event: Event<Self::Message>, effects: &mut Effects<Self::Message>)
          -> Result<Outcome>;
      fn view(&self, cx: &Context) -> Result<View>;
  }
  ```

  `update` is fallible because RFC-2 §6 makes it so; `view` is fallible
  because RFC-2 §8 pipeline rule 2 defines a failed describe.

### 3. The sans-IO runtime

`App<M: Model>` lives in `jilva-core`. It performs no I/O, reads no clock and
names no backend. Its whole interface is the object-safe `Driven` trait:

```rust
// PROPOSED API
pub trait Driven {
    /// Enter: first frame for this terminal.
    fn start(&mut self, caps: &TerminalCapabilities, size: (u16, u16), now: Duration)
        -> Result<Vec<OutputCommand>>;
    /// One source event in, at most one frame of commands out.
    fn handle(&mut self, input: InputEvent, now: Duration) -> Result<Vec<OutputCommand>>;
    /// True once the model requested quit (or the default Ctrl+C fired).
    fn is_finished(&self) -> bool;
}
```

`App::handle` implements RFC-2 §1 exactly:

1. Translate `InputEvent` into `Event<M>`. Key **releases are dropped** by
   default (Windows reports them; naive apps would double-count). Resize
   resizes the renderer, invalidates it and forces `Change::Layout`.
2. Call `update` once. If the root returns `Ignored` for Ctrl+C, the runtime
   quits — a calm default an app overrides by consuming Ctrl+C.
3. Drain `Effects::send` messages FIFO through `update`, at most **1024** per
   source event; beyond that, return `Error::UpdateOverflow` (new additive
   variant, named by RFC-2 §1 rule 4) instead of hanging.
4. Take the maximum `Change` over the batch. `None` skips `view` entirely and
   emits nothing. Otherwise call `view` once, then `Renderer::render` once.
   Slice A has no layout cache, so `Repaint` and `Layout` both run the full
   pipeline; the distinction is recorded now and becomes an optimisation later.
5. In debug builds, when the batch reported `None`, the runtime still calls
   `view` and `debug_assert`s it equals the last `View` — the RFC-2 §5
   under-reporting check (`View: PartialEq` makes it exact).

Errors from `update` or `view` abandon the batch: nothing is painted, the
previous frame stays on screen, and the error returns to the driver, which
restores the terminal and returns it to `main`.

Determinism (RFC-2 §4): same model, capabilities, size and
(time, input) sequence ⇒ bit-identical command stream. This is the property
ADR-0004 record/replay builds on, and slice A tests it directly.

### 4. The driver (`jilva-terminal`)

```rust
// PROPOSED API
pub fn run<D: Driven>(app: D) -> Result<D>;                       // feature "crossterm"
pub fn run_with<D: Driven, B: Backend>(backend: &mut B, app: D) -> Result<D>;
```

About 50 lines: `enter` → `start` → loop { `poll` → `read` → `handle` →
`submit` → `flush` } until `is_finished` → `leave`. It reads the wall clock
(`Instant` at start; `now` is elapsed time) and converts `io::Error` into
`Error::Io`. A drop guard calls `leave` on every exit path — normal return,
`?` error, and panic unwind (abort is out of reach, as Phase 2 documents).
The driver sees only `InputEvent`s, `OutputCommand`s and the `Driven` trait;
it never names `Model`, components or styles.

Headless tests either call `Driven::handle` directly with scripted time (the
pure path, preferred) or use `run_with` on a `TestDoubleBackend` whose script
ends in a quit key.

### 5. Slice B — components

Implements RFC-2 §§2–8 for the smallest useful case. Decisions below marked
*(owner)* were made by the project owner in conversation on 2026-10-17.

- **`Component` trait** (`jilva-core::component`):
  `trait Component: Clone + PartialEq + 'static` with `type State`,
  `type Message`, and required `init`, `update`, `view`. `update` has the
  same `(state, Event<Message>, Effects) -> Result<Outcome>` shape as
  `Model`, so there is one mental model. Defaulted: `cleanup` (drops the
  state) and `focusable() -> bool` (false). *(owner: `cleanup` is defaulted —
  this amends RFC-2 §7's "four required methods" to three; see
  "Amendment to RFC-2" below.)*
- **Placement** *(owner, open question 2)*: `View::component(c)`. `Self` is
  the component's configuration; `Clone + PartialEq` keep `View:
  Clone + PartialEq + Eq`, so views compare components by type and
  configuration and the debug under-report check stays exact. Painting a
  view outside the runtime draws a component as nothing.
- **Identity:** a path of segments from the root view — child index per
  container edge, `Inside` at a component's own view root. `View::key(k)`
  wraps *any* view; the key replaces that view's edge from its parent, so key
  the outermost view of each sibling. `Key` converts from `u64`, `u32`,
  `usize`, `&str` and `String`. Misuse per the RFC-2 §2 table: mixed keyed /
  unkeyed siblings fall back to positions; a duplicate key gets fresh state
  every frame (init, view, cleanup, never focusable). Both `debug_assert!`;
  release builds never panic. A type change at the same identity drops the
  old slot (with `cleanup`) and initialises a new one.
- **Slots:** a hand-rolled generational table (RFC-2 F2.3); a stale handle
  resolves to nothing. Expansion is all-or-nothing: if any `init` or `view`
  fails, slots created for that frame are cleaned up, nothing is dropped,
  and the identity map stays as it was (pipeline rule 2). Dropping the `App`
  cleans up every mounted slot.
- **Dispatch:** keys and paste go to the focused component, then each
  ancestor component (leaf first), then the `Model`; the first `Consumed`
  stops them. Resize goes to the model and then every component, parents
  first, and always repaints. Mouse and terminal-focus events go to the model
  only until hit-testing exists. Follow-ups of the model and of every
  component share one 1024 budget per source event.
- **Focus** *(owner, open question 4)*: one focus path. When nothing consumed
  it, Tab moves focus forward and Shift+Tab / BackTab back, through focusable
  components in paint order, wrapping; a focus move repaints. If the focused
  component unmounts, or nothing is focused, focus settles on the first
  focusable component (the frame is described once more so `is_focused` is
  right). `Context::is_focused()` lets a component draw its own focus ring.
- **Debug checks:** besides the model check, a component that reports no
  change must describe the same view after its update as before.
- **Demo:** `examples/counters.rs` — two counters; each keeps its own count
  across focus changes and resizes, and the model stores no counts.

Known gaps against RFC-2, recorded rather than hidden:

- RFC-2 F2.2 targets "never per-frame boxing in steady state". Component
  configurations are boxed each time a view is described, like the rest of the
  owned `View`; state is boxed once at mount. The homogeneous fast path is
  later work.
- Keys are a closed set of conversions, not any `Clone + Eq + Hash` type.
- `FocusGained` / `FocusLost` events are not delivered yet; components read
  `is_focused` in `view`.

Out of slice B (next slices): `Effects::emit` to parents and
`Effects::focus(id)`, ticks and subscriptions, mouse hit-testing, focus
scopes.

### Amendment to RFC-2 (proposed with this RFC)

RFC-2 §7 fixes four required methods (`init`, `update`, `view`, `cleanup`).
This RFC makes `cleanup` a defaulted method that drops the state, leaving
three required methods. Rationale: most components own nothing that needs
explicit release, and an always-empty required method is boilerplate every
third-party widget writes forever — the very cost §7 uses to justify keeping
the count small. `cleanup` still runs exactly once per mounted slot, and
widgets that own resources override it. The project owner chose this on
2026-10-17; it becomes normative only when this RFC is accepted.

## Implementation plan

Slice A (one PR):

1. `Change`, `Outcome`, helpers, unit tests.
2. `Event<M>` replacing the placeholder; `key_text` and `KeyEvent` matchers;
   update `tests/facade_smoke.rs` and `examples/version.rs`.
3. `Effects<M>` with the 1024 cap; `Error::UpdateOverflow`.
4. `Context`; `Model`; `Driven`; `App<M>` runtime with release filtering,
   Ctrl+C default, FIFO drain, change gating, resize, debug under-report check.
5. `run` / `run_with` driver with the restoration guard.
6. Facade re-exports; `examples/counter.rs` wired in `crates/jilva/Cargo.toml`.
7. `tests/app_runtime.rs`: scripted `+ + - q` gives count 1 and a golden
   frame stream; same script twice gives identical bytes; `None` emits zero
   commands; release keys ignored; Ctrl+C default; `update` error and `view`
   error leave the previous frame and restore the test double; a panicking
   `update` under `catch_unwind` still leaves the session; effect overflow
   returns `UpdateOverflow`.
8. CHANGELOG, ROADMAP, README status; full CI command set in both feature
   modes.

Slice A is done when `cargo run -p jilva --example counter` works in a real
terminal (`+`/`-` change the count, `q` and Ctrl+C quit, terminal restored)
and every test above passes headless on all three CI platforms.

Slice B (second PR): slot table and identity, `Component` trait, focus path
and dispatch, `View::component`, `examples/counters.rs`, identity/reorder/
focus tests, docs.

## Prior art

- **Bubble Tea:** `Init`/`Update`/`View`, `tea.Quit` as a command, key
  messages delivered to `Update` like any other message. Borrowed: one
  `update` receiving every event, quit as an effect. Adapted: Go returns a new
  model; here `update` mutates through `&mut` (RFC-2 F2.6).
- **Elm:** the loop shape and "view is a pure function of the model".
- **Ratatui:** the application owns the loop and the terminal setup.
  Deliberately not borrowed — RFC-1 Q1 makes the loop a framework duty, so
  restoration and ordering are guaranteed rather than advised.
- **Sans-IO protocol libraries** (state machines that consume bytes and emit
  bytes, with the caller doing I/O): the model for `App` + `Driven`, chosen
  so replay and headless tests need no special mode.

## Non-goals

Ticks and animation wiring, an external-thread message channel, inline mode,
theme tokens, mouse dispatch, async integration, record/replay file format —
each has its own later slice or RFC (ADR-0004 order of work).

## Alternatives considered

1. **Driver in the facade.** Smallest change, but breaks ARCHITECTURE §6
   rule 4 ("facade contains no logic") for good.
2. **New `jilva-runtime` crate** depending on core and terminal. Cleanest in
   letter, but adds a crate outside the reserved list and the extraction rule
   (§4.2) for ~50 lines of glue.
3. **Move the `Backend` trait into core** so the runtime calls it directly.
   Reopens the accepted Phase-2 seam and puts I/O-shaped calls in core.
4. **User-owned loop (Ratatui style).** Rejected by RFC-1 Q1.
5. **Separate `on_input → Option<Msg>` mapping plus `update(msg) -> Change`.**
   Purer Elm, but two methods for every app and a mandatory message enum for
   the simplest one; one `update` over `Event<M>` matches RFC-2 §4.

## Open questions

1. **Where the driver lives — decided by the project owner in conversation on
   2026-10-17:** `jilva-terminal`, with ARCHITECTURE §6 rules 1 and 3
   clarified ("terminal may pump a core-defined `Driven` object; it owns no
   dispatch, update or render logic"). This settles only this question; the
   RFC as a whole remains Draft.
2. **Component nodes inside `View` — decided by the owner on 2026-10-17:**
   `View::component(c)` with `Component: Clone + PartialEq + 'static`
   (rejected: a separate `Element` tree; `Rc` identity equality).
3. **`cleanup` — decided by the owner on 2026-10-17:** defaulted; see
   "Amendment to RFC-2".
4. **Calm defaults — decided by the owner on 2026-10-17:** Tab / Shift+Tab
   move focus when nothing consumed them. Dropping key releases and Ctrl+C
   quitting when ignored shipped in slice A.
5. The 1024 follow-up cap: keep as a constant, or make it an `App` option?
6. Name of `Driven` (`Program`, `Session` are alternatives).

## Drawbacks

- `Result<Outcome>` from every `update` costs an `Ok(…)` per return; the price
  of RFC-2 §6 fallibility and of `?` inside handlers.
- The `Driven` indirection is one more concept, visible only to people writing
  custom drivers.
- Replacing `Event` breaks the Phase-1 placeholder (no release has shipped).
- Debug builds call `view` on `None` batches; release builds do not.
