# RFC Design Review — Jilva Phase 1.5 (Rounds 1+2: Governance, CI, Accuracy + Final Architecture Quality Audit)

- Scope: RFC governance, CI status, ROADMAP accuracy, tree security scan,
  MIT-only consistency, toolchain-pin conformance (Round 1, §§1–5) +
  per-RFC design findings, confirmed cross-RFC conflicts, shared-question
  coherence, open decisions, per-RFC verdicts (Round 2 final, §§6–7).
- Branch: `phase1.5-rfc-sprint` (checked out; no switch/commit/push performed).
- Working tree at review time (Round 2): `M docs/ROADMAP.md` (Round-1
  corrections) + untracked `docs/RFC/0001-framework-scope.md`,
  `docs/RFC/0002-component-model.md`, `docs/RFC/0003-theming-architecture.md`,
  `docs/research/`, `docs/reviews/`. No commits by this reviewer.
- Round 2 reads the three drafts in their revised post-Round-1 state plus
  `docs/research/ARCHITECTURE_FEASIBILITY.md` incl. its appended Round-2
  challenge section. **Nothing is labelled Accepted.** Verdict scale is
  READY FOR DISCUSSION / NOT READY only.

## 1) Governance (rules for the coming RFC-1/2/3)

Source of truth: `docs/RFC/README.md`, `docs/RFC/0000-template.md`.

### 1.1 Lifecycle (binding)

`Draft` → `Discussion` (minimum 1 week) → `Accepted` | `Rejected` | `Postponed`.

- **Draft** — a PR adding `docs/RFC/NNNN-title.md` written from the template.
  Number = next free number (so the coming drafts are RFC-1, RFC-2, RFC-3
  only if no other numbered RFC exists; verify `docs/RFC/` contains only
  `0000-template.md` + `README.md` at the time of drafting — confirmed true
  in this round).
- **Discussion** — announced in Ideas; review focuses on flaws, alternatives,
  prior art. The 1-week minimum is a hard gate: no RFC-1/2/3 may move to a
  decision before 7 full days of Discussion, even under sprint pressure.
- **Decision** — a maintainer merges (accepted) or closes with reasons.
  Accepted RFCs may still change only via new short amendment RFCs.

### 1.2 Naming / placement rules for RFC-1/2/3

- One file per proposal: `docs/RFC/0001-scope.md`, `docs/RFC/0002-components.md`
  (exact titles TBD by authors), `docs/RFC/0003-theming.md`. filename pattern
  `NNNN-kebab-title.md`, zero-padded 4-digit number.
- One proposal per RFC. RFC-1 = scope/non-goals; RFC-2 = component model;
  RFC-3 = theming. No merging of topics into a single file.
- Every RFC MUST contain all template sections: Summary, Motivation,
  Proposal (with user-facing code sketch), Prior art (Ratatui / Cursive /
  Bubble Tea / Textual — what is borrowed vs deliberately rejected),
  Non-goals, Alternatives considered (≥1 serious alternative with
  trade-offs), Open questions (numbered), Drawbacks (honest costs).
- Header block required: Status, Author(s), Created date, Related
  (issue / Discussion link). Initial Status MUST be `Draft` — never
  `Accepted`, `Proposed`, or `Implemented` at creation.

### 1.3 Status vocabulary (do not conflate)

- `Draft` / `Discussion` = pre-decision. `Accepted` / `Rejected` /
  `Postponed` = terminal decision states, set only by maintainer merge/close.
- ROADMAP/doc checklists use a different axis
  (Implemented / Documented / Proposed / Discussion / Accepted / Verified).
  A checked ROADMAP box is NOT an RFC acceptance. This review preserves that
  distinction and Round 2 must do the same.

### 1.4 Acceptance authority

- Only a maintainer decision (merge = accepted, close-with-reasons =
  rejected/postponed) confers `Accepted`. Subagent reviews, sprint consensus,
  CI green, or self-declared status never constitute acceptance.
- This Round-1 review grants no acceptances. Round 2 will recommend;
  maintainers dispose.

### 1.5 Discussion-period enforcement

- Each of RFC-1/2/3 needs its own Discussion announcement + link recorded in
  its `Related:` header.
- Review focus per `docs/RFC/README.md`: flaws, alternatives, prior art.
- Early-merge (before 1 week) = procedural violation regardless of content
  quality; flag as blocking in Round 2 if observed.

