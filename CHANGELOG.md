# Changelog — jilva-rs

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project is pre-`1.0`: minor versions may break with migration notes,
patch versions never break. Any breaking change while in `0.x` still requires
an RFC amendment (see `docs/ARCHITECTURE.md` §3.1).

## [Unreleased]

### Changed

- `App::new` is no longer `const` (the component tree needs a hash map).

- Breaking (unreleased placeholder): the Phase-1 `Event { Tick, Resized,
  Custom(String) }` is replaced by `Event<M>` with `Key`, `Mouse`, `Paste`,
  `Resize`, `TerminalFocus` and `Message(M)`.

### Added

- Components, RFC-0008 slice B (Draft): the `Component` trait (required
  `init`, `update`, `view`; defaulted `cleanup` and `focusable`),
  `View::component`, `View::key` and `Key`. The runtime gives each mounted
  component a framework-owned state slot in a generational table, keyed by
  its path or key; expansion is all-or-nothing on errors. Keys go to the
  focused component, its ancestors, then the model; resizes reach every
  component. Tab / Shift+Tab move focus when nothing consumed them;
  `Context::is_focused`. `counters` example; `tests/components.rs`.
- App runtime slice A (RFC-0008 Draft): the `Model` trait, `Event<M>`
  dispatch taxonomy with `KeyEvent::{is_text, is_special, is_ctrl,
  plain_text}`, `Outcome`/`Change`, `Effects` (quit, FIFO follow-ups, 1024
  cap with `Error::UpdateOverflow`), `Context`, and the sans-IO `App` runtime
  behind the `Driven` trait. `jilva::run` / `run_with` drive it from
  `jilva-terminal`, restoring the terminal on return, error and panic unwind.
  Defaults: key releases dropped, Ctrl+C quits when ignored. `counter`
  example; `tests/app_runtime.rs`; `TestDoubleBackend::fail_poll_when_exhausted`
  so a script without a quit fails instead of spinning.

- Colour blending and motion (RFC-0007 Draft): `ResolvedColor::{to_rgb, blend}`,
  `Buffer::map_attrs`, `View::{gradient, gradient_text, tint}` with `Backdrop`,
  and `Easing`, `Lerp`, `Transition` (injected time, retargeting, reduced
  motion); `motion_demo` example. ADR-0004 records the beauty-first product
  direction, amended by the owner on 2026-10-17: AI-agent and streaming CLIs
  first, reproducibility as the second headline, an offline agent-chat demo
  instead of the dashboard.
- RFC-0008 Draft (`docs/RFC/0008-app-runtime.md`): plan for the app runtime
  (sans-IO `App`, `Model`, `Outcome`/`Change`, `Effects`, terminal driver) and
  the first `Component` slice; counter and two-counter examples. Not
  implemented.
- `View` (RFC-0006 Draft): an owned, geometry-free frame description with text
  and spans, row/column, overlay, padding and borders; `View::paint` and
  `Renderer::render` (describe → paint → diff); `view_demo` example.
- Phase 4 first slice (RFC-0005 Draft): `Layout` flex row/column solver with
  `Constraint`, `Direction`, `Align`, gap, a deterministic remainder rule and a
  never-panic overflow policy; `Rect::{right, bottom, intersection, inner}`.
- Phase 3 rendering engine (`jilva-core::renderer`, RFC-0004 Draft): a
  grapheme-cluster `Cell`, a bounds-disciplined `Buffer` that keeps
  double-width glyphs whole, damage tracking (`Damage`), a pure `diff` /
  `redraw` producing `OutputCommand` streams (coalescing, attribute tracking,
  cursor minimisation, capability-effective equality), and a double-buffered
  `Renderer`. Width and segmentation use `unicode-width` (no `cjk`) and
  `unicode-segmentation`. Facade re-exports, `render_demo` example, golden and
  round-trip property tests, and a std-only diff benchmark.

- Phase 2 synchronous terminal engine: optional Crossterm backend, tracked
  session cleanup, typed input/output, capability snapshots, and deterministic
  headless failure injection. The public facade exposes the backend contract.
- Real terminal and headless examples, multi-platform headless CI, terminal
  implementation/testing guides, and independent audit records.
- Maintainer-authorized RFC acceptance revalidation dated 2026-10-17, preserving
  earlier Discussion history and deferred implementation decisions.
- Foundation documentation: system boundaries, crate structure, and dependency
  rules (`docs/ARCHITECTURE.md`); API principles and facade contract
  (`docs/API_DESIGN.md`).
- Seven-phase roadmap from foundation to alpha release (`docs/ROADMAP.md`).
- Contributor setup guide (`docs/GETTING_STARTED.md`).
- Rewritten project README with status, philosophy, contributor commands, repo
  structure, and architecture overview.
- Workspace contribution rules and everyday `cargo` commands (`CONTRIBUTING.md`).

### Changed

- **Breaking (pre-release):** `Cell` is no longer `{ symbol: char }`.
  `Cell::new(char)` is replaced by `Cell::try_new(&str, attrs)` /
  `Cell::from_char(char, attrs)`; the symbol is read with `Cell::symbol()`.
  `jilva-core` gains two third-party dependencies (`unicode-width`,
  `unicode-segmentation`; no transitive dependencies).
