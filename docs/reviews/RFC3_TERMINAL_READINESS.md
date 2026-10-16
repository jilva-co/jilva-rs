# RFC-3 Terminal Readiness Review — Capabilities, Theming & Unicode

- Reviewer: Subagent 3 — Terminal Capabilities, Theming & Unicode Reviewer (Jilva Phase 1.7)
- Date: 2026-10-16
- Inputs: `docs/RFC/0003-theming-architecture.md` (full, as amended incl. Phase-1.6 notes),
  `docs/research/CAPABILITY_MODEL_RESOLUTION.md` (+ §12 cross-review),
  `crates/jilva-core/src/renderer.rs` (Cell), `crates/jilva-core/src/style.rs`,
  `crates/jilva-terminal/src/capabilities.rs`, `docs/adr/0003-terminal-backend-strategy.md`
- Constraints honoured: MIT-only, no production code changes, no new deps, branch untouched
  (read-only review; this file is the only write).

## Verdicts (one line each)

| # | Area | Verdict |
|---|------|---------|
| A | Domain ownership / detection-in-terminal / data-in-core | **ACCEPT** — hybrid model (§5 note + Resolution §§3–4) is coherent and ADR-0003-compliant |
| A | Snapshot semantics / versions / propagation / unknown handling | **ACCEPT with wording repairs** (3 exact amendments below) |
| B | Atomicity (snapshot-at-start, no torn frames) | **ACCEPT** — policy correct; lifecycle owns queue mechanics |
| C | Theming tokens/overrides/dark-light/degradation/NO_COLOR/a11y | **ACCEPT with 2 clarifications** (NO_COLOR done; ThemeKind + extension-fallback stay open, correctly) |
| C | Snapshot policy (what must be exhaustive) | **ENDORSE Resolution §10** representative+property with named exhaustive islands |
| D | Unicode (`Cell.symbol: char`) | **HOLD — structurally unsound for text, contained only by explicit RFC contract + future-RFC requirement** |

No production code change is recommended in Phase 1.7. One optional containment
change is proposed with full compat/scope analysis (§D.4) — default recommendation:
**document-and-defer, do not touch `Cell` now**.

---

## A. Domains: ownership, detection, snapshots, versions, propagation, unknown

### A.1 Ownership — ACCEPT

- RFC-3 §5 note (Phase-1.6 amendment) correctly states the typed-domain hybrid:
  `ColorProfile` consumed by style, `InputProfile` / `ScreenProfile` named as
  sibling domains owned by the input/lifecycle RFCs, thin `TerminalCapabilities`
  aggregate (domains by value + snapshot id + per-domain versions) travelling only
  the terminal→loop and loop→frame seams.
- Matches Resolution §§3–4 and ADR-0003 decisions 2–3 (backend sees resolved
  instructions only; core never names TTY/env/backend). `style.rs` currently holds
  only `ColorDepth` + 4 illustrative tokens — no detection leakage. `capabilities.rs`
  currently holds 3 bools with `basic()`/`full()` — correctly labelled placeholder,
  no premature tier logic. Both files are consistent with "data shapes now, RFC
  detail later".
- Consumption rule (Resolution §4: module takes its domain, never the aggregate,
  except the two seams) is the load-bearing property. It is stated normatively in
  the Resolution but **not yet restated in RFC-3 itself** — amendment P1 below
  carries the one enforceable sentence over.

### A.2 Detection-in-terminal / data-in-core — ACCEPT

- Direction is right both ways: core defines data (depends on nothing), terminal
  depends on core's data types for detection output (Resolution §4). Facade
  re-exports only. No inversion found.
- `reduced_motion` on the aggregate (Resolution §3.1) endorsed: one bool does not
  justify a domain; promotion path to a future `UserProfile` is defined.

### A.3 Snapshot semantics / versions — ACCEPT with wording repair

- Per-domain versions + aggregate `snapshot_id` (identity vs causality split,
  Resolution §6) is the correct design: `color_version` → repaint-only re-resolve
  with cache key `(theme_version, color_version)`; `input_version` → decoder
  re-arm, no render damage; `screen_version` → layout+repaint via resize path;
  `reduced_motion` → no-op in Phase 1. Merge cached per `theme_version`,
  quantization per `(theme_version, color_version)` — and merge is
  capability-independent (already amended into RFC-3 §4, correct).
