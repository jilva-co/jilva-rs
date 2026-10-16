# [RFC-0003 Discussion] Semantic theming, token vocabulary, capability tiers

> Status of the proposal under discussion: **Draft**
> (`docs/RFC/0003-theming-architecture.md`, 2026-10-16, Round-2 revision).
> Per `docs/RFC/README.md`, no decision before 7 full days from this thread's
> posting timestamp. Nothing here is accepted.

## Problem

App authors hardcode colors that break on light backgrounds, 16-color
terminals, and `NO_COLOR`; theming is all-or-nothing in most TUI frameworks
("just change the accent" requires forking a theme); style/layout coupling
forces full re-layouts for paint-only changes. This RFC proposes semantic
theming: tokens + variants in code, pure capability-driven resolution in core,
automatic truecolor → 256 → 16 → mono degradation.

## Options considered

1. **Stringly-typed tokens** (`theme["accent"]`) — flexible, serializable;
   rejected: silent typos, no exhaustive matching, hot-path hashing. (Closed
   enum + extension map keeps ~95% of the flexibility.)
2. **Full CSS-like cascade** — familiar, powerful; rejected: specificity is
   unpredictable and unsnapshot-testable for ~15 tokens; ordered deltas with
   last-writer-wins cover rebrand/dark-light/a11y cases.
3. **Resolution in the backend** (backend sees tokens) — rejected per ADR-0003
   decision 2: couples backends to app-facing types, test doubles replicate app
   semantics. Backend receives fully-resolved attributes only.
4. **Guessing light/dark on unknown background** — rejected: a wrong guess is
   catastrophic, a conservative default merely suboptimal.
5. **Proposed**: ~15 closed-enum tokens in 4 groups; orthogonal stacked variant
   overlays (`Hovered < Active < Focused < Selected < Disabled`, Disabled
   absorbing); delta themes with per-token last-writer-wins merge; two built-ins
   (`jilva-dark` default + `jilva-light`, mono automatic); no serde / no
   `palette` in core Phase 1 (additive-only later); no dithering (atomic cells).

## Consequences

Closed-enum rigidity (new tokens are breaking — extension map is the pressure
valve); stacked overlays enlarge the test surface (bounded: singles × tokens +
`Selected + Focused` pair snapshotted, rest property-tested); dual-background
legibility constrains palette beauty; resolution caching adds version-counter
state; no theme files on day one (deferred with a path).

## Recommendation going in (provisional — maintainer decides after Discussion)

Accept the token set, stacking overlays, merge semantics, fallback chain, and
the testing harness (tier matrix + mono distinguishability + delta-merge test)
as the design-system bounding box.

## Evidence

- Feasibility F3.1–F3.5 all aligned/resolved (Round-2 record); raw-ANSI ban
  hardened to permanent refusal citing RFC-1 Q9.
- Invalidation authority resolved with RFC-2 (RFC-3 classifies paint-only,
  loop applies max rule, debug assertions police under-reporting).
- Single-winner overlays withdrawn with evidence: selected-in-focused vs
  selected-in-unfocused (alpha list/table/input) is inexpressible otherwise.

## Specific questions for reviewers

1. Token set (~15 in 4 groups): any token missing that alpha widgets
   (text/list/table/input/tabs/status bar) cannot live without? Any to cut?
2. Closed enum + amendment-gating (post-acceptance renames/additions need an
   amendment RFC): acceptable with the extension map as pressure valve?
3. Missing-extension-key behaviour: `Text`-style-plus-debug-marker (current) vs
   loud magenta fail-loud in dev?
4. Snapshot scope: singles × tokens + `Selected + Focused` pair, rest
   property-tested — sufficient, or must more pairs be golden-filed? Decided
   shape going in (Phase-1.6 §10): representative goldens + property tests,
   with explicitly exhaustive islands — tier×background matrix (~360 rows),
   mono distinguishability incl. S+F per token AT MONO (a11y-critical tier,
   add explicitly), NO_COLOR, Unknown-background, per-attribute singles on a
   4-token subset; all other pairs via determinism/order-equivalence/
   Disabled-absorption/totality properties; non-interference property proves
   input/screen never affect styling. Graduation rule: if widgets-RFC evidence
   shows a third pair matters visually, THAT pair graduates to the exhaustive
   islands by amendment. Confirm the islands + graduation rule?