## 2) CI status (real, evidenced — NOT inferred)

- Method: `gh run list --repo jilva-co/jilva-rs --limit 5` + `gh run view`
  of latest run. No local `cargo` run substituted for CI status.
- Result: **Passed** (latest `main` push run green).
  - `gh run list` (2026-10-16) showed 3 completed/success runs; latest:
    `38921882379` ("chore: verify hamroqulovv signing key", `main`, push).
  - `gh run view 38921882379`: all 11 jobs ✓ — fmt, check ×3 OS,
    clippy ×3 OS, test ×3 OS, doc. Annotations are informational only
    (Ubuntu 26 migration notice, macOS capacity notice); no failures.
- Scope caveat (important): these runs are on **`main`**, not on the
  current `phase1.5-rfc-sprint` branch. `ci.yml` `on.push.branches` lists
  only `[main, phase1-foundation]` — pushes to `phase1.5-rfc-sprint` do
  **not** trigger CI. Per instructions no cosmetic CI edit is made; the
  sprint should rely on PR runs (PRs trigger regardless of branch) or have
  a maintainer extend the branch list. Do NOT read "CI green on main" as
  "the sprint branch is CI-verified."
- Workflow hygiene (read-only): `permissions: contents: read` (least
  privilege ✓), no secrets/publishing/release jobs in Phase 1 ✓, consistent
  with the "no secrets, no publishing" comment in `ci.yml:8-9`.

## 3) ROADMAP accuracy (verified against the actual tree)

Checked each Phase-1 claim against the filesystem. Corrections below are
factual checklist-state only; status-word distinctions preserved; no RFC
acceptance implied.

| ROADMAP claim | Tree evidence | Assessment |
|---|---|---|
| `[x]` Org profile / community files (`jilva-co/.github`) | External repo; not verifiable from this tree | Leave as-is; unverified locally, not contradicted |
| `[x]` Repo vision (README) | `README.md` present | Accurate |
| `[x]` Architecture boundaries (`docs/ARCHITECTURE.md`) | Present | Accurate |
| `[x]` API principles (`docs/API_DESIGN.md`) | Present | Accurate |
| `[ ]` Cargo workspace exactly per ARCHITECTURE §7 (crates ×3, `examples/`, `tests/`, `benches/`; no root `src/`, no extra crates) | `crates/{jilva,jilva-core,jilva-terminal}` ✓, `examples/version.rs` ✓, `tests/facade_smoke.rs` ✓, **no `benches/` dir** (`Test-Path` = False), no root `src/` ✓ | **Stale-in-part, left unchecked intentionally**: box correctly remains `[ ]` — `benches/` is genuinely missing. Correcting the box to `[x]` now would be false. Flag for sprint: either create `benches/` (even with a placeholder harness wired per facade-only rules) or amend the ROADMAP line if benches are deferred. |
| `[ ]` RFC process published (`docs/RFC/README.md` + template) | **Both files present** (`docs/RFC/README.md`, `docs/RFC/0000-template.md`) | **Factually stale — CORRECTED to `[x]`** in this round (1-box edit, no wording change) |
| `[ ]` RFC-1 / RFC-2 / RFC-3 | Drafts landed as untracked files in Round 2 (`docs/RFC/0001-framework-scope.md`, `0002-component-model.md`, `0003-theming-architecture.md`); all `Status: Draft`, none accepted | Accurate unchecked — drafts existing ≠ accepted; box flips only on maintainer acceptance |
| `[ ]` Contributor docs (`docs/GETTING_STARTED.md`, `docs/DEVELOPMENT.md`) | **Both files present** in `docs/` listing | **Factually stale — CORRECTED to `[x]`** in this round (1-box edit, no wording change) |

