# Jilva project guide

Reviewed **2026-10-17** against `43f285a`. This is the single maintained
design and planning document. [Quick start](../README.md#try-it) ·
[Architecture](#architecture) · [Comparison](#framework-comparison) ·
[Plan](#development-plan) · [Decisions](#design-decisions).

## Current implementation

| Area | Working now | Missing |
| --- | --- | --- |
| [Terminal](../crates/jilva-terminal/src/backend.rs) | Optional Crossterm, typed input/output, cleanup, capability snapshots, fault-injecting test backend | Managed inline viewport and shell scrollback |
| [Renderer](../crates/jilva-core/src/renderer.rs) | Grapheme cells, wide-glyph repair, clipping, diff, explicit damage regions, colour degradation | Runtime-generated damage; `Renderer::render` still paints and compares the whole viewport |
| [Layout and views](../crates/jilva-core/src/view.rs) | Flex rows/columns, constraints, spans, overlays, padding, borders, gradients | Intrinsic measurement, wrapping, scrolling, grid |
| [Runtime](../crates/jilva-core/src/lifecycle/app.rs) | `Model`, `App`, FIFO follow-ups, ignored-event fast path, injected time | External message ingress, timer events, animation wakeups, cancellation |
| [Components](../crates/jilva-core/src/component.rs) | Framework-owned slots, keys, cleanup, Tab focus, bubbling | Parent messages, focus events/scopes, mouse hit-testing; configurations are boxed per description |
| [Style](../crates/jilva-core/src/style.rs) | Four placeholder semantic tokens; resolved RGB/ANSI/mono output | Full token schema, theme resolver, curated themes, state overlays |
| [Motion](../crates/jilva-core/src/motion.rs) | Easing, retargetable transitions, injected time, reduced-motion sampling | Runtime scheduling and theme/component integration |

The counter examples prove the runtime, not a complete widget toolkit.
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
Flex remainders use largest fraction, earlier item wins ties; overflow makes
later items yield first. Tiny regions clip safely; a running app rejects zero size.
`Change::None < Repaint < Layout` is the invalidation contract, but repaint/layout
currently share the full paint path. Damage regions help only when supplied by
a lower-level caller; sparse terminal output alone does not prove cheap updates.

### Theming contract

The accepted design specifies 14 roles: Background, Surface, Overlay, Border,
Text, Muted, Accent, Success, Warning, Danger, Info, Selection, Cursor, Highlight.
It specifies two built-ins (`jilva-dark`, `jilva-light`) with automatic mono
resolution. App deltas override a base, with dark filling missing tokens.
Merge tokens first, apply state overlays, then degrade to terminal capabilities.
Overlay order is Hovered < Active < Focused < Selected; Disabled overrides all.
Focus/selection must remain distinguishable without colour. Missing extension
tokens fall back to Text with a debug marker. Theme switches invalidate paint,
not geometry; merge and quantization caches have separate version keys.
These are design commitments, not implemented APIs; the four-token placeholder
and the accepted vocabulary must be reconciled explicitly in the next PR.

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
All items below are **planned**; completing foundations does not complete them.

| Order | Deliverable and code area | Acceptance gate |
| --- | --- | --- |
| 1 | Theme resolver in `style.rs`, context/view integration, facade exports | Reconcile 4 vs 14 tokens; dark/light snapshots across truecolor/256/16/mono; partial overrides and Selected+Focused/Disabled cases; theme switch leaves geometry/focus intact |
| 2 | External typed messages and scheduled wakeups in runtime/driver | Bounded ingress with explicit full/disconnected results; per-turn fairness; deterministic ordering; cancellation/shutdown; fake-clock animation advances without keyboard input and stops waking when idle |
| 3 | Text measurement/wrapping, viewport, multiline prompt | Grapheme editing, paste limits, history; only visible transcript content painted; append preserves a scrolled-up anchor; resize reflows correctly; configurable byte/item/cache caps survive overflow |
| 4 | Managed inline mode plus plain transcript adapter in terminal/renderer | Interactive footer + committed scrollback; one terminal writer; resize does not duplicate/erase history; normal/error/unwind restoration; redirected mode emits text without cursor controls |
| 5 | Offline `agent_chat` example and reusable streaming components | Scripted replies, Markdown subset, code/tool/diff cards, approval; arbitrary chunk boundaries preserve final text; keyboard-only use, cancellation, no network/API key; same script drives headless tests |
| 6 | Opt-in recording/replay adapter and regression helper | Versioned initial state fixture + capabilities, dimensions, theme, width-policy/build identity, time, ordered input/messages/completions; replay runs no live effects; two replays match frames; report first mismatching event/cell |
| 7 | Alpha completion, measured performance, external use | Text/list/table/input/tabs/status bar and accepted grid scope; tested examples/package; platform/terminal evidence; two independently written downstream apps; migration notes and declared MSRV policy |

Start with milestone 1. Introduce a tiny scripted streaming example after
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

The existing [diff benchmark](../benches/render_diff.rs) measures explicit
buffers/damage, not end-to-end streaming. Proposed budgets below are engineering
targets; baseline them on a named reference machine before claiming success.

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
| RFC-0003 | Accepted, same dates | Semantic theme schema and capability fallbacks; implementation pending |
| RFC-0004–0007 | Draft | Rendering, flex, views, motion implemented; implementation is not design acceptance |
| RFC-0008 | Draft | Runtime/components implemented in slices; default cleanup and other amendments still need recorded decisions |
| ADR-0001–0004 | Accepted records retained | Rust first, three crates, Crossterm seam; agent/streaming CLIs with beauty and reproducibility |
| This plan | Proposed, 2026-10-17 | Priorities and gates above; no new feature acceptance implied |

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