- **Repair needed:** RFC-3 §6 "no dedicated propagation walk / no subtree walk"
  overclaims against the owned-View pipeline (Resolution §12.1): `repaint` still
  runs per-frame `describe`; versions gate merge/quantize redo and layout/paint
  skipping, never visitation. Amendment P2 fixes the sentence; no decision changes.

### A.4 Propagation — ACCEPT with combined-invariant repair

- Theme/capability travel via RFC-2 `&Context` threading + version bumps (§6) is
  consistent with Resolution §6 and §12.1 integration answer (all nodes re-describe
  next frame via the normal path; no special walk).
- **Repair needed:** RFC-3 Q7 snapshot-at-start answer should be stated jointly
  with the renderer's fail-clean buffer rule as the single frame-commit invariant
  owned by the lifecycle RFC (Resolution §12.2 + §12.5 item 2). Amendment P3.

### A.5 Unknown handling — ACCEPT

- `BackgroundHint::Unknown` → no guessing, keep app theme, dual-legible defaults
  (RFC-3 §5) correctly generalised by Resolution §5 to all domains
  (`Legacy`/`Off`/`false`/per-flag-false baselines, `Ansi16`-not-`Mono` default).
  Resolution is total over all domain values (no `Result`, no panic). No gaps found.
- Lifecycle implication: unknown values never produce errors, retries, or extra
  frames — they flow as ordinary inputs. No lifecycle state machine change needed.

## B. Atomicity

- **Snapshot-at-start (Resolution §7, RFC-3 Q7): ACCEPT.** Reasoning (a)/(b)/(c)
  is sound: (a) collapses into (c); (b) risks livelock and complicates RFC-2's
  transactional update rule; (c) is correct and cheap (`Copy` snapshot at frame
  start, queue mid-frame events to next boundary).
- **No mid-frame inconsistency:** the single-snapshot-per-committed-frame invariant
  ("never quantize half the tokens at 256 and half at 16", §7) is correctly scoped,
  with structural enforcement (by-value `ColorProfile`) + debug assertion mirroring
  RFC-2's damage-vs-diff philosophy. Test-double mid-frame-injection refusal (§8)
  makes the invariant testable. Endorsed.
- **Resize / color / input invalidation paths vs lifecycle boundary:**
  - Resize (`screen_version`) → full resize ordering (ARCHITECTURE §8.5:
    observe → re-layout → re-render). Lifecycle owns queue mechanics; policy agreed.
  - Color (`color_version`) / theme switch → repaint only, layout provably skipped
    (R2 width-neutrality). No lifecycle transition involved (stays `Running`).
  - Input (`input_version`) → backend-edge re-arm, future-decode only, `none`
    render class. Never synthesised as an `Effect::Task` payload (Resolution
    §12.4 bypass audit: all three bypass paths closed). Endorsed.
  - Combined cases (§12.2: mid-frame change + describe failure → abort, buffer
    retained, change queued not lost; rapid resize → one-frame staleness, no
    livelock): compose cleanly. No lifecycle amendment needed beyond owning the
    combined invariant (P3).

## C. Theming

### C.1 Tokens — ACCEPT

Closed enum (~15 tokens, 4 groups) + `ExtensionMap` pressure valve + amendment-gated
evolution (§2) is endorsed by Resolution §9 (exhaustive quantization snapshots
require a closed set). Extension keys excluded from tier snapshots (Text-fallback
rule, tested once per tier) — correct scope bound. `Selected`-fill-token vs
`Selected`-state-variant split is justified in-text. No change requested.

### C.2 Overrides / merge — ACCEPT

Per-token last-writer-wins, deltas-over-base-over-built-in-default, overlays after
merge, quantization last, capability-independent merge (§4) + delta-merge test
(Testing item 0). Answers F3.5 mechanically. No change.

### C.3 Dark / light / background-Unknown — ACCEPT