- Corrections made: two checkbox flips (`[ ]` → `[x]` for "RFC process
  published" and "Contributor setup … docs"). Nothing else in ROADMAP.md
  touched; all section prose, exit criteria, and Phase 2–7 content untouched.
- Deliberately NOT corrected: workspace box stays `[ ]` (benches/ missing —
  marking it done would be the exact "empty = success" error this audit
  forbids).

## 4) Security scan of the tree

- **Credentials/secrets:** literal-pattern grep for `api_key|secret|
  password|token = "…"` returned only benign hits — the `ci.yml` "No
  secrets" comment, `docs/DEVELOPMENT.md` "No secrets, tokens, or extra
  services needed", and audit-doc mentions. No keys, tokens, or passwords
  in the tree. ✓
- **Workflow permissions:** `contents: read` only; no `write`, no
  `id-token`, no secret references, pinned third-party actions
  (`checkout@v5`, `rust-cache@v2`, `rust-toolchain@stable`). ✓
- **Dependencies (supply chain):** zero third-party deps confirmed —
  `Cargo.lock` lists only the 3 workspace packages; `[dependencies]` in
  member crates are workspace `path` deps only (`crates/jilva/Cargo.toml`
  verified; `PHASE1_AUDIT.md` claim corroborated). No misleading "zero-dep"
  claim found: `jilva-core/src/error.rs` doc comment ("zero third-party
  dependencies") is currently true. `SECURITY.md` honestly states
  pre-release / no-production-use posture with a private-reporting path. ✓
- **Residual notes (non-blocking):** `SECURITY.md` Scope mentions `src/`
  (root `src/` does not exist — workspace layout uses `crates/*/src/`);
  read as shorthand, worth a one-word fix by a maintainer later. No action
  taken (out of Round-1 scope).

## 5) MIT-only consistency + toolchain-pin conformance

- **License:** `LICENSE` is the standard MIT text (Copyright (c) 2026 Jilva
  contributors). `Cargo.toml` workspace + `crates/jilva/Cargo.toml` declare
  `license = "MIT"` (workspace-inherited). No GPL/Apache/MPL text or
  dual-license marker found. **MIT-only holds — no deviation to flag.**
  (Per instructions: flag-only, never change.)
- **Toolchain pin:** `rust-toolchain.toml` pins `channel = "1.98.1"` with
  `rustfmt` + `clippy` components. ✓ present and exact.
  - **Drift risk recorded (no edit made):** `ci.yml` installs via
    `dtolnay/rust-toolchain@stable` (floating stable), NOT the `1.98.1`
    pin. Local builds pin; CI floats. If stable moves past 1.98.1 with a
    breaking lint/fmt change, CI could fail (or pass) code the pinned local
    toolchain treats oppositely. Recommend the sprint either pin CI to
    `1.98.1` or document floating-stable as deliberate — maintainer call,
    not a Round-1 edit.

## 6) RFC-1/2/3 findings (Round 2 final)

Method: each Round-1 feasibility finding and each Round-2 challenge
BLOCKER/HIGH re-verified against current draft text with `file:line`
evidence. Findings the parallel revision already fixed are **retired with
proof**, not repeated. Severity scheme: **BLOCKER** = acceptance must not
happen until resolved (Discussion may proceed); **HIGH** = owned + answered
during Discussion or becomes BLOCKER at acceptance; **MEDIUM** = answer in
Discussion, deferrable only with a tracked question entraining no API;
**LOW** = editorial/citation hygiene. Template note: `0000-template.md`
defines **8** required sections + header block (not 18 — the brief's "18"
matches no template count; audited against the actual 8). All three drafts
contain all 8 (Summary, Motivation, Proposal, Prior art, Non-goals,
Alternatives, Open questions, Drawbacks) plus sketches, records, and
amendment history. Header `Status: Draft` in all three
(`0001:3`, `0002:3`, `0003:3`); lifecycle notes present (`0001:9-13`,
`0002:10`, `0003:11-12`). No `Accepted` status claimed anywhere: the only
"accepted" strings are conditional/future governance language (`0001:11`
"Nothing in this document is accepted until a maintainer merges it";
`0003:157` "Once this RFC is accepted, … requires an amendment RFC";
`0001:391` "pending acceptance") — all correct usage.

### 6.1 Stale-challenge retirements (fixed by the parallel revision — with proof)

- **RETIRED-FIXED — R3-D1 (single-overlay contradiction).** RFC-3 withdrew
  single-winner: `0003:186-208` now specifies ordered stacking
  (`Hovered < Active < Focused < Selected < Disabled`, `Disabled` absorbing)
  with the `Selected + Focused` canonical case; OQ4 (`0003:519-527`) records
  the resolution. Residual escalated to maintainer/widgets-RFC evidence:
  snapshot coverage of just the `Selected + Focused` pair vs every pair.
- **RETIRED-RESOLVED-BY-ADOPTION — X1 / R2-C1 / RFC-1 "CONTESTED"
  (change-class divergence).** RFC-2 §5 (`0002:204-210`) adopts RFC-1 Q13
  verbatim — `none` / `repaint` / `layout`, "deliberately no fourth class,"
  with an explicit amend-RFC-1 rule if a fourth class is ever needed.
  RFC-1's Q13 position (`0001:248-260`) and RFC-2 now agree in substance.
  **Doc-sync owed (not a design conflict):** RFC-1 still labels this
  CONTESTED in three places written against the older RFC-2 — Q13
  (`0001:260-273`), §4 shared-A note (`0001:347-351`), record §R2-C1
  (`0001:659-669`) — all reference "RFC-2 §5 paint / layout / structure,"
  which no longer exists. Likewise RFC-3's terminology map (`0003:391-397`)
  maps "RFC-2 state-only(paint) / layout / structure" — stale names for the
  now-adopted triple. Fix: update the three RFC-1 passages + the RFC-3 map
  to `none / repaint / layout` during Discussion week. Severity of the sync
  itself: LOW.
- **DOWNGRADED BLOCKER→MEDIUM — R2-C1 (heterogeneity mechanism).**
  Mechanism now named: `0002:329-338` specifies a `dyn`-compatible render
  seam (object-safe `&self` methods, no generics on the render path), a
  homogeneous fast path (static dispatch, no boxing), and a mount-once
  boxing rule (≤1 `Box` per heterogeneous child, never per-frame). This
  answers the P3 `E0038` challenge directionally (associated-State/Message
  trait stays non-`dyn`; render splits to a `dyn` seam). Residuals keeping
  it MEDIUM: the heterogeneous-array sketch (`0002:446-454`,
  `frame.column([Header, TodoList, StatusBar])`) is not rewritten in the
  mechanism's terms (no erased-element type shown); enum-dispatch vs
  erasure fallback unchosen; allocation rule unprototyped. Sketch-mechanism
  alignment required before acceptance.
- **RETIRED-ADDRESSED — R2-C5 (key storage).** Storage family named:
  `0002:341-349` — framework-owned hand-rolled generational slot table
  (`Vec`-backed, zero-dep justification), generational indices,
  sibling-level `Key → index` map; behaviour contract unchanged from §2.
  Residual LOW: allocation/benchmark detail for the implementation RFC.
- **RETIRED-ADDRESSED — X3 (Cell-replacement scheduling).** The required
  sentence exists: `0001:312` (Unicode matrix row) schedules the
  `Cell { symbol: char }` replacement "no later than Rendering Engine
  (Phase 3)" with the first buffer/diff RFC owning symbol-type shape +
  width/segmentation authorities (detail list `0001:598-606`, OQ12).
  Residual LOW: dependency-path pre-approval (see D3 residual below).
- **RETIRED-RESOLVED — R3-D6 (theme-switch loop path).** Integration now
  named on both sides: RFC-3 version-bump + paint-only damage (`0003:399-406`,
  `theme_version` / `profile_version`); RFC-2 counter plumbing (`0002:221-226`,
  version mismatch → re-resolve + repaint, measure/arrange skipped). No
  taxonomy entry needed — the version path, not an event, is the driver.
- **CONFIRMED-FIXED (author self-fixed) — R2-C12 typo.** §6 now reads
  "abandons the batch" (`0002:259-260`). Residual LOW carried: normative
  `init` fallible via `Result` (`0002:254-256`) vs sketch `fn init() ->
  Self::State` infallible (`0002:430`) — align sketches after the C2 pick.

### 6.2 RFC-1 findings (scope) — all confirmed-current

- **MEDIUM — R1-B1 (Ratatui contrast overstates).** `0001:63-65` + Prior
  art (`0001:440-452`) still claim Ratatui "leaves the loop, state
  ownership, and restoration to user code." Restoration is framework-owned
  in current Ratatui (`ratatui::run` / `init` / `restore` + panic hooks).
  Fix: contrast on event-loop + state ownership (true, load-bearing), drop
  "restoration" or scope it to per-exit-path policy. Wording only;
  decision unaffected.
- **LOW — R1-B5 ("ever").** `0001:187-188` ("No audio output, ever in
  current scope"), matrix rows (`0001:324-325` "Out. … ever in current
  scope") contradict Q9's own revisit rule (superseding ADR + RFC). Delete
  "ever" (2 spots + `0001:325` 3D twin).
- **LOW — R1-B4 (typo).** `0001:489-490` "framework Erzählen" — stray word.
- **LOW — Open-question cross-references.** §5 table: row 8.3 (`0001:385`)
  cites "(Open Q13)" for width/segmentation authorities — that's OQ12
  (`0001:598`); row 8.6 (`0001:388`) cites "(Open Q14)" for the keyboard
  ladder — that's OQ13 (`0001:607`). Re-point to Q12 / Q13.
- **LOW carried — R1-B2/B3/B6.** License-hedge upgrades (Bubble
  Tea→MIT, Textual→MIT, iocraft→MIT-or-Apache-2.0 confirmed per
  feasibility §A) and prior-art softening (Ink internals, OpenTUI "Yoga",
  Textual specifics) not re-verified line-by-line here; owned as
  Discussion-week hygiene. B6 (grid placement-without-spanning coherence)
  stands as a Discussion question, presumed coherent.
- **F1 verdicts (carried, spot-confirmed):** F1.1 SATISFIED (bounded
  vocabulary, `0001:307`, no solver leak); F1.2 scheduling SATISFIED via
  X3 retirement, details in OQ12; F1.3 SATISFIED (async/CSS-units/
  macro-primary refusals, `0001:176-189`); F1.4 SATISFIED in proposal form
  (§6 bar `0001:398-412` + ~14-line G-sketch `0001:414-432`; RFC-2 sketches
  carry counts `0002:384-389`); F1.5 SATISFIED (§5 14-row §8 mapping,
  `0001:375-396`, modulo the Q12/Q13 cross-ref slips above).

### 6.3 RFC-2 findings (component model) — confirmed-current

- **BLOCKER (for acceptance; Discussion proceeds) — R2-C2 (two frame
  architectures).** Normative §§1–3 + §8 sell owned frame descriptions:
  "frame descriptions are owned values … never `&` into component state"
  (`0002:142-146`), "`view` takes `&` only" + "returns nothing by
  reference" (`0002:316-327`). Every sketch threads a mutable draw target:
  `fn view(app: &App, frame: &mut Frame)` (`0002:398`, `410`, `417`,
  `431`, `439`, `448`) with `frame.text/column/list` builder calls —
  Ratatui-style immediate draw, whose sibling composition needs
  region-scoped sub-borrows (layout computed inline during view — owner
  unnamed) or interior mutability (contradicts §1 rule 2, `0002:66-67`).
  The "never fight the borrow checker" story (`0002:351-358`) is proven
  for the owned model, unproven for the `&mut Frame` model. **Pick one as
  normative and rewrite the sketches to match before acceptance.**
- **MEDIUM — R2-C3 (return-taxonomy blur).** `update` returns change class
  (`0002:234`); handlers return `Outcome` (`0002:169-172`); sketches return
  `Change::Repaint` (`0002:410-415`) vs `Outcome` (`0002:431`). No single
  normative signature shape (e.g. `UpdateResult { consumption, change }`)
  — Discussion will otherwise burn a week on vocabulary.
- **MEDIUM — R2-C4 (`Send` bound).** `0002:307-309` requires `Model: Send`
  for a single-threaded zero-async loop; OQ3 (`0002:581-583`) already
  suspects it. Recommend `Send` only at the effect-bridge boundary.
- **MEDIUM — R2-C6 (`Observed`).** Present `0002:184-187`, flagged by its
  own author; OQ1 (`0002:574-576`) leans drop. Cut it or specify
  order-after-consumers + sink purity (replay argument `0002:197-200`
  depends on it).
- **MEDIUM — R2-C7 (resize policy).** `0002:164-168` punts drop-vs-complete
  to "implementation detail with a test"; ARCHITECTURE §8.5 requires the
  design to state it. Adopt-or-rebut Ratatui's precedent, then test the
  stated policy.
- **MEDIUM — R2-C8 (tick subscriptions).** `0002:173-179` declares
  subscriptions + injected `now` (good: position on RFC-1 OQ3 taken) but
  declaration/storage lifecycle stays thin — pure-`&` `view` cannot
  register, per-event `update` storing registration is unspecified. Name
  subscribe/unsubscribe points (mount/unmount + declared-intent channel)
  or "zero CPU" (`0002:175-176`) is unimplementable as written.
- **MEDIUM — R2-C9 (`Effect::Task`).** `0002:301-305` keeps
  `Effect::Task(fn_once_bridge)` beside the "data-only" rule; a `fn`
  pointer executed *by core* implies an execution policy
  (threads? poll point?) — executor-by-stealth against the
  runtime-agnostic constraint. Show opaque-data + external execution, or
  restrict 0.1.0 effects to timer-data.
- **LOW carried — R2-C10/C11.** Cursive-`dummy`-backend acknowledgement
  (`0002:473-479` still frames headless testing as the charge; accurate
  charge is emergent callback order) and prior-art receipts (discussion
  #552, Xilem async-depth) — Discussion-week hygiene, not re-verified here.
- **F2 verdicts:** F2.1 ADDRESSED directionally (`0002:316-327`, entangled
  in C2); F2.2 mechanism NAMED (C1 residual above); F2.3 seam order +
  method-count justification PRESENT (`0002:289-300`); F2.4 clone budget
  EXPLICIT ("no clone", `0002:360-372`); F2.5 macro-free core COMMITTED
  (`0002:280-288`); F2.6 reconciliation storage NAMED (C5 retired),
  layering ASSIGNED (`0002:366-372`).

### 6.4 RFC-3 findings (theming) — confirmed-current

- **HIGH — R3-D2 (input/screen capability homeless).**
  `CapabilityProfile` (`0003:301-310`) carries only
  color/attrs/background/reduced_motion, yet RFC-1 Q12 promises the
  key-encoding ladder + gated mouse + alt-screen fallback and RFC-2
  (`0002:150-151`) gates mouse on capability. **Resolution during
  Discussion:** either this struct IS the shared profile (add
  keys/mouse/viewport fields or explicit non-exhaustive reservations) or
  it is the style projection (name the superset owner — likely
  backend/lifecycle RFC — + projection rule). Shape is on RFC-1's J-list
  (`0001:285-293`), so it cannot drift.
- **MEDIUM — R3-D3 (zero-dep wording, partially stale).** `0003:267-269`
  cites a "zero-dep constraint" for excluding `serde`; RFC-1's constraint
  (`0001:336-338`) says "zero-dependency `jilva-core` **within the
  workspace**" — the qualification the challenge asked for now exists, so
  the "forbids unicode-width/segmentation" reading is weakened but the
  precedent-setting compression remains. Residual: one sentence
  pre-approving the Cell-replacement dependency path in principle
  (MIT-gate-passing width/segmentation/SSO candidates, `0001:598-606`),
  else X3's schedule has no approved means.
- **MEDIUM — R3-D4 residual (contrast-matrix scope).** Tier matrix now
  names the BackgroundHint dimension (`0003:592-598`); automated ratio
  checks remain truecolor-only (`0003:603-605`) while the Unknown default
  (`0003:607-610`) is unchanged-theme. Downgraded: state the check matrix
  (theme × tier × background incl. Unknown) or explicitly downgrade the
  Unknown guarantee — one sentence.
- **LOW — R3-D5 (normative vs question).** `0003:322-323` mandates
  "minimum perceptual distance" while OQ5 (`0003:528-531`) asks
  Euclidean-vs-perceptual. Move the metric into the question; normative
  text requires "a documented metric (Q5)".
- **LOW — RFC-3 terminology-map sync.** See X1 retirement: update
  `0003:391-397` to RFC-2's adopted `none / repaint / layout` names.
- **LOW carried — R3-D7–D11.** `NO_COLOR` convention citation, bold/mono
  reasoning sentence, blink refuse-vs-shim, citation receipts, 16-vs-256
  asymmetry sentence — Discussion-week hygiene, substance terminal-true.
- **F3 verdicts (carried, spot-confirmed):** F3.1 ADDRESSED (token set
  `0003:98-120` + justifications + tier matrix `0003:592-619`, strongest
  test plan in the sprint); F3.2 ADDRESSED for style (profile-as-data,
  backend sees resolved attributes only, `0003:70-75` + alternative 3
  `0003:498-502` upholding ADR-0003); F3.3/F3.4 ADDRESSED (additive-only
  serde/palette posture `0003:267-281`, modulo D3 wording); F3.5 RESOLVED
  (normative merge order + delta-merge test `0003:253-259`, `0003:588-591`).

### 6.5 Cross-RFC conflicts (confirmed-current only; stale X1 excluded)

- **X2 — HIGH residual (was BLOCKER).** Shaping/segmentation owner now
  NAMED ("first buffer/diff RFC", `0001:598-606` + §5 rows 8.3/8.4;
  recorded dependency `0002:374-380`; style-side constraint `0003:541-550`
  OQ8) but authority versions/pins absent and implementation deferred to
  Phase 3/4. Acceptance needs owner + named authority versions; owner is
  present, versions pending. Keep HIGH, not BLOCKER.
- **X4 — MEDIUM (confirmed).** Diagnostics sink still ownerless: RFC-2
  debug-mixing logs + `cleanup` log-only + damage asserts, RFC-3
  debug-visible markers + theme test helpers, vs ARCHITECTURE §2.3
  no-ambient-state. Assign to lifecycle/test-double RFC (test-double
  diagnostic stream, returned-diagnostics values, or feature-gated
  `eprintln!`).
- **X5 — MEDIUM residual.** Clock positions now agree (injected plain-data
  `now`, `0002:173-179`; RFC-1 OQ3 `0001:552-556` open but scoped); owner
  (lifecycle RFC) + fixed-clock seam named. Closes when the lifecycle RFC
  names source + seam; C8 depends on it.
- **X6(a) — LOW (confirmed).** Event-name drift: RFC-1 sketch
  (`0001:425-427`, `KeyPressed`/`Resized(size)`) vs RFC-2 taxonomy
  (`0002:150-152`, `Key`/`Resize(w,h)`). Sketches non-normative; align
  before Discussion ends.
- **X6(b) — MEDIUM residual.** Capability-refresh atomicity (RFC-3 OQ7
  `0003:537-540` frame-boundary refresh × RFC-2 batching `0002:73-76`):
  state mid-batch profile-change semantics alongside C7's resize answer —
  lifecycle-RFC owned.

### 6.6 Shared questions A–J coherence

| Q | Subject | Status | Evidence |
|---|---|---|---|
| A | Change classes | **Agreed in substance** (doc-sync owed) | `none/repaint/layout`: `0001:248-260`, `0002:204-210`; sync fixes §6.1 |
| B | Theme propagation w/o rebuild | **Agreed** | Version + paint-only: `0003:399-406`, `0002:221-226`, recorded `0001:365-367` |
| C | Determinism / replay | **Agreed** (residual: observer ordering) | `0002:197-200`; R2-C6 |
| D | Capability entry | **Agreed for style; OPEN for input/screen** | `0003:296-310`, `0003:641-645`; R3-D2 |
| E | Headless testability | **Agreed** | `0002:242-246`; `0003:575-619` headless matrix |
| F | Widget reuse w/o privates | **Agreed in principle** (mechanism residual) | Slots + messages `0002:433`; R2-C1 residual |
| G | Smallest complete API | **Agreed** | `0001:276-284` + sketch; `0002:384-389` brevity; `0003:245-252` deltas |
| H | Async compat | **Agreed** (execution-policy residual) | Sync core + data-only Effect; R2-C9 |
| I | Unicode-hard choices | **Agreed direction; HIGH residual** | Constraints aligned; X2 (versions pending) |
| J | Expensive-to-change | **Agreed + honoured** | 7-item list `0001:285-293`; identity/token/profile impacts flagged |

### 6.7 Architecture-quality checks (all three drafts)

- **Crate-boundary respect:** PASS. Detection-in-terminal / pure-resolution-
  in-core split held (`0003:296-310`, alt-3 `0003:498-502` upholds ADR-0003
  decision 2); no backend types in theme/component APIs; third-party seam
  order explicit (`0002:296-300`, depend-on-churning-core with eyes open
  per ADR-0002). No async-runtime coupling: sync loop baseline, zero async
  deps asserted (`0002:301-305`), `Effect` data-only (residual R2-C9).
- **ADR compatibility:** PASS. ADR-0001 (siblings after alpha; portability
  explicit non-goal `0001:160-174`, `0003:162-166`); ADR-0002 (no premature
  splits; extraction rule `0001:133-136`); ADR-0003 (backend seam, no
  engine-type leakage — iocraft-Taffy rejection `0001:479-483`,
  resolution-in-backend rejection `0003:498-502`). One self-flagged
  wording watch carried: ARCHITECTURE §3.9/§4.1 error-visibility
  (`0001:566-572`, OQ7) — confirm, no change proposed.
- **MIT-only:** HOLDS. No new dependency introduced (docs only); `palette`
  posture explicitly additive + gate-noted (`0003:277-281`); `serde`
  additive-only (`0003:267-276`); toolchain pin intact (`1.98.1`,
  re-verified); license-gate language consistent with the MIT-only rule.
- **ROADMAP accuracy (re-check):** `benches/` still absent (`Test-Path`
  False) — workspace box correctly stays `[ ]`. Drafts exist but all Draft:
  RFC-1/2/3 boxes stay `[ ]` (§3 amended). No other staleness found.
- **Docs internal links:** all `Related:` targets resolve
  (`ARCHITECTURE.md`, `API_DESIGN.md`, ADR `0001`/`0002`/`0003`,
  `ARCHITECTURE_FEASIBILITY.md`, `ROADMAP.md` — all present).
- **CI status (re-verified 2026-10-16):** UNCHANGED from Round 1 —
  `gh run list` shows the same 3 completed/success `main` runs (latest
  `38921882379`, all 11 jobs green). No new runs; nothing to promote or
  demote. Reading stands: **Passed on `main`**; sprint-branch pushes still
  outside `ci.yml` push triggers (`[main, phase1-foundation]`) — branch
  covered at PR time. No CI edit made.

## 7) Verdict (Round 2 final — recommendations, never acceptance)

No RFC is Accepted by this review. Acceptance authority stays with
maintainers after ≥1-week Discussion (§1.4–§1.5).

- **RFC-1 (Framework Scope) — READY FOR DISCUSSION.** No blockers.
  Discussion must produce: D2 profile-ownership decision (shared with
  RFC-3), X2 authority-version naming (with renderer-RFC owner), D3
  dependency-path sentence. Doc fixes in-week: retire the three stale
  CONTESTED passages (§6.1), B1 contrast wording, "ever" deletions, OQ
  cross-refs (Q12/Q13), B4 typo.
- **RFC-2 (Component Model) — READY FOR DISCUSSION WITH PRECONDITIONS.**
  Direction (hybrid MVU + paths/keys + pure view + data-only effects)
  is well-argued and §2 identity is the sprint's best-specified contract,
  but C2 (owned-descriptions vs `&mut Frame`) is BLOCKER-for-acceptance:
  require the author to state the normative pick at the Discussion
  kickoff (per feasibility handoff), or debate will orbit uncompilable
  sketches. Must-answer in Discussion: C1 sketch alignment, C3 signature
  shape, C5 bench detail, C7–C9. Contested items for the week: keys
  advisory-vs-required (OQ2), `Send` bound (OQ3), `Observed` survival (OQ1).
- **RFC-3 (Theming) — READY FOR DISCUSSION.** D1 fixed with small surface;
  Discussion must produce: D2 resolution, D3 clarification, terminology-map
  sync (§6.1), D4 one-sentence matrix/guarantee statement. Should-answer:
  D5 metric placement, OQ4 snapshot-scope escalation to maintainer.
- **Open decisions (cross-cutting, owned before Discussion closes):**
  (1) capability-profile superset owner + shape (D2/X6b — lifecycle RFC
  candidate); (2) shaping/segmentation authority versions (X2); (3) Cell
  replacement dependency path (D3/X3); (4) diagnostics sink (X4);
  (5) clock source + fixed-clock seam (X5/C8); (6) snapshot-scope for
  non-load-bearing overlay pairs (RFC-3 OQ4 residual).
- **Doc fixes needed (Discussion week, no design risk):** RFC-1 stale
  CONTESTED ×3 + B1/B4/B5 + OQ cross-refs; RFC-3 terminology map; RFC-2
  sketch-mechanism alignment (C1/C2) + init fallibility drift; carried LOW
  citation hygiene (B2/B3, C10/C11, D7–D11).
