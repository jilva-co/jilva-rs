# Jilva project guide

Updated **2026-10-18** for renderer optimization and the framework workbench. This is the single maintained
design and planning document. [Quick start](../README.md#try-it) ·
[Architecture](#architecture) · [Comparison](#framework-comparison) ·
[Plan](#development-plan) · [Decisions](#design-decisions).

## Current implementation

| Area | Working now | Missing |
| --- | --- | --- |
| [Terminal](../crates/jilva-terminal/src/backend.rs) | Optional Crossterm, typed input/output, cleanup, capability snapshots, fault-injecting test backend | Managed inline viewport and shell scrollback |
| [Renderer](../crates/jilva-core/src/renderer.rs) | Grapheme cells, wide-glyph repair, clipping, diff, explicit damage regions, colour degradation | Runtime-generated damage; `Renderer::render` still paints and compares the whole viewport |
| [Layout and views](../crates/jilva-core/src/view.rs) | Flex rows/columns, constraints, spans, opt-in grapheme-safe wrapping, overlays, axis padding, borders, gradients | Intrinsic measurement, language-specific line breaking, scrolling, grid |
| [Runtime](../crates/jilva-core/src/lifecycle/app.rs) | `Model`, `App`, FIFO follow-ups, ignored-event fast path, injected time | External message ingress, timer events, animation wakeups, cancellation |
| [Components](../crates/jilva-core/src/component.rs) | Framework-owned slots, keys, cleanup, Tab focus, bubbling | Parent messages, focus events/scopes, mouse hit-testing; configurations are boxed per description |
| [Style](../crates/jilva-core/src/style.rs) | 14 semantic roles, dark/light themes, partial overrides, extensions, composed states, cached tier resolution, runtime switching | Configurable overlay definitions, theme-file adapters |
| [Motion](../crates/jilva-core/src/motion.rs) | Easing, retargetable transitions, injected time, reduced-motion sampling | Runtime scheduling and theme/component integration |

Studio exercises component-owned state, adjustable layout, Unicode wrapping,
focus, and resizing in a monochrome workbench; counters provide smaller examples.
Scripted output equality is tested; no session file recorder/replayer exists.
See [app tests](../tests/app_runtime.rs), [component tests](../tests/components.rs),
and [render tests](../tests/render_pipeline.rs) for executable evidence.

## Architecture

```text
app -> jilva -> jilva-core
            -> jilva-terminal -> jilva-core
                              -> optional Crossterm
```

- `jilva` curates the public API; behaviour belongs in core or terminal.
- Core has no terminal I/O, environment reads, OS branching, clock reads, or
  executor. Its only external dependencies are the two Unicode authorities.
- Terminal owns detection, session resources, the clock, and the event pump.
  It drives the opaque `Driven` interface; it never inspects app/component state.
- State and capability data flow explicitly. Models and slots need no universal
  `Send`/`Sync`; only values actually crossing a future bridge need such bounds.
- Examples, benches, and integration tests import the facade. Their targets are
  declared in `crates/jilva/Cargo.toml`; the root manifest is a virtual workspace.
- Keep three crates until a measured need justifies extraction. Dependencies
  need a stated purpose; safe Rust, edition 2024, and MIT remain workspace rules.

### Runtime

App domain state belongs to `Model`; ephemeral component state belongs to
framework slots. Pure `view` returns an owned, geometry-free `View`.
One source event drains FIFO follow-ups before at most one submitted frame;
a shared 1024 follow-up budget reports `Error::UpdateOverflow`. This is a
processing limit, not a memory bound on `Effects::send`.
No partial frame is submitted after an update/view error; model mutation is
not rolled back. Focus settling can describe a frame again before submission.

Keys and paste route focused component -> ancestors -> model until consumed.
Unconsumed Tab / Shift+Tab traverse focus; unconsumed Ctrl+C quits. Releases
are dropped. Resize reaches model then all components and forces repaint.
Mouse and terminal-focus events currently reach only the model.
`Effects::send` sends local follow-ups, not cross-thread or parent messages.

Use keys on the outermost view of dynamic siblings. State follows a key;
unkeyed state follows position. Mixed siblings fall back to position; duplicate
keys keep the first slot and give extras fresh ephemeral state. Both misuse
cases assert in debug builds. Replacement changes type => cleanup and remount.
Failed expansion removes new slots while preserving the previous identity map.

### Terminal

`Backend` owns `enter/leave`, `size/poll/read`, `submit/flush`, and cached
capabilities. One live session per instance; `leave` is idempotent; operations
requiring a session fail outside it. Setup failures roll back acquired modes,
and cleanup retries failed steps. The driver attempts restoration on normal
return, errors, and panic unwind; cleanup failures can still leave modes active.
Abort, forced termination, and physical emulator behaviour need separate care.

Submissions validate the entire batch before buffering. Control bytes in
printed text are rejected; a failed flush discards the attempted batch rather
than replaying possibly written bytes. Resize invalidates stale geometry.
Capabilities have independent colour/input/screen versions; consumers compare
their relevant domains. Detection is cached, conservative, and overrideable.
Unknown colour evidence defaults to ANSI16; any present `NO_COLOR`, including
empty or `0`, selects mono. Optional input modes require explicit support/options.
The real backend requires a TTY; a future plain-output adapter is separate work.

### Rendering and layout

The pipeline is describe -> expand components -> layout/paint -> diff -> backend.
`unicode-segmentation` defines clusters; `unicode-width` without `cjk` defines
cell widths (ambiguous characters use one cell). Terminal width disagreement
remains possible. Clip/truncate without splitting graphemes or wide-cell pairs.
Text is data, never raw ANSI. Layout consumes measured cell sizes.
`View::wrap` preserves span styles and explicit text lines, prefers ASCII-space
breaks, and splits long words at grapheme boundaries. A two-cell glyph in a
one-column region becomes a styled blank. Parents still allocate row counts.
`View::padding_xy` and `Rect::inner_xy` separate horizontal/vertical spacing.
Flex remainders use largest fraction, earlier item wins ties; overflow makes
later items yield first. Tiny regions clip safely; a running app rejects zero size.
`Change::None < Repaint < Layout` is the invalidation contract, but repaint/layout
currently share the full paint path. Damage regions help only when supplied by
a lower-level caller; sparse terminal output alone does not prove cheap updates.

### Theming contract

The implementation provides 14 roles: Background, Surface, Overlay, Border,
Text, Muted, Accent, Success, Warning, Danger, Info, Selection, Cursor, Highlight.
Two built-ins (`jilva-dark`, `jilva-light`) have automatic mono
resolution. App deltas override a base, with dark filling missing tokens.
Merge tokens first, apply state overlays, then degrade to terminal capabilities.
Overlay order is Hovered < Active < Focused < Selected; Disabled overrides all.
Focus/selection must remain distinguishable without colour. Missing extension
tokens fall back to Text with a debug marker. Theme switches invalidate paint,
not geometry. Merged palettes and resolved caches are separate values: the app's
theme revision changes only for a different palette; input, time, and resize
reuse resolution. Use `App::with_theme`, `Context::style/style_with`, and
`Effects::set_theme`. Requests commit after successful update/follow-up dispatch,
last request wins; a repeated palette needs no repaint. Raw `View::text` remains
terminal-default text; pass semantic attributes to styled text, spans, and borders.
Missing extensions return Text plus an explicit `missing_extension` diagnostic.
Disabled is visual state, so widgets must still reject disabled interaction.
Mono retains supported reverse video; widgets need structural markers for
independent focus/selection indications. Studio uses a filled row for selection;
its live component uses a heavy border and a `>` marker for focus.

`SemanticToken::Selected` retains its original name and resolves as Selection.
Migration: `Context` is now cheaply `Clone` rather than `Copy`, sharing the
resolved cache; use `.clone()` when retaining a context. Its `new` and
`with_focused` are no longer const; the latter borrows the source context.
The initial overlays have fixed transforms; configurable transforms remain
follow-up work. [Theming tests](../tests/theming.rs) cover switching/state/focus
and all terminal tiers. `themes --headless` exercises three working examples and sizes
across all tiers. `--snapshots DIR` exports actual renderer commands as SVG/text;
SVG pixels assume a monospace font and the documented standard ANSI palette,
so physical emulator/font checks remain separate.
The value-based overrides and role vocabulary take cues from
[Cursive palettes](https://docs.rs/cursive/latest/cursive/theme/index.html) and
[Textual theme variables](https://textual.textualize.io/guide/design/);
this implementation keeps resolution pure and adds no parser dependency.

The default visual language uses black, white, and neutral grays. Status roles
need explicit labels or symbols; colour is not their sole signal. Built-in text
roles meet a 4.5:1 contrast floor on
Background/Surface/Overlay in truecolor, 256-colour, and the standard ANSI16
table; user-remapped ANSI colours can differ. Selection/cursor/highlight pairs
are checked too. 256-colour conversion chooses the nearest stable cube or gray
entry, preserving near-neutral surface depth. The [Studio](../examples/themes.rs)
uses semantic attributes and stable keys. Counter, flex split, and wrapping
settings live in one framework-owned component state slot. Switching examples,
palettes, or viewport sizes preserves that state and focus, including the tiny
window fallback. Its geometry fixture covers 120×36, 80×24, and 40×18.

## Framework comparison

Primary sources checked **2026-10-17**. This is a documentation comparison,
not a benchmark or a claim that unlisted features are absent.

| Framework | Already documented | Implication for Jilva (our assessment) |
| --- | --- | --- |
| Ratatui (Rust) | [Inline/fixed/fullscreen viewports](https://docs.rs/ratatui/latest/ratatui/enum.Viewport.html), [test backend](https://ratatui.rs/concepts/backends/) | Diffing, inline mode, and headless tests are baseline capabilities |
| Cursive (Rust) | [Views, callbacks, themes](https://docs.rs/cursive/latest/cursive/) | A reusable component model and themes alone do not establish a difference |
| iocraft (Rust) | [Declarative flexbox, hooks, animation](https://docs.rs/iocraft/latest/iocraft/) | Readable layout and attractive defaults already have Rust alternatives |
| R3BL TUI (Rust) | [Tokio architecture, Markdown editor, partial/full TUIs](https://docs.rs/crate/r3bl_tui/latest) | Prove useful streaming primitives while retaining a synchronous core |
| Bubble Tea (Go) | [Elm model, inline/full-window apps, Bubbles components](https://github.com/charmbracelet/bubbletea) | Measure complete app ergonomics against an integrated ecosystem |
| Textual (Python) | [Pilot and SVG snapshots](https://textual.textualize.io/guide/testing/), [devtools](https://textual.textualize.io/guide/devtools/) | Regression debugging needs an easy user workflow, beyond a test double |
| OpenTUI (TS/Zig) | [Sticky scrolling/culling](https://opentui.com/docs/components/scrollbox/), [input/clock/frame testing helpers](https://opentui.com/docs/core-concepts/testing/) | Streaming scroll behaviour and test tools must be first-class |
| FrankenTUI (Rust) | [Agent harness and inline logs](https://github.com/Dicklesworthstone/frankentui/blob/main/docs/tutorials/agent-harness.md), [recorder/replayer and frame comparisons](https://github.com/Dicklesworthstone/frankentui/blob/main/CHANGELOG.md#deterministic-replay) | Replay and agent focus already overlap; compete on the complete small API |
| TachyonFX (Ratatui ecosystem) | [Composable effects and animation](https://github.com/ratatui/tachyonfx) | Motion alone is not a unique feature; prioritize purposeful, reduced motion |

## Product direction

**Proposed product promise: build a polished streaming CLI and turn a failing
session into a regression fixture using the same app.** This refines the existing
agent-CLI direction; it remains a hypothesis until real users validate it.

The three priorities are a virtualized transcript with stable scroll position,
consistent themed prompt/tool/approval components, and replay of all runtime
inputs with a useful first-divergence report. Inline scrollback and bounded
producer queues connect these into a usable shell workflow.
Networking, LLM providers, tool execution, and executors belong to the app.
No GUI, web renderer, plugin system, or sibling-language port before a usable
Rust alpha. General TUIs remain supported; the streaming demo sets priorities.

## Development plan

Each milestone is a small sequence of reviewable PRs, in dependency order.
Milestone 1 is implemented; milestones 2–7 remain planned.
Visual quality is a release gate: review real renderer frames in dark/light and
mono at wide/compact sizes, preserve focus/state through resize, and keep the
primary action and exit visible. New widgets must meet the same baseline.

| Order | Deliverable and code area | Acceptance gate |
| --- | --- | --- |
| 1 | Theme resolver in `style.rs`, context/view integration, facade exports | Reconcile 4 vs 14 tokens; dark/light snapshots across truecolor/256/16/mono; partial overrides and Selected+Focused/Disabled cases; theme switch leaves geometry/focus intact |
| 2 | External typed messages and scheduled wakeups in runtime/driver | Bounded ingress with explicit full/disconnected results; per-turn fairness; deterministic ordering; cancellation/shutdown; fake-clock animation advances without keyboard input and stops waking when idle |
| 3 | Intrinsic measurement, cached reflow, viewport, multiline prompt (basic wrapping implemented) | Grapheme editing, paste limits, history; only visible transcript content painted; append preserves a scrolled-up anchor; resize reflows correctly; configurable byte/item/cache caps survive overflow |
| 4 | Managed inline mode plus plain transcript adapter in terminal/renderer | Interactive footer + committed scrollback; one terminal writer; resize does not duplicate/erase history; normal/error/unwind restoration; redirected mode emits text without cursor controls |
| 5 | Offline `agent_chat` example and reusable streaming components | Scripted replies, Markdown subset, code/tool/diff cards, approval; arbitrary chunk boundaries preserve final text; keyboard-only use, cancellation, no network/API key; same script drives headless tests |
| 6 | Opt-in recording/replay adapter and regression helper | Versioned initial state fixture + capabilities, dimensions, theme, width-policy/build identity, time, ordered input/messages/completions; replay runs no live effects; two replays match frames; report first mismatching event/cell |
| 7 | Alpha completion, measured performance, external use | Text/list/table/input/tabs/status bar and accepted grid scope; tested examples/package; platform/terminal evidence; two independently written downstream apps; migration notes and declared MSRV policy |

Next: milestone 2, bounded external messages and scheduled wakeups.
Introduce a tiny scripted streaming example after
milestone 3, then grow it through 4–6; do not wait for a complete widget gallery.
Parent-message routing and focus scopes belong with interactive widgets;
explicitly design them before approval dialogs. Generic grid and additional
widgets may proceed after transcript prerequisites, but remain alpha gates.
Moving inline/recording/approval work into alpha changes the old scope:
record that amendment before treating the expanded release plan as accepted.

Ingress must bound both queued count and payload bytes, coalesce stream updates
without dropping text, and service input/resize between producer batches.
Wrapping caches must invalidate on width/theme changes as appropriate.
First virtualization target: append rewraps the unfinished tail, then paints the
viewport; initial layout and resize may scan history. Profile before adding
arenas, interning, alternate dispatch, or dependencies for optimization.

Recording is explicitly enabled by the app; no automatic collection/upload.
An app supplies its initial fixture and message codec; record external results
and nondeterministic inputs, including seeds when used. Exact replay applies to
the same build/profile and recorded boundary, not arbitrary user code or terminal
pixels. Offer redaction and size limits; a redacted fixture needs its own expected
frames. Never promise an original-session match after changing its inputs.

## Performance and release evidence

The [render benchmark](../benches/render_diff.rs) measures pure diffing,
retained `Renderer::present`, and paint + present. It does not include app update,
terminal I/O, or streaming. Proposed budgets below are engineering targets;
baseline them on a named reference machine before claiming success.

Presentation now emits and synchronizes each damaged row together. Raw-equal
spans skip both work and grapheme cloning; raw changes still synchronize when
their colours downgrade to an identical visible colour. Invalidation, resize,
and capability changes retain full redraw behavior. `Renderer::render` still
paints the full viewport; this is not cached layout or automatic damage tracking.

Local comparison on **2026-10-18**: Intel Core i5-12400, Windows build 26200,
Rust 1.98.1, release profile, 200×60 cells, truecolor test capabilities. The same
harness ran against `c74c345` and this optimization, offline and without terminal
I/O. Each value is the median of five batch means after 10% warmup; batches
contain 1,000 frames, except full restyling (100) and long emoji (400).

| Presentation workload | Before, µs/frame | After, µs/frame | Commands |
| --- | ---: | ---: | ---: |
| Unchanged mixed text, full damage | 118.7 | 64.8 | 0 |
| One-cell edit, full damage | 123.2 | 67.4 | 4 |
| One-cell edit, one-cell damage | 0.2 | 0.2 | 4 |
| Every cell restyled, including back-buffer copy | 677.5 | 538.2 | 181 |
| Unchanged heap-backed family emoji | 318.1 | 57.5 | 0 |
| Unchanged text view, paint + present | 280.2 | 74.8 | 0 |

Run `cargo bench -p jilva --bench render_diff --locked` to reproduce the scenes.
These are local CPU samples, not per-frame p95, allocation counts, interactive
terminal latency, or a comparison with other frameworks. Machine load changes
the absolute times; command equality and Unicode/damage round trips are tested
separately.

| Workload | Proposed gate |
| --- | --- |
| Idle, no animation | Zero output and zero animation wakeups; bounded input waits remain allowed |
| 10k fixed 80-cell messages, 100 text chunks/s, 80x24 and 200x60 | Warm update+paint+diff p95 <= 2 ms / 8 ms respectively; input-to-frame p95 <= 50 ms under load |
| Scrollback cap reached, then another 100k chunks | Retained text/items/caches stay within configured limits; no continuing memory growth; viewport and anchor stay correct |
| Same versioned session replayed twice | Identical typed commands and batch boundaries; saved fixture also checks expected cells |

Run release builds with fixed scenes, chunk sizes, capability tiers, and seeds.
Report compiler, dependency revisions, machine/OS/terminal, warmup/sample count,
p50/p95, allocations or peak memory, commands, and encoded bytes. Separate
paint/diff CPU cost from actual terminal latency. Compare Ratatui and FrankenTUI
only with equivalent scenes, features, and output correctness; publish harnesses
and raw results before any speed claim. No comparative measurement exists yet.

Before alpha, exercise Windows Terminal, a Linux terminal, a macOS terminal,
and tmux/SSH with resize, CJK/emoji, paste, reduced motion, and cleanup failures.
Headless CI covers Linux/macOS/Windows contracts; it is not visual validation.
Keep checks in [CONTRIBUTING.md](../CONTRIBUTING.md) and API examples in rustdoc.

## Design decisions

| Record | Preserved status | Meaning |
| --- | --- | --- |
| RFC-0001 | Accepted, recorded 2026-10-16; revalidated 2026-10-17 | Rust alpha scope includes runtime, flex/grid, themes, essential widgets |
| RFC-0002 | Accepted, same dates | Hybrid model, framework slots, explicit identity, pure descriptions, synchronous core |
| RFC-0003 | Accepted, same dates | Theme roles, palettes, overlays, fallbacks, and switching implemented; configurable transforms remain |
| RFC-0004–0007 | Draft | Rendering, flex, views, motion implemented; implementation is not design acceptance |
| RFC-0008 | Draft | Runtime/components implemented in slices; default cleanup and other amendments still need recorded decisions |
| ADR-0001–0004 | Accepted records retained | Rust first, three crates, Crossterm seam; agent/streaming CLIs with beauty and reproducibility |
| This plan | Proposed, 2026-10-17 | Priorities and gates above; no new feature acceptance implied |
| Theme integration | Implemented, review pending, 2026-10-17 | Preserve Selected alias; Context shares a resolved cache via Clone; fixed default transforms; no new dependencies |

Keep future proposals in PR descriptions: problem, contract, alternatives,
tests, and unresolved items with owners. Record dated maintainer decisions
here; retain independent technical review and cross-contract checks, with no
mandatory external waiting period. Public seam/error changes require two reviewers.
Breaking changes in `0.x` still need an explicit design amendment and migration
notes; patch releases do not break APIs. The pinned toolchain is not a declared
long-term MSRV promise.

The 43 former design/audit files and four experiment READMEs are consolidated
here; original discussions, rationale, and measurements remain in Git at
`43f285ab0df2945cc630a8bec9cc05e33bdc3e20`. For example:
`git show 43f285a:docs/RFC/0002-component-model.md`.
Historical [experiments](../experiments/rfc-0002/) are detached crates, outside
workspace checks: run `cargo test --manifest-path <experiment>/Cargo.toml`.
Their `negative_*.rs` files intentionally fail compilation; prototype allocation
results do not establish production performance. Update this guide in place;
keep implementation invariants in rustdoc and avoid new audit/report files.