Dual-legible defaults + no auto-switch on `Unknown` (§5) + contrast matrix over
Dark/Light/**Unknown** (assert both surfaces) is the correct conservative posture.
`ThemeKind` advisory-only with Open Q2 (warn vs documentary) — correctly left open;
recommendation: keep documentary in Phase 1 (a warning policy needs evidence of
demand; auto-behaviour is already refused).

### C.4 ANSI degradation — ACCEPT

Truecolor passthrough → 256 nearest-palette (metric deferred to Q5, correctly) →
16 role-over-distance (Danger→red etc., with the 16/256 asymmetry sentence present)
→ Mono attribute/symbol carriage. Conservative default `Ansi16`-not-`Mono`
amendment present. No change.

### C.5 Mono / NO_COLOR — ACCEPT (amendments already applied)

Mono-distinguishability test (§11 item 2, incl. explicit `Selected+Focused` pair
per Phase-1.6 amendment) + `NO_COLOR` policy (mono + bold-off/plain + reverse
retained, cited to no-color.org, labelled Jilva policy per R3-D7) + `no_color`
flag carrier (Resolution §3.1 deliberate redundancy) — all present and correct.

### C.6 Selected / focused / disabled — ACCEPT

Ordered stacking `Hovered < Active < Focused < Selected < Disabled` with Disabled
absorbing (§3, Round-2 position) satisfies the alpha-widget requirement
(selected-in-focused vs unfocused). Residual Q4 (snapshot scope for non-load-bearing
pairs) correctly escalated with an evidence-driven graduation rule (Resolution
§10). No further action here; widgets-RFC evidence decides.

### C.7 Accessibility — ACCEPT

Never-colour-alone, Disabled visual+event gate split (visual here, gate in RFC-2),
focus-by-reverse/underline, contrast-checked defaults + test helper, reduced-motion
plumbing now. A11y assertions live in the **exhaustive** islands (mono S+F,
NO_COLOR selection visibility, focus-never-hue-alone per tier — Resolution §10),
not relegated to properties. Correct placement. No change.

### C.8 Snapshot policy — ENDORSE Resolution §10 (what MUST be exhaustive)

Exhaustive golden islands (all else property-tested):

1. Tier matrix: every token × `Normal` × 4 tiers × 3 backgrounds × 2 built-ins
   (≈360 rows) — taste + role-mapping contract.
2. Mono distinguishability (mechanical ≥1-step assertion, not just golden),
   incl. `Selected+Focused` per token at `Mono`.
3. `NO_COLOR` profile: full token set × Normal+Selected/Focused/Disabled —
   zero colour escapes + reverse-visible selection.
4. Unknown-background: full token set × Normal — theme unchanged, surfaces local.
5. Per-attribute singles: every attribute × {Text, Accent, Danger, Selection} —
   documented fallback each (subset exhaustive; full set not repeated).
6. Non-interference property: style goldens invariant across varying
   input/screen with fixed color (proves the §4 consumption rule mechanically).
7. Contrast: numeric assertions (≥4.5:1 body, ≥3:1 chrome, truecolor) per
   built-in × declared background incl. Unknown-both-surfaces; helper for customs.

Rationale endorsed: 30k-row full cross product would rot; goldens pin intent,
properties pin invariants. Graduation mechanism defined (a third pair graduates to
exhaustive mono+truecolor islands by amendment on widget evidence).

## D. Unicode — the `Cell.symbol: char` analysis

Actual code (read 2026-10-16): `crates/jilva-core/src/renderer.rs:13-21` —
`pub struct Cell { pub symbol: char }`, i.e. **one Unicode scalar value per cell**,
with the module docs honestly flagging the limitation ("holds one Unicode scalar
value… future design work, not modelled"). `Cell::blank/is_blank/new/default` are
the only API. Nothing yet depends on multi-scalar content, so the debt is
**contained but load-bearing**: every future buffer/diff/paint path will touch
`Cell`, making `char` the highest-entrenchment-risk item in the Phase-1 tree.

### D.1 Why `char` fails — precise inventory

`char` is a Unicode scalar value (U+0000–U+D7FF, U+E000–U+10FFFF), explicitly **not**
a grapheme cluster, not a glyph, not a cell. Failure modes, each independently fatal:

1. **Combining marks become detached cells.** `e` + U+0301 (combining acute) is one
   user-perceived character but two `char`s → two `Cell`s → cursor/diff/paint
   address a fragment; rendering splits base from mark.
2. **Extended grapheme clusters need many scalars.** Family emoji (ZWJ sequences),
   flags (regional-indicator pairs), skin-tone modifiers, keycaps (base + VS16 +
   enclosing) are 2–7+ scalars per cluster. `char`-per-cell explodes one visual
   unit across many cells with independently styleable halves — torn emoji.
3. **Emoji presentation selectors.** U+FE0F (VS16) has zero standalone meaning;
   stored alone in a cell it renders as invisible/tofu while consuming a cell.
4. **Double-width (East Asian / many emoji) needs 2 columns for 1 cluster.**
   A `char` cell cannot express "this cluster occupies this cell + the next";
   without a wide-continuation convention, CJK/emoji either overlap neighbours or
   leave phantom blanks, and diff/width math disagrees with the terminal.
5. **Zero-width characters consume cells they must not.** ZWJ (U+200D), ZWNJ,
   word-joiner, control/format characters (e.g. U+200B ZWSP) stored as `symbol`
   occupy a visible cell each while rendering as nothing — phantom spacing and
   cursor drift.
6. **Clipping splits clusters.** Truncation at a cell boundary with scalar cells
   cuts mid-cluster (base kept, VS16/ZWJ tail dropped or vice versa) → mojibake
   halves. RFC-3 Q8 correctly flags this as needing an authority; `char` cells
   make every truncation site a corruption site.
7. **Width disagreement.** `char::len_utf8` / scalar count ≠ display width.
   Layout measuring scalars while the terminal measures cluster-cells guarantees
   misalignment on any non-ASCII content; style's R2 width-neutrality holds only
   because style never measures — the moment the renderer does, `char` lies.
8. **Precomposed vs decomposed inequivalence.** `é` (U+00E9, 1 scalar) vs
   `e`+U+0301 (2 scalars) are canonically equivalent but occupy 1 vs 2 cells —
   buffer equality, diffing, and snapshots become normalization-sensitive.

Net: `char` can represent ASCII exactly; for everything else it confuses encoding
unit with addressing unit. It is adequate **only** as a Phase-1 placeholder behind
the current "no buffer/diff yet" firewall (`renderer.rs` docs state this plainly —
to their credit).

### D.2 Future rendering/Unicode RFC contract (requirements on the orchestrator)

The future RFC (owner: renderer/layout track; **timing: no later than Phase 3**,
before any buffer, diff, widget, or snapshot-golden code treats `Cell.symbol` as
settled) MUST decide, at minimum:

1. **Cell content type:** grapheme-cluster string (`heapless`/`ArrayString`-style
   small-string vs `String` alloc policy — allocation-map consequences per
   Component-Feasibility pattern) vs scalar + width-tag vs segment-indexed buffer.
2. **Segmentation authority:** which crate owns UAX #29 boundaries (and whether a
   segmentation dep is admitted with an ARCHITECTURE §6.6 justification — the
   serde-wording precedent in RFC-3 §4 applies: state cost honestly, never as a
   no-external-deps precedent).
3. **Width authority:** UAX #11 / `wcwidth`-table source, version-pinning policy,
   wide-continuation representation, zero-width elision rule, and the terminal-
   disagreement fallback (referenced already by RFC-3 Q8 (a)/(b) constraints).
4. **Style-span alignment rule:** spans align to cluster boundaries from that
   authority; splitting a cluster across attributes is forbidden (RFC-3 Q8 stance
   stands — renderer must not assign the duty back to style without amending RFC-3).
5. **Normalization policy:** NFC-in/NFC-out vs boundary-only (decides buffer
   equality + golden determinism; item D.1.8 above).
6. **Migration path for `Cell.symbol`:** rename/retype with deprecation or
   coordinated break (it is pre-`0.1.0`, but test doubles + examples may already
   name it — grep-gated).
7. **Risk statement — char entrenchment:** every frame committed with scalar cells
   bakes scalar-indexed diff, damage, and golden files that a later cluster model
   must rewrite. The firewall holding today is "renderer.rs implements no diffing"
   — the RFC must land **before** that sentence is deleted. If widget work
   (RFC-1 Q4 alpha list/table/input) needs non-ASCII content first, it proceeds
   behind an explicitly ASCII-scoped test contract, never by extending `char`.

### D.3 Lifecycle implications of Unicode

None in Phase 1 (no lifecycle transition touches text). The only lifecycle-adjacent
rule: truncation/clipping authority belongs to layout/renderer, never to the loop;
mid-frame text-encoding changes are not a capability event (no version bump kind
exists for them — correctly, there is no such signal).

### D.4 Immediate containment change — proposed ONLY with compat/scope analysis

**Recommendation: do NOT change `Cell` in Phase 1.7 (document-and-defer).**
The module docs already contain the honest limitation; a retype now buys risk
without consumers.

If the orchestrator nevertheless wants a compile-time tripwire before Phase 2,
the **only** containment change I endorse, verbatim:

```rust
// On `Cell::new` (and only there):
assert!(!symbol.is_control(), "Cell holds visible content; UAX#29/UAX#11 RFC pending");
```

- Compat: `const fn` **cannot** host `assert!` on stable for non-const-panic paths
  in the current toolchain posture — `new` would lose `const`, breaking
  `Cell::blank`-in-const contexts downstream. Alternative `debug_assert!` keeps
  `const` but fires only in debug builds, i.e. exactly where golden tests run —
  acceptable but weak. Either variant changes the public `const` contract:
  needs an MSRV + semver note even pre-`0.1.0`.
- Scope: does not fix any D.1 item; merely converts silent zero-width/control
  storage into loud failure. ZWJ/VS16 are not `is_control`, so the tripwire is
  porous by construction.
- Verdict on the tripwire: **net-negative in Phase 1.7** (contract churn for a
  porous guard). Prefer the Phase-3 RFC requirement (§D.2) + keeping the
  "implements no diffing" firewall until that RFC lands. Recorded here so the
  "do nothing" is an explicit decision, never silence.

---

## Acceptance questions (for the orchestrator / RFC-3 owner)

1. Do you accept amendments P1–P3 verbatim (consumption rule restatement,
   describe-still-runs fix, combined frame-commit invariant)?
2. Do you accept the §10 snapshot-policy endorsement as the Q12c/Q8 harness
   (exhaustive islands §C.8 + properties), with pair-graduation by amendment?
3. Do you accept the Unicode disposition: `Cell.symbol: char` stays in Phase 1.7
   with its honest docs, and a rendering/Unicode RFC owning §D.2 items 1–7 lands
   no later than Phase 3, before any diff/buffer/widget code settles `Cell`?
4. `ThemeKind` Q2: confirm documentary-only for Phase 1 (no auto-warn behaviour)?
5. Extension-missing-key fallback (Text+debug-marker vs fail-loud magenta, Q3):
   confirm current stance holds for alpha, or direct an amendment?
6. Quantization metric (Q5 Euclidean vs perceptual): confirm deferral with snapshot
   evidence, or direct a choice now?

## Exact amendment proposals (apply to RFC-3; this review file is unchanged)

**P1 — §5 (after the aggregate paragraph), append:**
> Consumption rule (normative): every consumer takes its own domain struct
> (`ColorProfile` for style, `InputProfile` for input, `ScreenProfile` for
> renderer/lifecycle), never the `TerminalCapabilities` aggregate; the aggregate
> crosses only the terminal→loop and loop→frame seams (cf.
> `docs/research/CAPABILITY_MODEL_RESOLUTION.md` §4).

**P2 — §4 merge paragraph, append one sentence:**
> Per-frame `describe` still runs on every frame (descriptions are rebuilt
> regardless); the `(theme_version, color_version)` keys gate merge/quantize redo
> and layout/paint skipping only — never visitation.

**P3 — Q7 resolution, append one sentence + owner:**
> Combined frame-commit invariant (owned by the lifecycle RFC): a committed frame
> resolves all cells against exactly one start snapshot AND reaches paint only
> through error-free describe/layout; anything else retains the previous buffer
> and emits nothing.

No other RFC-3 text changes requested. Resolution §§12.5 deltas are P2/P3 restated;
no sibling-doc changes required.

---

## Handoff

- **To orchestrator:** this file (`docs/reviews/RFC3_TERMINAL_READINESS.md`) is the
  deliverable. Verdicts: domains/atomicity/theming ACCEPT (P1–P3 wording repairs);
  snapshot policy ENDORSED as written in Resolution §10; Unicode HOLD with
  document-and-defer + Phase-≤3 RFC requirement (§D.2). Acceptance questions §1–6
  are the decision gates; amendments P1–P3 are exact strings ready to apply.
- **To renderer/Unicode RFC author:** start from §D.1 (8 failure modes) and §D.2
  (7 required decisions + entrenchment risk + ASCII-scoped interim contract).
  Do not re-litigate snapshot-at-start or domain splits — §§A–B are settled here.
- **To lifecycle RFC author:** own the P3 combined invariant and queue mechanics;
  no state-machine change needed for capabilities (unknowns are ordinary inputs;
  input re-arm is backend-edge, never an effect payload).
- **To widgets RFC author:** Q4 graduation rule stands — bring visual evidence if a
  third variant pair needs exhaustive goldens; mono S+F coverage is already
  mandatory.
- Constraints: no production code touched, no deps added, branch unmoved.
