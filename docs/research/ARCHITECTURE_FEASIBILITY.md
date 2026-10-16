# Architecture Feasibility — Independent Framework Research (Phase 1.5, Round 1)

- Role: Subagent 4 — Independent Framework Research & Feasibility Engineer. Job: CHALLENGE assumptions, not agree.
- Repo: `D:\jilva\jilva-rs`, branch `phase1.5-rfc-sprint`. No production code changed; no commits.
- Read first: `docs/ARCHITECTURE.md`, `docs/API_DESIGN.md`, `docs/adr/*.md`,
  `docs/RFC/0000-template.md`, `crates/*/src/*.rs` (all read 2026-10-16).
- Round status: **Round 1 — independent research only.** At time of writing,
  `docs/RFC/` contains only `0000-template.md` + `README.md`; no RFC-1/2/3 drafts
  have landed, so there is nothing to cross-review yet. Round 2 will review the
  actual drafts when they land.
- License gate applied to every recommendation below: **MIT-only**
  (MIT or MIT-or-Apache-2.0 dual is acceptable since MIT is offered;
  Apache-only, GPL, LGPL recommendations are excluded).

## 0. Primary sources consulted (web, 2026-10-16)

| # | Source | URL |
|---|--------|-----|
| 1 | Ratatui crate docs (v0.30.2): immediate-mode + buffer/diff model, `run`/`init`/`restore`, `Constraint` layout | https://docs.rs/ratatui/latest/ratatui/ |
| 2 | Ratatui repo (ARCHITECTURE.md, workspace split ratatui-core/widgets/backends) | https://github.com/ratatui/ratatui |
| 3 | `ratatui_core::buffer::Cell`: `CompactString` symbol (`&str`, heap spillover), fg/bg/modifier, `CellDiffOption`, `CellWidth` | https://docs.rs/ratatui-core/latest/ratatui_core/buffer/struct.Cell.html |
| 4 | `ratatui_core::buffer::Buffer`: `Vec<Cell>` + `Rect`, `diff`/`diff_iter`, `set_stringn` grapheme/width handling | https://docs.rs/ratatui-core/latest/ratatui_core/buffer/struct.Buffer.html |
| 5 | `ratatui_core::widgets::Widget`: `fn render(self, area: Rect, buf: &mut Buffer)`, by-value commands + `&W`/`WidgetRef` evolution, issue #1287 boxed-widget motivation | https://docs.rs/ratatui-core/latest/ratatui_core/widgets/trait.Widget.html |
| 6 | Ratatui-core deps: `kasuari`, `unicode-segmentation`, `unicode-truncate`, `unicode-width`, `compact_str`, `lru`, `hashbrown` | https://docs.rs/ratatui-core/latest/ratatui_core/ (sidebar deps list) |
| 7 | Bubble Tea (Elm `Init/Update/View`, `tea.Cmd`, cell renderer, Lip Gloss/Bubbles split) | https://github.com/charmbracelet/bubbletea |
| 8 | Textual (retained DOM, CSS, reactive, async-optional, dev console, pilot-test framework) | https://github.com/Textualize/textual |
| 9 | iocraft crate docs (React-like `element!`, `#[component]`, hooks, Taffy flexbox, `TerminalBackend` seam) | https://docs.rs/iocraft/latest/iocraft/ |
| 10 | iocraft repo (MIT-or-Apache, crossterm n.29 optional, `generational-box`, `futures`) | https://github.com/ccbrown/iocraft |
| 11 | `iocraft::Component`: `type Props<'a>`, `new/update/draw/poll_change`, **not dyn-compatible** | https://docs.rs/iocraft/latest/iocraft/trait.Component.html |
| 12 | OpenTUI (Zig core, TS/React/Solid renderers, flexbox, MIT, production use by OpenCode) | https://github.com/sst/opentui (redirected from github.com/anomalyco/opentui) |
| 13 | Cursive (retained `View` trait, `Cursive` root + callbacks, crossterm/termion/pancurses backends, dummy backend) | https://docs.rs/cursive/latest/cursive/ |
| 14 | Taffy (flexbox/grid/block, `TaffyTree` high-level vs `LayoutPartialTree` low-level API, measure-fn for text leaves, `f32` geometry) | https://docs.rs/taffy/latest/taffy/ |
| 15 | kasuari (Cassowary fork maintained by Ratatui org; incremental, strengths, `f64` variables) | https://docs.rs/kasuari/latest/kasuari/ |
| 16 | Crossterm 0.29 (command/queue API, `PushKeyboardEnhancementFlags`/Kitty protocol support list, MIT) | https://docs.rs/crossterm/latest/crossterm/ |
| 17 | Crossterm Kitty-protocol page (supported terminals: kitty, foot, WezTerm, alacritty, notcurses, neovim, kakoune, dte) | https://docs.rs/crossterm/latest/crossterm/event/struct.PushKeyboardEnhancementFlags.html |
| 18 | `unicode-width` 0.2.2 rules (UAX #11 + TR51 emoji/ZWJ/modifier sequences, CJK `width_cjk`, `cjk` feature) | https://docs.rs/unicode-width/latest/unicode_width/ |
| 19 | `unicode-segmentation` (UAX #29 grapheme/word/sentence iterators, `no_std`) | https://docs.rs/unicode-segmentation/latest/unicode_segmentation/ |
| 20 | Cursive `View` traitalah: retained views + `Printer` drawing on screen subsets (corroborates retained-mode borrow pattern) | https://docs.rs/cursive/latest/cursive/trait.View.html (linked from #13) |

> Deliberately NOT used as evidence: benchmark numbers, blog opinions, secondary
> summaries. No performance numbers appear in this report (per brief: no invented
> numbers; upstream publishes none I could cite either).

## 1. Component-model feasibility risks in Rust

Assumption under challenge: *"RFC-2 can pick any of retained / immediate / Elm-style
and the ARCHITECTURE §3.3 boundary absorbs the choice."* The boundary does absorb it,
but each flavour carries a distinct, load-bearing Rust-specific cost. Findings:

### 1.1 Lifetimes — the Elm-style (Bubble Tea-like) model fights the borrow checker

- Bubble Tea's `Update(msg) -> (Model, Cmd)` + `View() string` works in Go because
  models are cheaply copied/moved with GC and views are plain strings
  (source #7). In Rust, `View(&self) -> String`-per-frame forces either
  whole-model clone or lifetime-parameterised view borrows threaded through the
  run loop. Ratatui explicitly rejected storing widgets: widgets are *consumed
  commands per frame* (`fn render(self, …)`), precisely to dodge this
  (source #5: "Prior to 0.26.0, widgets … were consumed during rendering …
  not meant to be stored").
- Challenge: any RFC-2 proposing `View(&self) -> impl Widget`-style Elm APIs must
  show where the returned value's lifetime lives across the event→update→render
  transaction (API_DESIGN §5). If the answer is "clone the model", the RFC must
  budget that clone for large app state. If the answer is arena/bump allocation,
  that is a new dependency + seam decision, not a detail.

### 1.2 Borrowck — retained trees (Cursive/Textual-style) need an ownership story up front

- Cursive's `View` impls receive `&mut Cursive` in callbacks and mutate the tree
  through the root handle (source #13: "callbacks … take an `&mut Cursive`
  argument, allowing them to modify the view tree at will"). Textual does the same
  via a message-passing/DOM-handle layer in Python (source #8). Neither lets
  components hold `&mut` borrows into siblings — ARCHITECTURE §3.4 already bans
  sibling mutation, which is good, but RFC-2 must still answer: **who owns widget
  state, and does `render` take `&self`, `&mut self`, or `self`?**
- Ratatui's three-answer evolution is the cautionary tale: by-value `Widget`
  → `impl Widget for &W` → unstable `WidgetRef` for boxed heterogeneous children
  (source #5). Each step was a breaking or additive API change driven by real apps
  needing stored widgets and `Vec<Box<dyn …>>` collections. RFC-2 should pick the
  end state (reference-capable + dyn-capable seam), not re-live the migration.
- Specific risk: `&mut` render (needed for cursor/scroll memoisation inside
  widgets) + `&self` traversal of the parent = double-borrow of the tree unless
  state is split (Ratatui `StatefulWidget(state, …)` split) or interior mutability
  is blessed. The RFC must choose one and name the cost.

### 1.3 Allocations + type erasure + dynamic dispatch — heterogeneous children are the crux

- Every framework surveyed pays this toll somewhere: Ratatui via `WidgetRef` +
  blanket `impl Widget for &W` (dyn-compatible trait, source #5 notes
  "This trait **is** dyn compatible"); iocraft via `AnyElement` type-erased
  elements + `generational-box` handles (sources #9, #10).
- iocraft's `Component` trait itself is **not dyn-compatible** (`type Props<'a>`
  with GAT-like lifetime, source #11) — so dynamic dispatch happens one level up,
  at the element level, with generated glue (`#[component]` macro + `AnyElement`).
  Lesson for RFC-2: a beautiful trait that isn't `dyn`-compatible forces a macro
  or codegen layer to recover heterogeneity. That is DX risk §4, not just perf.
- Challenge: RFC-2 must state the heterogeneous-children mechanism
  (`dyn WidgetRef`-style object, `AnyElement`-style erasure, or enum dispatch)
  and its allocation behaviour (Box per child? Rc? arena?). "Composition is
  first-class" (API_DESIGN §4) is unimplementable without this paragraph.

### 1.4 Cloning — props-by-value vs props-by-reference is decided by evidence, not taste

- iocraft explicitly documents "Pass props and context by reference to avoid
  unnecessary cloning" (source #9). Ratatui's `Cell`/`Buffer` are `Clone`
  (sources #3, #4) and cloning buffers per frame is the standard double-buffer
  pattern — acceptable because cells are small and the clone is bounded by screen
  area, not tree size.
- Challenge: RFC-2 must distinguish *frame-buffer cloning* (bounded, fine) from
  *tree/state cloning per event* (unbounded, the Elm-tax). Any `#[derive(Clone)]`
  on app state in examples should be flagged in review as normative-looking but
  non-normative.

### 1.5 Custom-widget integration — the seam that decides ecosystem success

- Ratatui's split is the model to beat: `ratatui-core` holds only traits/types so
  third-party widget crates depend on the small stable crate, not the app facade
  (sources #1, #2: "For widget library authors: consider depending on
  `ratatui-core`"). Our ARCHITECTURE §4.2 reserves `jilva-widgets` but keeps all
  of §3.3–§3.8 inside `jilva-core` pre-0.1.0 — meaning external widget authors
  depend on the *churning* core. That is a deliberate Phase-1 trade (ADR-0002),
  but RFC-2 must name the stabilisation order: component contract first, extraction
  later, with a migration note — otherwise the first external widget crate
  freezes an immature seam by accident.
- Cursive offers the alternative extreme: implement `View` (retained, rich
  lifecycle: `layout`, `draw`, `on_event`, `required_size`) — maximal power,
  maximal surface to stabilise (source #13 module list: 15+ view-related modules).
  RFC-2 should justify its trait-method count against this: every required method
  is a method every custom widget must implement forever.

### 1.6 Reconciliation — identity without a story is a bug farm

- ARCHITECTURE §8.8 + current `ComponentId(u64)` (opaque, "allocation policy is
  RFC-2 detail") correctly defer the mechanism. Prior art bounds the options:
  (a) positional (Ratatui: no identity at all — re-render everything each frame,
  diff at the *buffer* level, sources #1, #4); (b) keyed (iocraft `ElementKey`
  "used to minimize the number of times components are destroyed and recreated",
  source #9); (c) explicit handles (Cursive `ScreenId`/view paths).
- Challenge: keyed reconciliation in Rust needs key→instance maps with stable
  addresses across frames — i.e. `SlotMap`/`GenerationalBox`-style storage or
  `Pin`. iocraft's `generational-box` dependency exists for exactly this
  (source #10). If RFC-2 chooses keyed identity, it must also choose (or defer
  with a named placeholder) the storage mechanism; "keys" alone are half a design.
  If RFC-2 chooses Ratatui-style no-identity + buffer-diff, it must say so
  explicitly and accept full-tree re-render per frame as the perf baseline (§3).

## 2. Terminal correctness

### 2.1 WHY `Cell { symbol: char }` must be replaced — prototype evidence

`crates/jilva-core/src/renderer.rs` documents the limitation honestly ("holds one
Unicode scalar value … future design work, not modelled"). An isolated prototype
in `C:\Users\hamro\AppData\Local\Temp\opencode\jilva-proto-cell\` (outside the repo;
`cargo run` 2026-10-16, deps `unicode-segmentation 1` + `unicode-width 0.2`, both
MIT-or-Apache-2.0 → MIT-satisfying) measured scalar vs grapheme vs width counts:

| input | scalars (`char`s) | graphemes | width | fits in one `char`? |
|---|---|---|---|---|
| `x` | 1 | 1 | 1 | yes |
| `é` precomposed (U+E9) | 1 | 1 | 1 | yes |
| `é` decomposed (`e` + U+0301) | 2 | 1 | 1 | **no** |
| family ZWJ `👨‍👩‍👧` | 5 | 1 | 2 | **no** |
| CJK `中` | 1 | 1 | 2 | yes (but width ≠ 1) |
| `😀` | 1 | 1 | 2 | yes (but width ≠ 1) |
| flag `🇺🇸` (2 regional indicators) | 2 | 1 | 2 | **no** |

Plus: ZWJ cluster width (2) ≠ sum of scalar widths (6) — so per-`char` width
accumulation is not merely lossy, it is *wrong in magnitude*. Three independent
failure modes, hence three reasons `char` must go:

1. **Storage:** one `char` = one Unicode scalar value (by language definition).
   Any multi-scalar grapheme (combining marks, ZWJ sequences, flags) is
   unrepresentable. Not a sizing bug — a type-level impossibility.
2. **Width:** `char` carries no width; CJK/emoji are width-2, combining marks
   width-0, and cluster width ≠ Σ scalar widths. Column math built on `char`
   count drifts (considerations §8.3–§8.4).
3. **Segmentation authority:** grapheme boundaries require UAX #29 tables
   (`unicode-segmentation`, source #19); widths require UAX #11 + TR51 emoji rules
   (`unicode-width` 0.2.2 documents ZWJ/modifier/presentation-sequence handling,
   source #18). A `char` cell answers neither question and invites ad-hoc
   reimplementation.

What was tested: scalar/grapheme/width counts on the seven inputs above +
ZWJ cluster-vs-sum check. What was NOT tested: actual terminal rendering,
font/terminal disagreement behaviour, truncation/clipping policy, buffer/diff
integration, performance. The prototype proves the *type* is wrong, nothing more.

### 2.2 Replacement shape (prior-art-backed, for the RFC author — not a decision)

Ratatui's `Cell` stores its symbol as a `CompactString` (`&str`-accepting,
stack-inline with heap spillover: "puts the symbol onto the stack … requires to
be placed on the heap" when too large, source #3) plus fg/bg/modifier and a
`CellDiffOption` (Skip/ForcedWidth) for wide-cell and image-protocol holes.
That is the existence proof that `String`-family storage + explicit wide-cell
continuation policy works. MIT-gate note: `compact_str` is MIT (per its crates.io
listing; verify version pin at implementation time), `unicode-width` and
`unicode-segmentation` are MIT-or-Apache-2.0 — all admissible.

### 2.3 In which phase/RFC must the replacement happen?

- **No later than the Rendering Engine work (ROADMAP Phase 3) and no earlier than
  RFC-2's component contract needs it.** Concretely: the RFC that first defines
  buffer/diff semantics (whether that is RFC-2 extended or the Phase-3
  implementation RFC) must replace `Cell.symbol: char` with a grapheme-capable
  symbol type, because `Buffer::diff` correctness (empty-vs-styled-blank,
  wide-cell continuation skip, §8.1–§8.2) is undefined while a cell cannot name
  what it holds. Keeping `char` through Phase 3 would let tests and examples
  entrench `Cell::new('x')`-shaped APIs that the replacement then breaks.
- Minimum viable sequencing: RFC-2 (or a narrow RFC-2 amendment) specifies the
  new cell symbol type + width-table authority (`unicode-width`, with or without
  the `cjk` feature — source #18 notes `width` vs `width_cjk` differ) +
  segmentation authority (`unicode-segmentation`, source #19); Phase 3 implements
  diff against it. The current `is_blank`/`Cell::blank` ground-state semantics
  must be re-specified at the same time (Ratatui treats `None`-symbol ≡ `" "`,
  source #3 — a subtle equality rule our RFC should adopt or explicitly reject).
- **Challenge to any RFC that defers this past Phase 3:** deferral means the
  golden-output tests of Phase 3 (ROADMAP exit criterion) are written against a
  known-wrong cell model and must be rewritten. Name that rework cost or move the
  replacement up.

### 2.4 Remaining correctness checklist (each needs an RFC owner)

- **Clipping/truncation:** never split a grapheme or a wide cell; `set_stringn`
  in Ratatui "skips zero-width graphemes and control characters" (source #4) —
  our renderer RFC needs the equivalent sentence plus wide-cell edge behaviour
  (truncate vs. pad-with-space at region edge when only 1 column remains).
- **Cursor:** cursor position is a *width-computed* value, not a string index;
  Ratatui warns that backend-level cursor moves "can be overwritten by the next
  render pass" (source #1) — our lifecycle RFC must designate one cursor owner.
- **Capability variance:** Kitty keyboard protocol is supported by only
  kitty/foot/WezTerm/alacritty + a few apps (source #17) — the event RFC's
  "encoding ladder" (§8.6) must degrade on all other terminals, and detection
  (push/pop enhancement flags) must be paired with teardown on *every* exit path
  (§8.7), including the pop on panic-unwind.
- **Resize:** Ratatui's policy is instructive — no auto-redraw on resize; the app
  re-`draw`s and Ratatui re-reads backend size at render time, tolerating
  coalesced/missed events (source #1). Our lifecycle RFC should adopt or rebut
  this; "drop vs. complete in-flight frames" (§8.5) must not be left implicit.
- **Emoji width disagreement:** `unicode-width` documents its rule stack
  (source #18) but terminals disagree in practice (especially flags/ZWJ); the
  renderer RFC needs the §8.4 fallback sentence (e.g. ForcedWidth-style override
  per cell, cf. Ratatui `CellDiffOption::ForcedWidth`, source #3).

## 3. Perf risks

No numbers are cited anywhere below — none exist yet (no benches: ARCHITECTURE §7
reserves `benches/` with "nothing to measure yet"), and upstream frameworks do
not publish citable frame-budget figures either. Risks are structural:

1. **Full-screen diff is O(cells) per frame, always.** Ratatui diffs prev vs next
   buffers cell-by-cell (`diff_iter`, source #4) and pushes `PushKeyboard…`-style
   minimal updates only after paying the comparison. At 80×24 this is trivial; at
   large scrollback/wide screens the comparison itself is the floor. Our
   performance posture (§8.11) must be stated as *scaling behaviour* (linear in
   cells, no worse), not as a frame-time number, until benches exist.
2. **Invalidation vs. diff layering.** ARCHITECTURE §8.9–§8.10 require both
   layout-invalidation rules AND damage tracking composed with diffing. Prior art
   splits: Ratatui has diff but (in current design) re-renders all widgets per
   frame and relies on buffer-diff to save I/O (source #1: "your app must render
   all widgets … for each frame"); Textual has DOM-level dirty checking + CSS
   (source #8). Doing *both* full re-render and fine invalidation risks paying
   twice; doing neither risks full churn. The RFC must state which layer carries
   the optimisation and prove the other layer is O(1)-ish when idle.
3. **Allocation hot path.** `Buffer { content: Vec<Cell> }` double-buffering
   (source #4) means per-frame allocation unless buffers are reused via
   `reset`/`resize` (both exist in Ratatui's API — evidence the problem is real).
   `String`-family cell symbols (cf. §2.2) add per-cell allocation pressure;
   Ratatui answers with `CompactString` stack-inline + `lru` layout cache
   (source #6 dep list). Our RFC must name the reuse story (buffer pooling?
   `Cell::reset` semantics?) before benches, not after.
4. **Large trees / deep nesting.** Flexbox/grid solvers are per-node work;
   Cassowary is incremental (source #15: "add and remove constraints at runtime
   … minimum work to update"), Taffy caches via `CacheTree`/`compute_cached_layout`
   (source #14). Whichever solver backs §3.6, the RFC must state the caching
   invalidation rule or accept re-solve per frame. Constraint vocabulary size
   (§4 RFC-1 scope!) directly bounds this cost — another reason scope discipline
   is a perf decision.
5. **Headless benchmarking precondition.** Golden-output tests (frame in →
   instruction stream out, §8.13) are also the bench harness inputs: deterministic
   frames + fixed virtual sizes. The lifecycle/test-double RFC should require that
   every golden test be bench-reusable (criterion-style harness choice is
   implementer's, per §7 — Ratatui itself uses `criterion` as dev-dep, source #1).
   No invented targets: the RFC should define *regression-detection* (bench
   asserts no-worse-than-baseline scaling), not absolute budgets, until real
   terminal measurements exist.

## 4. DX (code volume + macro caution)

- **Code volume for basic apps is the adoption gate.** Ratatui hello-world is
  ~10 lines with `ratatui::run` + `terminal.draw` (source #1); Bubble Tea's
  shopping-list tutorial is ~60 lines for Init/Update/View + `main`
  (source #7); iocraft's hello-world is ~10 lines of `element!` (source #9);
  Cursive's is ~8 lines (`Cursive::new` + `add_layer` + callback + `run`,
  source #13); Textual's clock is ~25 lines with CSS (source #8). **Challenge:**
  any RFC-1 scope or RFC-2 API sketch must include a hello-world and a
  counter/list example at this level of brevity, or it has not met
  API_DESIGN principle 1 ("anything requiring manual cell math is an API bug")
  and principle 3a (build-and-run helpers). Review Round 2 will count lines.
- **Macro caution (explicit).** The `element!`/`#[component]` pattern (iocraft,
  sources #9–#11) buys ergonomics but: (a) non-`dyn`-compatible traits *require*
  macro-generated glue, hiding the real contract from rustdoc and error messages
  (violates API_DESIGN §2 "internal topology never visible in error messages" if
  macro expansions leak); (b) Ratatui ships `ratatui-macros` as *optional*
  (`macros` feature, source #1) — macros as opt-in sugar, never the only path;
  (c) `missing_docs = deny` (workspace lints) interacts badly with macros that
  generate public items. **Recommendation:** RFC-2 must present the macro-free
  expansion of every macro-based API as the normative definition, with macros
  strictly additive. Any RFC proposing a macro as the *primary* component API
  should be challenged to show rustdoc + compiler-error output for a misuse case.
- **Error-message DX** is part of the API (API_DESIGN §5: "descriptive errors,
  never silent misrendering"). Cassowary-style solvers fail with
  `AddConstraintError` enums (source #15) — over-constrained layouts (§8.9,
  API_DESIGN §8) must map solver errors to the facade error taxonomy (§3.9),
  never leak `kasuari`/`taffy` types through `jilva::Error`.

## 5. Per-RFC findings (for Round-2 cross-review)

> RFC drafts have NOT landed (only the template). These are *advance* finding
> lists — acceptance criteria the drafts will be judged against in Round 2.

### RFC-1 (scope and non-goals) — findings to verify in the draft

- [F1.1] Does the scope bound the constraint vocabulary (flex row/column +
  grid subset?) so solver cost (§3.4) and API surface stay proportional? An
  unbounded "Taffy, all of it" scope without a cell-quantisation story repeats
  ADR-0003's rejected alternative #4.
- [F1.2] Is `Cell.symbol: char` replacement explicitly in scope for Phase 3 at
  the latest (§2.3), with width/segmentation authorities named? Silence = challenge.
- [F1.3] Does the non-goals list include: async runtime (per §2.4 principle),
  pixel/CSS-unit leakage into public API (ADR-0003), macro-primary APIs (§4)?
- [F1.4] Is there a hello-world line-count comparison against Ratatui/Cursive
  (§4) with concrete code, not adjectives (per ARCHITECTURE §9 "concrete and
  falsifiable")?
- [F1.5] Are all 14 §8 considerations mapped to RFC-1/2/3 or explicitly deferred
  with a tracking issue (conformance checklist §11)? Unmapped = challenge.

### RFC-2 (component model) — findings to verify in the draft

- [F2.1] Render-signature choice (`self` / `&self` / `&mut self` / split
  stateful) justified against Ratatui's migration history (source #5) — must
  land on the end state, not the start.
- [F2.2] Heterogeneous-children mechanism named (dyn-compatible trait object?
  type-erased element? enum?) with allocation behaviour (§1.3). Missing = challenge.
- [F2.3] Identity mechanism (positional/keyed/explicit) + storage story for keys
  (§1.6); misuse/collision behaviour documented (API_DESIGN §4 requirement).
- [F2.4] State-ownership rule (ephemeral-local vs app-owned) + event-bubbling
  order, with borrowck-feasible signatures (§1.2). `&mut Cursive`-style root
  handles need Send/Sync analysis for the `unsafe_code = forbid` lint.
- [F2.5] Macro-free normative definition + additive macros only (§4); custom-widget
  required-method count justified vs Cursive's `View` (§1.5); third-party widget
  seam stabilisation order named (depend on churning core — with eyes open).
- [F2.6] Elm-style proposals must budget model cloning or name the arena
  (§1.1); reconciliation-vs-buffer-diff layering stated (§3.2).

### RFC-3 (theming) — findings to verify in the draft

- [F3.1] Token vocabulary + truecolor→256→16 fallback chain with snapshot tests
  per tier (ARCHITECTURE §3.8 rule) — Textual's theme system (source #8:
  "predefined themes … look good out of the box") and Ratatui `Style`/`Stylize`
  (source #1) are the comparison points; challenge any raw-ANSI leakage
  (API_DESIGN §7 bans it).
- [F3.2] Capability *profile as plain data* flowing into style resolution
  (ADR-0003 decision #2) — no backend types in theme APIs; detection cached at
  startup, never sniffed mid-frame (§8.14).
- [F3.3] `serde` for theme files: Ratatui gates style serde behind a feature
  (source #6); iocraft likewise (`serde` optional, source #10). RFC-3 should
  follow: serialisation additive, not default.
- [F3.4] `palette`-crate interop is optional upstream (Ratatui `palette`
  feature, source #1) — adopt the same posture (additive), and note it is
  MIT-or-Apache-2.0 → gate-passing but still needs an RFC/PR justification per
  ARCHITECTURE §6.6.
- [F3.5] selective-override-with-default-gap-fill (API_DESIGN §7) needs a merge
  semantics paragraph (which side wins per token?); "no all-or-nothing theme
  authoring" must be testable, not aspirational.

## 6. Unresolved questions (carried into Round 2)

1. Which solver backs §3.6, if any pre-0.1.0 — hand-rolled `Constraint`
   combinators (Ratatui `Layout` + kasuari cell-splitting) or Taffy-behind-seam?
   Taffy outputs `f32` (source #14) → integer snap/remainder policy is ours to
   specify either way. Prototype candidate (Round 2, if RFC is ambiguous):
   cell-quantised Taffy measure-fn round-trip in scratch dir.
2. `unicode-width` `cjk` feature on or off by default (source #18: `width` vs
   `width_cjk` differ on Ambiguous)? Affects CJK users directly; needs a decision
   with terminal-disagreement fallback (§2.4).
3. `compact_str` (or equivalent SSO string) for cell symbols — admissible
   (MIT) but needs the §6.6 justification in the implementing PR, plus
   stack-spillover failure-mode docs (source #3 pattern).
4. Keyboard ladder depth: Kitty-protocol push/pop (source #17) vs legacy-only
   for 0.1.0? Determines event-taxonomy size and teardown complexity.
5. Does `jilva-terminal`'s test double need an event-timing model (tick sources,
   coalesced resizes) or is scripted-sequence sufficient for Phase 2 exit?
6. Cross-language portability pressure (ADR-0001 says none — but theme-token and
   event-taxonomy shapes chosen now become the "portable artefacts" later;
   worth one explicit non-goal sentence in RFC-1/RFC-3).

## 7. Prototype log (labeled per brief)

- **P1 — `Cell`-as-`char` inadequacy probe — DONE, evidence above (§2.1).**
  Location: `C:\Users\hamro\AppData\Local\Temp\opencode\jilva-proto-cell\`
  (scratch dir, outside repo; repo untouched — `git status` shows only
  pre-existing `docs/ROADMAP.md` modification + untracked `docs/reviews/` from
  sibling agents, neither mine).
  Tested: scalar/grapheme/width counts over 7 inputs; cluster-vs-sum width check.
  NOT tested: rendering, terminal agreement, clipping, diff integration, perf.
- **P2 (conditional, Round 2)** — Taffy cell-quantisation round-trip, only if the
  layout RFC leaves float→cell snapping ambiguous. Not run: no uncertainty to
  resolve yet (no drafts landed).
- **P3 (conditional, Round 2)** — `dyn WidgetRef`-style heterogeneous child list
  borrow/allocation sketch, only if RFC-2 omits §1.3 mechanism. Not run.

## 8. Handoff

### Top 5 risks (ranked by irreversibility × likelihood)

1. **Cell-as-`char` entrenches before Phase 3.** Every test/example written
   against `Cell::new('x')` is future rework. Mitigation: RFC-2 amendment
   specifies the grapheme-capable symbol type + width/segmentation authorities
   now (§2.3); prototype P1 already proves necessity.
2. **RFC-2 picks a component flavour without the heterogeneity + borrow story.**
   A pretty trait that isn't `dyn`-compatible or can't hold mixed children
   forces a macro/codegen rescue (iocraft precedent, §1.3) — the single most
   expensive late-stage rework in this design space.
3. **Keyed identity without storage design.** Keys without `SlotMap`/generational
   storage (iocraft precedent, §1.6) is half a reconciliation design; collisions
   and use-after-reorder become user-visible bugs.
4. **Kitty-protocol ambition without the ladder.** Progressive key encoding is
   supported by a minority of terminals (source #17); promising it for 0.1.0
   without documented degradation + teardown pairing turns §8.6–§8.7 into
   support load.
5. **Macro-primary API.** Ergonomics now, opaque errors + rustdoc gaps forever
   (§4). Ratatui's optional-macros posture is the proven alternative.

### Differentiation opportunities (honest, falsifiable — per §9 no-superiority rule)

- **Rust-native Elm Hutch, done once:** Ratatui owns immediate-mode, Cursive
  owns retained-callbacks, iocraft owns React-hooks. A borrowck-clean Elm-style
  contract (if RFC-2 can actually specify §1.1–§1.4 without cloning the world)
  is an unoccupied, citable position — but only if the RFC shows signatures.
- **Correctness-first cell model from day one:** shipping grapheme/width-correct
  cells + per-tier theme snapshots at 0.1.0 (while competitors retrofit: cf.
  Ratatui's `CellDiffOption::Skip` deprecation churn, source #3) is a durable,
  testable differentiator — "concrete and falsifiable" by construction.
- **Backend seam + headless-first testing as the headline:** every boundary
  testable with fake terminal/scripted events/fixed clock (§2 principles 3, 5)
  plus bench-reusable golden tests (§3.5) is a stronger test story than any
  surveyed framework documents end-to-end. The devtools observer seam (§4.2
  `jilva-devtools`) extends this into Textual-dev-console territory later.

### Per-RFC findings preview (for the main session)

- RFC-1 review will check: scope-bounded solver vocabulary, Cell-replacement
  scheduling, non-goals (async/CSS-units/macro-primary), hello-world brevity
  with line counts, full §8-consideration mapping (F1.1–F1.5).
- RFC-2 review will check: render-signature end state, heterogeneity mechanism +
  allocation, identity + storage, state/bubbling borrow feasibility, macro-free
  normative core, Elm-clone budget (F2.1–F2.6).
- RFC-3 review will check: token vocabulary + tier snapshots, profile-as-data
  flow, additive serde/palette posture, override-merge semantics (F3.1–F3.5).
- All findings are grounded in the 20 primary sources tabled in §0; no
  performance numbers asserted; one labeled prototype (P1) with tested/not-tested
  bounds; two conditional prototypes (P2, P3) queued for Round 2 if drafts are
  ambiguous.

---

# Round-2 draft review (2026-10-16, appended without rewriting Round-1)

- Scope: read all three drafts cold in Round 2
  (`docs/RFC/0001-framework-scope.md`,
  `docs/RFC/0002-component-model.md`,
  `docs/RFC/0003-theming-architecture.md`).
  They landed mid-Round-1 as sibling agents' untracked files; Round-1 above was
  written without reading them.
- Method: (a) verify every prior-art behavior claim against Round-1 sources —
  confirm or correct; (b) pressure-test RFC-2 against predicted holes F2.1–F2.6,
  RFC-1 against F1.1–F1.5, RFC-3 against F3.1–F3.5; (c) check cross-RFC
  consistency on change classes, layout-vs-style authority, capability-profile
  shape, Cell replacement scheduling.
- Rank scheme: **BLOCKER** = acceptance must not happen until resolved
  (contradiction or hole in a load-bearing contract; Discussion may start, but
  these are the acceptance gates). **HIGH** = must be resolved/owned during
  Discussion (named owner + committed amendment/answer), else becomes BLOCKER at
  acceptance. **MEDIUM** = should be answered in Discussion; deferrable only with
  a tracked question that entrains no API. **LOW** = editorial/citation hygiene;
  fix anytime. All refs are `file §section` (`0001` = RFC-0001, etc.).

## A. Prior-art claim verification (confirm or correct)

| # | Draft claim | Verdict |
|---|---|---|
| A1 | 0001: Ratatui immediate-mode `terminal.draw`, buffer+diff, Backend trait + 4 backend crates, core/widgets split, Constraint/Layout, rich widgets | **CONFIRMED** (R1 sources #1, #2). |
| A2 | 0001 Q1 + 0002: Ratatui "leaves the run loop, state ownership, and restoration to user code" | **PARTLY STALE** → R1-B1. `ratatui::run`/`init`/`restore` (0.28.1/0.30.0) handle init + restoration incl. panic hooks (R1 source #1). True part: event loop + state ownership are user code. |
| A3 | 0001: Bubble Tea Elm triple, `Update→(Model,Cmd)`, whole-UI View | **CONFIRMED** (R1 source #7 tutorial). Nuance: v2 signature returns `tea.View` (`NewView(s)`), not `string` — one-word fix. License hedge ("MIT — interpretation") → **upgrade to CONFIRMED**: repo carries MIT license (R1 source #7 resource list). |
| A4 | 0001: Textual compose-yields, TCSS/CSS, async-first, textual-dev | **CONFIRMED core** (R1 source #8: `compose()` yields, CSS string, "asynchronous framework under the hood", `textual-dev`). Specifics (`CSS_PATH`, `@work`, live CSS reload, LWN 2023, Real Python) **UNVERIFIED** in R1 sources → R1-B3. License hedge → **upgrade to CONFIRMED MIT** (R1 source #8 resource list). |
| A5 | 0001: iocraft `element!`/`#[component]`/hooks/`TerminalBackend`/`View,Text,TextInput`/taffy re-export/Dioxus+Ink | **ALL CONFIRMED** (R1 sources #9–#11, incl. counter example, `pub use taffy`, `Component` not-dyn-compatible). License hedge → **upgrade to CONFIRMED** MIT-or-Apache-2.0 dual (LICENSE-APACHE + LICENSE-MIT, "Licensed under either of", R1 source #10). |
| A6 | 0001: Cursive retained views, root object, backend switchability | **CONFIRMED** (R1 source #13: views module, `Cursive::new` example, backends module, crossterm/termion/pancurses features). |
| A7 | 0001: Ink internals; OpenTUI "Yoga Flexbox", snapshot testing, "Bun FFI" | **UNVERIFIED** in R1 sources (Ink not researched R1; source #12 shows OpenTUI "flexbox" not Yoga, Bun+Zig toolchain not FFI mechanism, no snapshot-testing evidence) → R1-B3. Repo-path hygiene: `sst/opentui` fetch resolved to `anomalyco/opentui` content — confirm canonical URL before citing. |
| A8 | 0002: Ratatui `Widget`/`StatefulWidget`, `&mut State`, buffer+diff; Cursive `cb_sink`; signals need observer context | **CONFIRMED** as far as R1 sources go (Widget render signature #5; `CbSink` alias + `cursive::dummy` in #13 fn list; signals reasoning is sound architecture). "Discussion #552 explicitly missing by design" + Xilem async-depth specifics **UNVERIFIED** → R2-C11. |
| A9 | 0002: Cursive callbacks "complicate headless testing" | **OVERSTATES** → R2-C10. `cursive::dummy` backend exists (R1 source #13). Accurate claim: callback order is emergent, weakening replay — rejection still stands on unidirectionality. |
| A10 | 0003: Ratatui Style fg/bg/modifier triple + Rgb/Indexed/Named ladder | **CONFIRMED** (R1 sources #1, #3 incl. `underline_color`, named+indexed+RGB). |
| A11 | 0003: env-var ladder (`COLORTERM`/`TERM`/`NO_COLOR`/`COLORFGBG`), 6×6×6 cube + grayscale, SSH-forwarding loss | **CONFIRMED** standard terminal truth (256 = 16 + 216 + 24). Solid. |
| A12 | 0003: Cursive theme TOML, Textual design-tokens/`watch`, `ratatui-themes` crate | **UNVERIFIED** specifics in R1 sources (Cursive `theme` + `builder` modules confirmed; TOML/design-token/`watch`/crate-existence details not) → R3-D10. |

## B. RFC-1 findings (scope)

- **R1-B1 [MEDIUM]** — 0001 §Prior art (Ratatui bullet) + Q1 contrast: "leaving
  the run loop, state ownership, and restoration to user code". Restoration is
  NOT left to user code anymore: `ratatui::run()` "initializes the terminal,
  runs your app, and restores the terminal on exit", with `init`/`restore` +
  panic-hook handling (R1 source #1). Fix: contrast on event-loop + state
  ownership (true, load-bearing) and drop "restoration" from the list, or scope
  it to "restoration *policy per exit path* is user-orchestrated". Decision
  unaffected; differentiation wording is.
- **R1-B2 [LOW]** — 0001 §Prior art license hedges ("MIT — interpretation, widely
  reported; verify at implementation time" ×2; "MIT/Apache-style dual —
  interpretation"). Per A3–A5, upgrade Bubble Tea→MIT confirmed, Textual→MIT
  confirmed, iocraft→MIT-or-Apache-2.0 confirmed. Keep "verify at
  implementation time" only for Ink/OpenTUI.
- **R1-B3 [LOW]** — 0001 §Prior art specifics per A7/A4-unverified: soften to
  interpretation or add links (Ink reconciler/Yoga/`<Static>`; OpenTUI Yoga claim
  → "flexbox" until sourced; OpenTUI snapshot testing; Textual CSS_PATH/@work/
  live-reload/LWN/Real-Python). Also confirm canonical OpenTUI repo URL.
- **R1-B4 [LOW]** — 0001 §Prior art Cursive bullet: "framework Erzählen" —
  stray word (typo/editing slip).
- **R1-B5 [LOW]** — 0001 Q9: "No audio output, ever in current scope" / "No 3D
  rendering, ever in current scope". "Ever" contradicts Q9's own revisit rule
  (superseding ADR + RFC). Delete "ever"; "Out (revisit needs ADR+RFC)" is
  already the mechanism.
- **R1-B6 [LOW]** — 0001 §2 matrix, Layout Must ("grid tracks + placement")
  vs Should ("spanning refinements"): confirm placement-without-spanning is a
  coherent alpha subset (presumed yes: track assignment vs multi-track span).
  Question for Discussion, not a gap.
- **F1.1 verdict: SATISFIED at scope level.** Layout vocabulary is bounded
  (flex row/column + grid subset, remainder rule promised, CSS-units Out,
  Taffy-behind-seam Could with ADR-0003 conditions). No solver-bound leak in
  scope text. Implementation risk moves to the layout RFC (Open Q1 tie-break).
- **F1.2 verdict: PARTIAL → owned by X3.** Segmentation-authority Must +
  char-math Out + width-fallback Should are present, but the `Cell.symbol: char`
  type replacement itself is scheduled nowhere (see X3).
- **F1.3/F1.4/F1.5 verdicts: SATISFIED.** Non-goals name async/CSS-units/
  macro-primary correctly; hello/loop sketch present (line-count comparison vs
  prior art still owed in Discussion per F1.4 — sketches exist, counts not
  tabulated); §8 mapping enforced via Open Q7-style flags.

## C. RFC-2 findings (component model)

- **R2-C1 [BLOCKER]** — Heterogeneity mechanism missing (predicted F2.2).
  §"API sketches" composes `frame.column([Header::new(&model.title),
  TodoList::new(&model.todos).on_select(…), StatusBar::new(model.status())])` —
  a heterogeneous array literal with no common element type. Without an erased
  element type, `Signal`-free `IntoIterator` bound, or enum dispatch, this
  sketch cannot compile even in principle. Normative text ("tree nesting +
  typed child messages + keys") names no mechanism. **P3 prototype evidence**
  (NEW, `C:\Users\hamro\AppData\Local\Temp\opencode\jilva-proto-hetero\`,
  zero-dep, outside repo): an associated-`State`/`Message`, no-`self`-receiver
  trait as `Box<dyn …>` fails with `E0038` ("not dyn compatible … `init`/`update`
  ha[ve] no `self` parameter"); rustc itself suggests enum dispatch and warns
  third-party extensibility is lost with enums. The dyn-compatible render-only
  alternative (`WidgetRef`-style) compiles and holds heterogeneous children —
  but carries no state/message channel, which must then live in a second
  (erased/enum) channel. Tested: E0038 failure mode + dyn-render success. NOT
  tested: enum-dispatch ergonomics, `AnyElement`-style erasure cost, allocation
  rates. RFC-2 must pick: (i) enum dispatch (closed world — contradicts
  third-party widgets), (ii) `AnyElement`-style erasure + generated glue
  (re-opens the macro question §7 claims closed), or (iii) split traits
  (dyn-render + separate state channel, paid in complexity). Any is acceptable;
  silence is not.
- **R2-C2 [BLOCKER]** — Two frame architectures in one RFC (predicted §1.2
  borrow hole). Normative §§1–3: frame descriptions are *owned values*
  ("regions + styled spans as data", "no lifetimes escape", "`view: &S`
  shared borrows compose freely"). Sketches: `fn view(app: &App, frame: &mut
  Frame)` with `frame.text/column/list` — a *mutable draw target* threaded
  through nested calls, Ratatui-style. Sibling composition through one
  `&mut Frame` requires region-scoped sub-borrows (layout computed inline
  during view — who owns that? unnamed) or interior mutability (contradicts
  "no interior mutation of framework state" §1 rule 2). The borrow story the
  draft sells ("never fight the borrow checker") holds for the owned model and
  is unproven for the `&mut Frame` model. Pick one as normative; rewrite the
  sketches to match before acceptance.
- **R2-C3 [MEDIUM]** — Return-taxonomy blur. §4 Outcomes
  (Consumed/Ignored/Observed/RequestFocus) × §5 change classes
  (State-only/Layout/Structure) × sketches (`-> Change`, `Change::Paint` in the
  counter; `-> Outcome` in input). Does `update` return consumption,
  invalidation, or a pair? Require one normative signature shape (even
  abstract, e.g. `UpdateResult { consumption, change }`) — Discussion will
  otherwise burn a week on vocabulary.
- **R2-C4 [MEDIUM]** — §7 "`Model: Send` is required" overreaches for a
  single-threaded, zero-async-dep loop: every app model pays the bridge's tax
  (`Rc`-cached/!Send state becomes unusable). Open Q3 already suspects this.
  Recommend: default `Send`-free; require `Send` only at the effect-bridge
  boundary (completes crossing threads), documented as the bridge's bound.
- **R2-C5 [MEDIUM]** — §2 specifies key semantics (first-wins, fresh slots,
  debug asserts) but no *storage*: map type? boxing? arena/generational
  handles? who allocates? `ComponentId`'s punted "allocation policy" is still
  punted. P3's compiler hint is the implementer's wall verbatim. Name the
  storage family (even as "HashMap<Key, Box<dyn Slot>> placeholder, benchmark
  before 0.1.0") or the keyed-identity contract is unimplementable.
- **R2-C6 [MEDIUM]** — `Outcome::Observed` vs §4 "frame stream is bit-identical"
  determinism: non-consuming observers with record-like duties (analytics)
  need ordering + sink rules, else replay is only frame-identical by luck of
  sink purity. Draft invites scrutiny (§4); agree — cut `Observed` unless a
  surviving use case is tabled, or specify order-after-consumers + sink purity.
- **R2-C7 [MEDIUM]** — §4 Resize: "drop-vs-complete policy is an implementation
  detail with a test". ARCHITECTURE §8.5 requires *designs* to state
  drop-vs-complete. Punting violates the consideration. Adopt-or-rebut
  Ratatui's precedent (backend size re-read at render; coalesced/missed events
  tolerated — R1 source #1), then the test asserts the stated policy.
- **R2-C8 [MEDIUM]** — Tick subscriptions ("declared state … keeps idle apps at
  zero CPU") lack declaration + storage lifecycle: `view` is pure-`&` (cannot
  register), `update` is per-event (registration side effect where stored?).
  Who holds the subscription set — slot metadata? Unspecified ⇒ "zero CPU"
  unimplementable as written. Specify subscribe/unsubscribe points
  (mount/unmount + declared-intent channel).
- **R2-C9 [MEDIUM]** — `Effect::Task(fn_once_bridge)` vs §7 "data-only
  `Effect`": a `fn` pointer executed *by core* implies an execution policy
  (threads? poll point?) — executor-by-stealth against the runtime-agnostic
  constraint. Show the type is opaque-data + externally executed, or restrict
  0.1.0 effects to `After/cancel` timer-data. Open Q4 covers the cap, not the
  execution question.
- **R2-C10 [LOW]** — §Prior art Cursive: fix per A9 (dummy backend exists;
  accurate charge is emergent callback order, not headless impossibility).
- **R2-C11 [LOW]** — §Prior art receipts per A8: link/quote Ratatui #552
  ("missing by design"); verify-or-soften Xilem async-depth claim.
- **R2-C12 [LOW]** — §6 "the loop abons the batch" (typo → abandons/aborts);
  normative `init … fallible via Result` vs sketch `fn init() -> Self::State`
  (infallible) drift — align sketches after C2 pick.
- **F2 verdicts:** F2.1 (lifetimes) ADDRESSED directionally (`&mut` slots kill
  whole-model clone) but entangled in C2. F2.3 (custom-widget seam) PARTY —
  required-method count still unbounded (trait shape sketched, not fixed) and
  third-party stabilisation order unaddressed. F2.4 (clone tax) ADDRESSED
  explicitly — good. F2.5 (macro-free core) ADDRESSED (proc-macro rejection +
  rationale) — good; weakened only if C1 resolves toward generated erasure.
  F2.6 (reconciliation storage) OPEN → C5 + X2.

## D. RFC-3 findings (theming)

- **R3-D1 [BLOCKER]** — Internal contradiction: §3 single-overlay precedence
  (`Disabled > Selected > Focused > …`, "exactly one overlay applies") × §9
  "focus is indicated by reverse-video or underline, never hue shift alone". A
  selected+focused row resolves `Selected` only — focus indication vanishes in
  the most common focused case (keyboard-driven list). Either focus composes
  (stack at least Focused over selection, with ordered application rules), or
  §9's guarantee is re-scoped (and the mono test §11.2 rewritten). Small fix
  surface, but acceptance must not precede it: it falsifies the draft's own
  test.
- **R3-D2 [HIGH]** — `CapabilityProfile { color, attrs, background,
  reduced_motion }` (0003 §5) has no keyboard/mouse/viewport fields, yet
  0001 Q12 promises a key-encoding ladder + gated mouse + alt-screen fallback
  and 0002 gates mouse on capability. Input/screen capability is homeless
  across all three drafts. Resolution: either this struct IS the shared profile
  (add `keys`/`mouse`/`viewport` fields or explicit `#[non_exhaustive]`
  reservations) or it is the style projection (then name the superset owner —
  likely the backend/lifecycle RFC — and state the projection rule). The shape
  is on 0001's J-list ("measure twice"), so this cannot drift.
- **R3-D3 [HIGH]** — 0003 §4: "No `serde` dependency (zero-dep constraint,
  MIT-only; even MIT-licensed `serde` adds compile-time and MSRV-surface
  cost…)". No absolute zero-*external*-dep constraint exists: ARCHITECTURE §6.6
  and 0001 §3 say minimal-and-justified, and 0001 §3's "zero-dependency
  `jilva-core`" is qualified "**within the workspace**" (workspace-internal).
  Compressed as written, §4 reads as "core takes no external dependencies" —
  which would forbid `unicode-width`/`unicode-segmentation`/`compact_str`-class
  deps the Cell replacement needs (all MIT-gate-passing, R1 §2.2). Charitable
  read (serde passes the license gate; exclusion rests on compile-time/MSRV)
  is probably intended — but precedent-setting text must not need charity.
  Clarify in Discussion; links X3 (dependency path for the replacement).
- **R3-D4 [MEDIUM]** — Unknown-background default (§5: keep app-selected base,
  dual-background-legible accents) × automated contrast checks (§11.3: ≥4.5:1
  body, ≥3:1 chrome at truecolor): against WHICH background(s) do the checks
  run? Mid-contrast accents legible on both black and white live in a narrow
  luminance band; body `Text` under `Unknown` on the wrong background fails
  badly by construction. Specify the check matrix (theme × tier × background
  incl. Unknown) or explicitly downgrade the Unknown guarantee. (Drawbacks
  admits the beauty constraint — extend the honesty to the check.)
- **R3-D5 [MEDIUM]** — §5.2 normatively mandates "minimum perceptual distance"
  while Open Q5 asks Euclidean-vs-perceptual. Move the metric into the question;
  normative text should require "nearest-palette by a documented metric (Q5)".
- **R3-D6 [MEDIUM]** — Shared-B answer (§6: version bump + paint-only damage)
  is clean, but the switch travels through NO named 0002 loop path: theme
  switch is absent from 0002's event taxonomy (Key/Mouse/Resize/Tick/Message/
  Focus). Name the integration (a `Message`? a Resize-like broadcast with its
  own damage rule?) or add the taxonomy entry — else §11.6's invalidation test
  has no driver.
- **R3-D7 [LOW]** — §5.5 `NO_COLOR`: the convention (no-color.org) suppresses
  *color*; stripping bold + retaining reverse-video are Jilva policy choices.
  Cite the convention, label the extensions as policy.
- **R3-D8 [LOW]** — §5 "never relying on bold for meaning" (bold-as-bright
  neutralisation) vs §5.4 Mono tier carrying meaning "only by attributes"
  (incl. bold): add one sentence why bright-substitution preserves mono
  distinguishability (it does — bright-vs-normal remains visible — but the
  reasoning must be on the page).
- **R3-D9 [LOW]** — `blink` deprecated-at-birth with zero legacy users (Q10
  already asks keep-vs-refuse): recommend refuse-entirely/fail-loud; a shim
  for nonexistent legacy is pure surface. Also `dim → Muted`-fallback needs a
  resolution-order note (Muted resolved first; non-text elements unaffected).
- **R3-D10 [LOW]** — citation receipts per A12: `ratatui-themes` crate link,
  Cursive TOML-theme link, Textual design-tokens/`watch` link.
- **R3-D11 [LOW]** — 16-tier role-mapping (Danger→red-family, §5.3) vs 256-tier
  distance metric (§5.2): one sentence why distance works for 256 but roles win
  for 16 (named colors are semantic). Both rules are terminal-true; the
  asymmetry just needs stating.
- **F3 verdicts:** F3.1 (token chain + snapshots) ADDRESSED with the
  strongest test plan in the sprint (§11 tier matrix). F3.2 (profile-as-data)
  ADDRESSED for style; the FULL profile is D2. F3.3 (additive serde/palette)
  ADDRESSED (serde deferred with reasons — modulo D3 wording; palette posture
  matches upstream optionality). F3.4 (override-merge semantics) ADDRESSED
  (delta-over-base, last-writer-wins). F3.5 (extension fallback) ADDRESSED as
  designed (Text+marker, Q3 fail-loud alternative tabled honestly).

## E. Cross-RFC consistency

- **X1 [BLOCKER]** — Change-class divergence triggers 0001's own amendment
  clause. 0001 Q13: three classes (none/repaint/layout); "Reconstruct … is
  *not* a fourth change class … If RFC-2 shows a case that needs a distinct
  fourth class, it must amend this RFC explicitly." 0002 §5: three classes
  with DIFFERENT names and members (State-only/Layout/**Structure**, where
  Structure = mount/unmount/key-change/type-replacement ≈ reconstruct as a
  distinct class) and NO `none` counterpart — while 0002 §1 rule 4 mandates
  "exactly one view + diff per source event", leaving 0001's `none` class
  unimplemented. 0002 never maps its names onto Q13 nor requests amendment.
  Resolution: (a) Structure ≡ reconstruct-output (rename + mapping table,
  keep three), or (b) Structure is genuinely fourth (file the Q13 amendment).
  Mechanical, but the clause exists precisely for this — use it.
- **X2 [BLOCKER]** — Shaping/segmentation owner missing in ALL THREE drafts
  (predicted §2.3). 0001: segmentation alpha-Must, "layout never shapes text".
  0003 Q8: style width-neutral + cluster-aligned, authority = "layout/renderer
  RFCs". 0002: views emit "regions + styled spans" via `frame.text(…)` with no
  shaping step anywhere. Nobody segments, nobody shapes, nobody owns the
  `unicode-segmentation`/`unicode-width` version pins. Acceptance needs a named
  owner (core text helper vs renderer vs layout-measure) + authority versions,
  even with implementation deferred to Phase 3/4.
- **X3 [HIGH]** — `Cell.symbol: char` replacement scheduled nowhere. 0001 bans
  char-math (Out row ✓) and demands segmentation (Must ✓); no draft owns the
  type replacement, its phase, or its dependency path (see D3). Required
  Discussion outcome: one sentence in 0001 Q4 (or the Phase-3 plan) — "the
  rendering RFC replaces the cell symbol type with a grapheme-capable type
  backed by named segmentation/width authorities" — plus D3 clarification so
  the deps are pre-approved in principle.
- **X4 [MEDIUM]** — Diagnostics sink vs no-globals, cross-cutting: 0002
  (debug-mixing logs, `cleanup` log-only, damage-containment asserts) + 0003
  (debug-visible markers, theme test helpers) assume a sink; ARCHITECTURE §2.3
  forbids ambient state. Assign to the lifecycle/test-double RFC: candidates
  are test-double diagnostic stream, returned-diagnostics values, or
  feature-gated `eprintln!` — pick one.
- **X5 [MEDIUM]** — Clock ownership split three ways: 0001 Open Q3 (clock
  abstraction), 0002 `Tick(now)` + subscriptions (C8), 0003 `reduced_motion`
  signal. Name the owner (lifecycle-RFC candidate) and the fixed-clock test
  seam; until then C8 and Q3 cannot close.
- **X6 [MEDIUM]** — Frame-boundary integration seams: (a) event-name drift —
  0001 sketch (`KeyPressed`/`Resized(size)`) vs 0002 taxonomy
  (`Key`/`Resize(w,h)`); sketches are non-normative but align before
  Discussion ends; (b) capability-refresh atomicity — 0003 Q7 (profile refresh
  at frame boundaries) × 0002 §1 batching (what does a mid-batch profile change
  do?); state drop-vs-complete for profiles alongside C7's resize answer.

## F. What's solid (acknowledged — adversarial, not hostile)

- **0001:** the 14-question structure + Must/Should/Could/Out matrix is genuine
  scope discipline; the Layout row satisfies F1.1 with no solver leak; Q10's
  measurable-form-not-values is exactly the R1 §3 recommendation; interpretation
  flags are honest (and three can now be upgraded per A3–A5); Q13's amendment
  clause is well-designed — X1 is that clause working, not failing; Open Q7's
  self-flagged ARCHITECTURE tension shows careful reading.
- **0002:** the §2 identity table (positional/keys/collision/type-change/branch
  rules + debug-assert-never-panic-in-release) is the best-specified contract
  in the sprint and directly answers §8.8 + API_DESIGN §4's misuse-behaviour
  demand; signals rejection on no-globals + determinism grounds is correct;
  proc-macro rejection carries real rationale (compile time, error opacity,
  pre-0.1.0 syntax freeze); total-order + replayable scripts + overflow-cap
  honesty; Open Q2/Q3 show the author knows where the bodies are buried.
- **0003:** §2 token-by-token inclusion/exclusion justifications are thorough
  (esp. three-tier surfaces, Info≠Accent, no per-widget tokens); the
  Unknown-background no-guess rule with catastrophic-vs-suboptimal reasoning is
  correct and important; the per-attribute degradation table is terminal-true;
  §11's tier snapshot matrix is the strongest test plan of the three drafts;
  the shared-question ownership section (B/D/I) is exemplary sprint handoff;
  Drawbacks admits the closed-enum semver cost plainly.

## G. Verdicts + handoff

- **0001 — DISCUSSION-READY.** No blockers. Discussion must produce: X3
  scheduling sentence (with D3 dependency-path clarification) and D2 profile-
  ownership decision. Top challenges: B1 (Ratatui restoration correction),
  X3, D2-share, B3 citations.
- **0002 — DISCUSSION-READY WITH PRECONDITIONS.** The hybrid direction is
  well-argued and §2 is excellent, but C1 (heterogeneity mechanism) and C2
  (owned-descriptions vs `&mut Frame`) are BLOCKERs for acceptance and should
  OPEN the Discussion: require the author to state the intended resolution
  direction for both in the Discussion kickoff, or debate will orbit sketches
  that cannot compile. P3 evidence (`E0038`, enum-vs-erasure tradeoff) is
  attached above for that debate. Further must-answer-in-Discussion: C3, C5,
  C7–C9.
- **0003 — DISCUSSION-READY.** D1 is a BLOCKER for acceptance but has a small
  fix surface (stack Focused or re-scope §9 + §11.2) — fixable inside the
  Discussion week. Further must-answer: D2, D3. Should-answer: D4–D6.
- **Prototype log update:** P2 remains correctly UNRUN (no layout/solver RFC in
  this sprint; Taffy unchosen — nothing to disambiguate). P3 RUN (trigger
  condition met: 0002 omits the §1.3 mechanism): location
  `C:\Users\hamro\AppData\Local\Temp\opencode\jilva-proto-hetero\`, zero-dep;
  tested E0038 failure + dyn-render success; NOT tested enum ergonomics,
  erasure cost, allocation rates.
- **Top cross-cutting challenge for the main session:** the sprint's hardest
  unsolved problem is not any single RFC but the unnamed middle layer — text
  shaping/segmentation ownership (X2), the cell-type replacement schedule
  (X3), and the capability-profile superset (D2) form one connected
  terminal-truth package. Recommend the main session assigns all three to
  explicitly named owners before closing Discussion, or each RFC will assume
  another one handled it.
- **Repo hygiene:** edited ONLY `docs/research/ARCHITECTURE_FEASIBILITY.md`
  (this append). No production code touched; prototypes live outside the repo;
  no commits.