5. Quantization metric for 256 mapping: Euclidean RGB vs perceptual (Oklab-ish)
   hand-rolled — worth it? Snapshot comparison evidence wanted. Additions:
   normative text should require only "nearest-palette by a documented metric"
   (metric choice stays in this question); record the 16-vs-256 asymmetry —
   distance fits 256, role-mapping wins at 16 (Danger→red-family even when
   naive distance disagrees). Confirm both wordings?
6. Scoped (subtree) themes: needed for dialogs at alpha, or is the single
   global `theme_version` sufficient?
7. Profile refresh cadence (shared with lifecycle RFC): event-driven at frame
   boundaries — lifecycle implementers, anything missing? Superseded in part
   by new Q12 below (snapshot-at-start answers the mid-frame policy); what
   remains for the lifecycle RFC is queue mechanics only — confirm that split?
8. Unicode truncation authority (shared with layout/renderer): style stays
   width-neutral and cluster-aligned — renderer side, can you own the
   width/segmentation table source? Extension-token note (new): extension keys
   are EXCLUDED from tier snapshots (Text-fallback rule tested once per tier,
   not per key) — confirm?
9. High-contrast/colorblind-safe theme: third built-in or documented delta
   recipe first?
10. `blink`: keep the parse-and-degrade-to-bold shim or refuse entirely?
    Phase-1.6 recommendation recorded: refuse-entirely/fail-loud (no legacy
    users exist) — RFC-3 owner's call; agree or defend the shim?
11. **Profile shape (new Phase-1.6 resolution — input/screen capability is
    currently homeless, R3-D2): typed-domain hybrid vs RFC-3's single
    visual-heavy `CapabilityProfile`?** Problem: RFC-3's profile covers color
    only; key-encoding ladder, mouse mode, alt-screen/viewport live nowhere,
    while `jilva-terminal`'s profile is three bools with no tier, no `Unknown`,
    no versions. Options: (A) one unified struct — fewest names, but style can
    observe mouse/keyboard, over-invalidation, junk-drawer future; (B) fully
    separate profiles — strongest separation, but "terminal truth at frame N"
    unnamed; (recommended hybrid) separate `Copy` domain structs in core —
    `ColorProfile { tier, attrs, background, no_color }` (the ONLY profile
    style may read), `InputProfile { key_encoding, mouse_mode }`,
    `ScreenProfile { alternate_screen, size, resize_pending }` — plus a thin
    by-value `TerminalCapabilities` aggregate (domains + `snapshot_id` +
    per-domain versions + `reduced_motion`) as transport + identity only.
    Rules: consumers take domain structs, never the aggregate (outside two
    seams — review flag otherwise); `Ansi16` conservative default (`Mono` only
    on positive evidence or `NO_COLOR`); `Legacy` key baseline (no `Unknown`
    variant — behavior identical); guessing forbidden in every domain;
    `reduced_motion` stays on the aggregate (no fourth domain for one bool);
    fields addable with defaults, removal/retype needs amendment (RFC-1 Q14
    carries over). Input/screen domains owned by input/lifecycle RFCs — this
    thread decides the style side + aggregate + consumption rule. Evidence:
    `CAPABILITY_MODEL_RESOLUTION.md` §§2–5 comparison matrix. Questions: does
    the consumption rule satisfy ADR-0003 reviewers? Is the `no_color`-alongside-
    `Mono` redundancy (audit + policy distinction) acceptable? Merge output
    stated capability-independent (cached per `theme_version`, quantization
    per `(theme_version, color_version)`) — confirm?
12. **Frame atomicity + invalidation (proposed DECIDED — `CAPABILITY_MODEL_
    RESOLUTION.md` §6–§7): snapshot-at-start?** Rule: a committed frame
    resolves all cells against exactly one snapshot; mid-frame changes queue
    for the next frame (one-frame latency, never tears). Versions:
    `color_version` → repaint-only re-resolve; `input_version` → decoder
    re-arm, no render damage; `screen_version` → layout+repaint via resize
    path; `reduced_motion` no-op in Phase 1; test-double mid-frame injection
    refused normatively. Rejected: complete-against-new-mid-flight (violates
    the invariant), abort-uncommitted (wasteful, livelock risk — lifecycle RFC
    keeps abort only for catastrophic backend loss). Question for lifecycle/
    renderer reviewers: is the policy + debug-assertion enforcement sufficient,
    needing only queue mechanics from the lifecycle RFC?
