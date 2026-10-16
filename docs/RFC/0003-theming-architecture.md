# RFC-0003: Semantic Styling, Theme Tokens, Capability Fallbacks

- Status: Draft
- Author(s): Subagent 3 — Design System & Theming Architect (Jilva Phase 1.5 RFC sprint)
- Created: 2026-10-16
- Related: `docs/ARCHITECTURE.md` (§3.8, §8.9, §8.14), `docs/API_DESIGN.md` (§7),
  `docs/adr/0003-terminal-backend-strategy.md`,
  `docs/RFC/0001-framework-scope.md` (scope parent: amendment rule, Q12c, refusals),
  `docs/RFC/0002-component-model.md` (change classes, Context threading),
  `docs/research/ARCHITECTURE_FEASIBILITY.md` (Round-1 findings F3.1–F3.5)
- Lifecycle note: per `docs/RFC/README.md` this RFC stays **Draft** (min. 1 week
  Discussion before any decision) and must never read as Accepted.
- Round-2 note (2026-10-16): revised after cross-review of RFC-1, RFC-2, and
  the feasibility report. Resolutions and residual disagreements are recorded
  in §"Round-2 cross-review record"; unresolved items stay in Open questions.

## Summary

`jilva-rs` gets a **semantic theming system**: widgets and app code reference
strongly-typed semantic tokens (`Accent`, `Danger`, …) and state variants
(`Selected`, `Focused`, …), never raw ANSI values. A `Theme` maps tokens to
concrete attributes; a pure, capability-driven **resolution** step in
`jilva-core` turns tokens plus a plain-data `CapabilityProfile` into final
draw attributes, degrading automatically along the
truecolor → 256 → 16 → mono chain. Capability *detection* lives in
`jilva-terminal`; capability *profiles* and token *resolution* live in core as
data and pure functions. Style changes are paint-only by default and never
force re-layout unless a layout-affecting property changed. Animation gets
future-compatibility plus a reduced-motion signal only — no engine.

> All Rust sketches below are labeled **PROPOSED — NOT IMPLEMENTED**. They are
> illustrative, do not compile, and create no API commitment.

## Motivation

Three concrete pains this solves:

1. **App authors hardcode colors that break elsewhere.** A hand-picked RGB
   accent is unreadable on a light background, renders as grey mush on a
   16-color terminal, and disappears entirely under `NO_COLOR`. Without a
   semantic layer, every widget author re-solves degradation badly or not at all.
2. **Theming is all-or-nothing in most TUI frameworks.** Users who want "just
   change the accent" must fork an entire theme. Partial overrides with the
   default filling gaps make customization approachable (API_DESIGN.md §7).
3. **Style/layout coupling causes needless work.** If changing a color forces a
   full re-layout, interactive apps (filter-as-you-type, theme preview) pay
   layout cost for paint-only changes. The style boundary must state the
   invalidation rule up front (ARCHITECTURE.md §8.9).

Who feels it: widget authors (future `jilva-widgets`), app authors shipping to
heterogeneous terminals (dark/light, 16-color corporate boxes, `NO_COLOR`
pipelines), and accessibility-conscious users who need contrast guarantees.

## Proposal

### 1. Separation of five concerns (normative)

The design separates five things that other frameworks conflate:

| # | Concern | Lives in | Examples |
|---|---------|----------|----------|
| 1 | **Semantic style** (intent) | widget/app code, core types | `Accent`, `Muted`, `Danger` |
| 2 | **Visual attributes** (concrete) | resolved output, core types | `fg: Rgb(…), bold` |
| 3 | **Variants** (state overlays) | core resolution rules | `Selected`, `Focused`, `Disabled`, `Active` |
| 4 | **Layout constraints** (geometry) | layout protocol (§3.6), never style | width, padding, wrapping |
| 5 | **Capability resolution** (degradation) | core pure function + terminal-provided profile | truecolor → 256 → 16 → mono |

Rules:

- **R1.** Components reference semantic tokens + variants only (ARCHITECTURE.md
  §3.8 rule, RFC-1 Q9 permanent refusal). Raw ANSI/SGR values are unnameable
  in component APIs, and there is **no escape hatch** (RFC-1 matrix: raw-ANSI
  styling is Out — revisit needs ADR + RFC, not a feature flag). SGR bytes
  exist only inside the backend encoder, which receives fully-resolved
  attributes, never tokens.
- **R2.** Visual attributes carry **no geometry**. Adding `bold`, changing a
  color, or toggling `italic` never changes a cell's width. Style resolution
  output is fixed-width-neutral by construction.
- **R3.** Variants are **orthogonal overlays**, not tokens. A widget renders
  `Token::Text` in a set of co-present states (e.g. selected *and* focused);
  resolution composes base + overlays in documented precedence order (§3). This
  keeps the token set small and makes state styling uniform across widgets.
- **R4.** Capability resolution is a **pure function** of
  `(Theme, CapabilityProfile)` producing resolved attributes. It is the only
  place degradation happens, so it is snapshot-testable without a terminal.
- **R5 (invalidation).** A theme/style change is **paint-only** unless the
  changed property is on the layout-affecting list (§6). Because no token,
  attribute, or variant carries geometry, ordinary theme switches and runtime
  re-themes never invalidate layout.

### 2. Semantic token vocabulary (final set, justified)

Tokens are a **closed enum** (strongly typed, exhaustively matchable), not
strings. The final set is deliberately small (~20 tokens in 4 groups).
Anything not here is expressed as token + variant composition or a custom
extension token (§4).

```rust
// PROPOSED — NOT IMPLEMENTED (illustrative sketch, does not compile)
pub enum Token {
    // — Group A: surfaces (where ink sits) —
    Background,   // app canvas default
    Surface,      // raised panels, dialogs, cards
    Overlay,      // popups/menus above Surface
    Border,       // hairline borders and dividers
    // — Group B: text roles (what the ink says) —
    Text,         // primary body text
    Muted,        // secondary/de-emphasised text, placeholders
    Accent,       // brand/links/active affordances
    // — Group C: feedback (status with meaning) —
    Success,
    Warning,
    Danger,
    Info,
    // — Group D: selection-adjacent chrome that is NOT widget state —
    Selection,    // selected-row/cell fill (state overlay lives in Variant)
    Cursor,       // text-cursor / caret colour
    Highlight,    // search-match / mark highlight
}
```

**Justification for each inclusion/exclusion decision:**

- `Background / Surface / Overlay` (three tiers, not one): nested UI
  (dialog over panel over app) needs distinguishable depth steps. One
  background token forces widgets to invent ad-hoc lightening, which breaks
  under 16-color fallback. Three is the minimum that composes; more tiers
  would bloat every theme for no demonstrated need.
- `Border` separate from surfaces: borders must stay subtle at every tier
  (often a dimmed shade of the surface). Merging border into surface loses the
  contrast relationship themes need to preserve during quantization.
- `Text / Muted / Accent`: the minimum legible text system. `Muted` covers
  hints, placeholders, timestamps, disabled-adjacent text (with `Variant::Disabled`
  for the state itself). `Accent` is the single brand affordance; links and
  active items share it deliberately so a one-token override rebrands an app.
- `Success / Warning / Danger / Info` (all four): status colour without these
  four degrades into authors improvising with raw greens/reds — the exact
  breakage this RFC exists to prevent. `Info` is included (not merged into
  `Accent`) because info banners on an accented page must not look clickable.
- `Selection / Cursor / Highlight`: chrome that multiple widgets share (lists,
  tables, inputs, search). If each widget defined its own selection colour,
  multi-widget screens would clash. Centralising them guarantees coherence.
- **Excluded (deliberately):** `Focused` and `Disabled` are **variants, not
  tokens** (see §3) — they modify any token rather than naming a colour.
  `Selected` as a *fill* is `Token::Selection`, but the *state* is
  `Variant::Selected`; the fill token exists because selection backgrounds
  need theme-level contrast tuning against all four feedback colours.
  `Foreground` as a bare name is rejected in favour of `Text` (clearer role,
  matches "what the ink says" framing). No per-widget tokens (no
  `ButtonBackground`): widget-specific looks compose from shared tokens plus
  variants; per-widget tokens would make cross-widget coherence untestable.
- Size discipline: ~15 tokens keeps theme authoring feasible (a full custom
  theme is ~15 lines of overrides), keeps the quantization snapshot matrix
  small (tokens × tiers), and keeps the closed enum exhaustively matchable in
  user code that inspects themes.
- **Amendment-gated (RFC-1 Q14 "measure twice").** Token names are public API
  on the expensive-to-change list. Once this RFC is accepted, adding, removing,
  or renaming a `Token` variant requires an amendment RFC (per ARCHITECTURE.md
  §3.1, user-facing breaking changes need an RFC amendment even in `0.x`),
  because new variants break downstream exhaustive matches. The extension map
  (§4) is the pressure valve that lets this rule hold without freezing apps.
- **Portable artefact (RFC-1 Q8).** The token *names and semantics* (not the
  Rust enum syntax) are documented as portable spec for future sibling
  implementations (`jilva-go`, `jilva-ts` per ADR-0001). No API in this RFC is
  shaped for cross-language parity; conformance later is by shared token
  list, not code sharing.
- **Extension map = the Should-seam (RFC-1 matrix).** The feature matrix lists
  "token extension mechanism" as Should (designed-for: seam present at alpha,
  no further alpha API). `ExtensionMap` is that seam — nothing more is added
  at alpha.

### 3. Variants: state overlays orthogonal to tokens

```rust
// PROPOSED — NOT IMPLEMENTED (illustrative sketch, does not compile)
pub enum Variant {
    Normal,     // no overlay
    Hovered,    // mouse hover (no-op where mouse is unsupported)
    Active,     // pressed / in-progress interaction
    Selected,   // item is selected
    Focused,    // widget holds keyboard focus
    Disabled,   // non-interactive; must also gate events (RFC-2 concern)
}
```

Resolution composes `base(token)` with the overlays for **all co-present
states**, applied as sequential total transforms in ascending precedence
order: `Hovered < Active < Focused < Selected < Disabled` (position revised
in Round 2 — see §"Round-2 cross-review record"; this supersedes the Draft-1
single-winner rule). Each overlay is a *transform* (e.g. "swap fg/bg",
"add bold", "dim", "use Selection fill") rather than a full second palette,
keeping theme size linear. Composition rules:

- Overlays **stack**: a selected row in a focused list renders
  `Selected ∘ Focused` over the base, visibly distinct from the same row when
  the list is unfocused. Focus visibility is load-bearing (keyboard users must
  see where input goes; cf. §9), so `Selected + Focused` is the canonical
  stacking case alpha widgets (list, table, input — RFC-1 Q4) depend on.
- `Disabled` is **absorbing**: when present, only the `Disabled` transform
  applies. Rationale: a disabled widget refuses interaction (RFC-2 event
  gate), so selected/focused distinctions inside it are unactionable noise;
  one uniform dimmed look also bounds the snapshot matrix.
- Transforms are total functions over `StyleDef`, so composition order fully
  determines output: resolution stays deterministic and snapshot-testable.
  The matrix covers every token × every single overlay × the load-bearing
  pair `Selected + Focused` (§"Testing strategy"); further pairs resolve by
  the same rule and are covered by composition property tests, not exhaustive
  snapshots (residual scope contested — see open question 4).

`Disabled` has a semantic duty beyond colour: it SHOULD coincide with the
widget refusing interaction (RFC-2 defines the event gate; this RFC defines
only the visual: dimmed text + no accent, never colour-alone — see §9).

### 4. Theme model

```rust
// PROPOSED — NOT IMPLEMENTED (illustrative sketch, does not compile)
pub struct Theme {
    pub name: &'static str,          // e.g. "jilva-dark"
    pub kind: ThemeKind,             // Dark | Light | Mono (advisory only)
    pub tokens: TokenMap,            // Token -> StyleDef
    pub variants: VariantMap,        // Variant -> OverlayDef
    pub extensions: ExtensionMap,    // app custom tokens (see below)
}
pub struct StyleDef {
    pub fg: ColorSpec,               // authored at truecolor fidelity
    pub bg: Option<ColorSpec>,       // None = inherit surface
    pub attrs: AttrSet,              // bold/italic/underline/… (see §7)
}
pub enum ColorSpec { Rgb(u8, u8, u8), Ansi16(Ansi16), Inherit }
```

- **Built-ins.** Exactly two curated themes ship in the default surface:
  `jilva-dark` (default) and `jilva-light`. A third pseudo-theme,
  `jilva-mono`, is not authored — it is the *automatic* mono resolution of the
  active theme (§5) and is always available. Two built-ins bound the snapshot
  matrix; more built-ins are a post-`0.1.0` RFC decision.
- **Named themes.** Third-party themes are values, not registrations: there is
  no global theme registry (ARCHITECTURE.md principle 3 — no ambient global
  state). Apps pass the `Theme` (or a `ThemeHandle`, §6) explicitly.
- **Partial overrides + default fills gaps.** A custom theme is expressed as a
  *delta* over a base:
  ```rust
  // PROPOSED — NOT IMPLEMENTED (illustrative sketch, does not compile)
  let my_theme = Theme::delta(&Theme::jilva_dark(), ThemeDelta {
      tokens: vec![(Token::Accent, StyleDef { fg: ColorSpec::Rgb(0x7c, 0x5c, 0xff), ..keep() })],
      ..ThemeDelta::empty()
  });
  ```
  Any token not overridden resolves from the base. There is no all-or-nothing
  authoring. Deltas compose (`delta over delta`), giving **inheritance by
  composition** rather than class hierarchies.
- **Merge semantics (normative; answers feasibility F3.5).** Resolution order
  per token is: app delta(s), innermost-first → base theme → built-in default
  (`jilva-dark` fills any gap the base leaves). **Per-token last-writer-wins**:
  each token resolves independently, so overriding `Accent` never perturbs any
  other token. Variant overlays apply after the token merge; capability
  quantization applies last. Token-merge output is **capability-independent**:
  the merged theme is cached per `theme_version`; only quantization re-runs
  per `color_version`. (Pipeline note: per-frame `describe` still runs on
  every frame — descriptions are rebuilt regardless; the
  `(theme_version, color_version)` keys gate merge/quantize redo and
  layout/paint skipping only, never visitation; see RFC-2 pipeline.)
  Testable form: a
  one-token delta must leave every other token's snapshot byte-identical to
  the base (§"Testing strategy").
- **Strongly typed + custom tokens.** The closed `Token` enum covers the
  framework; apps needing more (e.g. `DiffAdd`, `DiffRemove`) declare them in
  `extensions: ExtensionMap` keyed by a `&'static str` with a `StyleDef`
  value. Extension keys have **no fallback across themes**: if a widget reads a
  missing extension key, resolution returns the `Text` token style plus a
  debug-visible marker (never a panic, never invisible text). This keeps custom
  tokens second-class by design — promotion to first-class is an RFC decision.
- **Serialization: none in core Phase 1; additive-only thereafter (answers
  feasibility F3.3).** No `serde` dependency in `jilva-core` — minimal and
  justified per ARCHITECTURE.md §6.6: `serde` is excluded for its
  compile-time and MSRV-surface cost against a style system that does not
  need it (not as a precedent that core takes no external dependencies —
  future segmentation/width crates remain on the table with their own §6.6
  justifications). This matches the surveyed posture
  (Ratatui gates style serde behind a feature; iocraft keeps `serde`
  optional): if theme files (TOML/JSON) are ever supported, parsing lives in
  app code or a future crate, and any `serde` impl on theme types must be an
  additive, non-default feature with an ARCHITECTURE.md §6.6 justification —
  never a default. Theme-file demand is revisited only with evidence, as a
  short amendment RFC.
- **`palette`-crate interop: same posture (answers feasibility F3.4).**
  Phase 1 hand-rolls nearest-match quantization (zero-dep; the math is small
  and auditable). `palette` (MIT-or-Apache-2.0 → license-gate-passing) may be
  adopted later only via RFC with §6.6 justification, as an additive option —
  never as a core dependency.
- **Dithering policy (RFC-1 matrix Should row, RFC-3 detail): no dithering.**
  Cells are atomic (one grapheme + attributes per cell); there is no sub-cell
  addressable unit to dither *with*. Approximation means quantization only
  (§5). Ordered-diffusion or half-block tricks belong to the postponed
  images/graphics topic (RFC-1 Q5), not to text styling.
- **Runtime switching + consequences.** Switching the active theme bumps a
  version counter (§6) and issues a **paint-only invalidation** of the whole
  frame; layout, component identity, and focus are untouched. Because
  resolution output carries no geometry (R2), a theme switch provably cannot
  move anything — the renderer diffs cell attributes only. Consequence for
  authors: theme previews are cheap (no re-layout); consequence for the
  renderer RFC: the diff layer must already handle attribute-only churn
  efficiently (full-screen attribute churn is the worst case, §10).

### 5. Terminal compatibility tiers and fallback strategy

Capability detection lives in `jilva-terminal`; the profile crossing into core
is plain data (ARCHITECTURE.md §6.2, ADR-0003):

```rust
// PROPOSED — NOT IMPLEMENTED (illustrative sketch, does not compile)
pub struct CapabilityProfile {
    pub color: ColorTier,        // Truecolor | Palette256 | Ansi16 | Mono
    pub attrs: AttrSupport,      // per-attribute flags: bold/italic/underline/…
    pub background: BackgroundHint, // Dark | Light | Unknown
    pub reduced_motion: bool,   // user/OS prefers-reduced-motion signal
}
pub enum ColorTier { Truecolor, Palette256, Ansi16, Mono }
```

> Phase-1.6 domain model (resolution §3): the sketch above is the
> **color projection** of a typed-domain hybrid — `ColorProfile` (this
> struct, consumed by style), with sibling `InputProfile` (key/mouse
> encodings, consumed by input decoding) and `ScreenProfile` (size,
> resize semantics, consumed by renderer/lifecycle) named as domains
> owned by the input/lifecycle RFCs. A thin `TerminalCapabilities`
> aggregate (domains by value + snapshot id + per-domain versions)
> travels only the terminal→loop and loop→frame seams; every other
> module takes its own domain, never the aggregate. Unknown/degraded
> values are explicit everywhere; guessing is forbidden.

Consumption rule (normative): every consumer takes its own domain struct
(`ColorProfile` for style, `InputProfile` for input, `ScreenProfile` for
renderer/lifecycle), never the `TerminalCapabilities` aggregate; the
aggregate crosses only the terminal→loop and loop→frame seams (cf.
`docs/research/CAPABILITY_MODEL_RESOLUTION.md` §4).

The `CapabilityProfile` shape itself is on RFC-1 Q14's "measure twice" list
(profile shape threaded into render/style is expensive to change): fields may
be *added* with defaults, but removing or retyping a field requires an
amendment RFC.

**Conservative default:** `Ansi16` — not `Mono`. `Mono` applies only on
positive evidence (detection or explicit app choice) or `NO_COLOR`; the
default assumes the ubiquitous 16-colour floor so capable terminals are
never flattened by caution.

**Fallback chain (normative):** every authored `Rgb` value quantizes
deterministically down the chain Truecolor → 256 → 16 → Mono. Quantization
tables live in core as data (they are math, not detection). Rules:

1. **Truecolor:** authored values pass through untouched.
2. **256:** nearest-palette mapping by a documented metric (default: minimum
   perceptual distance; the exact metric — Euclidean RGB vs. perceptual — is
   open question 5, decided by snapshot comparison evidence), computed
   once per `(Theme, profile)` and cached (§10). The standard 6×6×6 cube +
   grayscale ramp is assumed; terminals with non-standard 256 palettes get
   "close enough" colour (documented limitation, not an error).
3. **16:** map to the nearest base/bright colour *by role, not just distance*:
   `Danger` MUST land on a red-family slot, `Warning` on yellow, `Success` on
   green, `Info`/`Accent` on blue/cyan, even when a naive distance metric
   would pick otherwise. Meaning survives; hue accuracy does not. (Why the
   asymmetry: 256 slots are dense enough that distance preserves meaning;
   16 slots are so coarse that distance can land on a wrong-meaning colour,
   so roles override.)
4. **Mono:** all colour is dropped; meaning is carried **only** by attributes
   and symbols — bold/underline/reverse for emphasis, plus text-level cues
   (`!`, `?`, `[x]`) which are widget content, not style. A theme is shippable
   only if its mono snapshots keep every feedback token distinguishable from
   body text (tested, §11).
5. **`NO_COLOR` (env, [no-color.org](https://no-color.org)):** detection
   reports `Mono` with all decorative attributes stripped to bold-off/plain
   (the `NO_COLOR` convention suppresses colour; structural reverse-video for
   selection is retained since it is functional, documented in the theme
   docs). Jilva policy: command-line-present `NO_COLOR` (any value including
   empty) disables colour; attribute stripping follows the per-attribute
   table, and reverse-video retention is the documented exception.

**Background-detection uncertainty:** detection consults, in order,
explicit app override → `COLORFGBG`/`TERM`-derived hints → OS query where
available → `Unknown`. When `Unknown`, the framework does **not** guess: it
keeps the app-selected base theme (`jilva-dark` default) and relies on the
default theme being engineered for dual-background legibility (mid-contrast
accents readable on both black and white, no pure-black/white text). Auto-
switching dark/light on an uncertain signal is rejected — a wrong guess
inverts readability, while a conservative default merely looks slightly off.
Apps may expose a manual toggle; that toggle is a paint-only theme switch (§4).

**Unsupported attributes:** each attribute degrades independently per the
`AttrSupport` flags: missing `italic` → no-italic (never simulated with
colours, which would corrupt the palette); missing `strikethrough` →
`crossed-out` content gets a text-level fallback decided by the widget
(e.g. `~like this~` — widget content, out of scope here); missing
`underline-styles/curly` → plain underline → none, in that order; `dim` where
unsupported → map to the `Muted` token colour instead of an attribute
(`Muted` is resolved before any dim-substitution runs, so the fallback is
total; non-text elements are unaffected by this substitution).
Bold-as-bright (terminals rendering bold as bright colours) is neutralised by
never relying on bold for meaning — bold is emphasis only, always paired with
a token or variant that already carries the meaning.

**Contrast/readability:** the default themes target WCAG-AA-inspired ratios
(≥ 4.5:1 body text, ≥ 3:1 large/chrome elements) *at the truecolor tier*,
verified by an automated check in the test suite (§11) that runs the full
`BackgroundHint` matrix — `Dark`, `Light`, and `Unknown` (for `Unknown`,
assert against *both* dark and light surfaces: the dual-legibility rule,
no auto-switch to hide behind). Lower tiers cannot
promise ratios (16 colours are what they are); the guarantee there is weaker
but still testable: every feedback token MUST differ from `Text`/`Background`
by at least one attribute-or-colour step at every tier (no silent merges).

**Small terminals:** out of scope for theming (layout/RFC territory). The only
style rule: no token or variant may assume a minimum width (no "selection
adds brackets" tricks at the style layer — adornments are widget content and
subject to layout clipping).

### 6. Style vs layout: the invalidation contract

**Normative list — layout-affecting properties:** none in this RFC. The style
system defines zero layout-affecting properties in Phase 1. (A future RFC
adding e.g. configurable border glyphs or padding tokens would extend this
list explicitly, with migration notes.)

**Who decides style-vs-layout (resolved with RFC-2, Round 2).** Classification
authority follows ownership: the style module classifies its own changes —
every theme/token/variant/capability change in Phase 1 reports **paint-only** —
and each `update` returns that change class per RFC-2 §5; the loop takes the
maximum over the batch. The loop never second-guesses a paint claim at
runtime; instead RFC-2's debug-build damage-vs-diff assertions catch
under-reporting (a style change that actually moved cells fails the test
double, it does not silently misrender). So: RFC-3 owns the *classification*
(style changes are paint), RFC-2 owns the *mechanism* (class plumbing, max
rule, debug verification). No disagreement remains on authority.

**Terminology map (RFC-1 Q13 ↔ RFC-2 §5).** Both RFCs now use
none / repaint / layout (RFC-2 §5 adopts RFC-1 Q13 verbatim, with `none`
as a first-class return value). Mount/unmount, key-set change, and type
replacement are identity-mechanism outputs that *produce* layout+repaint
work — not a fourth invalidation kind. Theme switching is `repaint`
in both vocabularies, full stop.

**Theme propagation (shared question B, refined).** The active theme travels
as part of RFC-2's explicit `&Context` environment threading (never a global —
RFC-1 Q6(d) refusal), alongside the capability profile. Switching bumps a
`theme_version: u64`; frames record the version painted. Mismatch on an
otherwise clean tree → re-resolve attributes and repaint, skip
measure/arrange. Propagation is thus a version bump plus paint-only damage;
layout is skipped and no dedicated propagation walk exists — while the
normal per-frame `describe` still runs (versions gate caches and skipping,
never visitation; see RFC-2 pipeline). Capability changes travel
the same path with a `profile_version` counter.

### 7. Visual attributes: the closed set

Authored attributes (`AttrSet`) are a fixed bitflag set: `bold`, `dim`,
`italic`, `underline`, `reverse`, `strikethrough`. No curly/colour-underline
authoring in Phase 1 (resolution may *emit* them as progressive enhancement,
but themes cannot require them). There is deliberately **no `blink`**: it is
refused at authoring time with a loud error (open question 10, resolved —
no legacy users exist to migrate, and no theme may depend on blinking).

### 8. Animation: future-compat + reduced motion, no engine

This RFC defines no timers, no keyframes, no interpolation — the future
`jilva-motion` crate owns that seam (ARCHITECTURE.md §4.2). Theming's duties
are two narrow hooks:

1. `CapabilityProfile::reduced_motion` (sourced from OS settings / env where
   detectable, defaulting to false) is threaded into resolution so that when
   `jilva-motion` exists it can query "should this transition play or snap".
2. Every style decision in this RFC is static-safe: nothing about tokens,
   variants, or fallbacks assumes animation. A theme renders identically frame
   1 and frame N. Any future motion design MUST degrade to the static resolved
   frame under `reduced_motion` — recorded here as a constraint on that future
   RFC, not as functionality here.

### 9. Accessibility

- Colour is never the sole carrier of meaning at any tier: feedback tokens
  pair with text labels/icons (widget content); `Disabled` pairs dimming with
  interaction gating; focus is indicated by reverse-video or underline, never
  hue shift alone. (Mono-tier survival, §5, is the enforcing test.)
- Default-theme contrast checked automatically (§11); custom themes are *not*
  checked at runtime (no global validator, no panics) but the docs provide the
  check as a test helper apps can run on their themes.
- `NO_COLOR` is honoured as a first-class tier, not an afterthought.
- Reduced-motion signal plumbed now (§8) so the future motion crate cannot
  plead missing context.

## Prior art

- **Ratatui** (`Style { fg, bg, underline_color, add/sub_modifier }`,
  `Color::Rgb/Indexed/Named`, `Stylize` shorthands): borrows — the
  fg/bg/modifier triple maps cleanly onto our `StyleDef`; the named+indexed+
  RGB colour ladder prefigures our quantization chain. Rejects — Ratatui styles
  are ad-hoc per-span with no semantic token layer and no built-in tier
  degradation; conjunction (`add_modifier`/`sub_modifier` stacking) is more
  expressive than our single-overlay rule, but we trade that expressiveness
  for snapshot-testability.
- **Cursive / Bubble Tea / Textual:** Cursive's theme TOML ([TOML theme docs](https://docs.rs/cursive/latest/cursive/theme/index.html), stringly-typed named colours)
  supports the "named theme + partial override" ergonomics we want, but as
  stringly-typed config; Textual's CSS-like [design tokens](https://textual.textualize.io/guide/design/) + [`watch`-driven](https://textual.textualize.io/guide/reactivity/)
  reactive updates inform our token vocabulary and version-bump propagation,
  while its CSS cascade is explicitly rejected (no specificity wars in a
  cell grid; deltas compose by ordered override, last-writer-wins, fully
  documented).
- **Terminal detection reality** (`COLORTERM=truecolor/24bit`, `TERM=*256color`,
  `NO_COLOR`, `COLORFGBG`, SSH-forwarding loss): borrowed as the detection
  ladder in `jilva-terminal`; this RFC's contribution is making `Unknown`
  background and `NO_COLOR` first-class resolution inputs rather than edge
  cases, motivated by real-world reports (e.g. truecolor lost over SSH when
  `COLORTERM` is not forwarded).
- **ratatui-themes crate ecosystem:** validates demand for curated named
  themes with light/dark awareness and serde-optional integration — we adopt
  the named-theme + light/dark-kind shape but defer serialization (§4).

## Non-goals

- No animation or transition engine (future `jilva-motion`).
- No theme file format, no `serde` integration, no theme discovery/loading
  from disk in Phase 1.
- No per-widget token namespaces, no CSS cascade/specificity, no runtime
  contrast enforcement on custom themes.
- No layout-affecting style properties; no border-glyph or spacing theming.
- No new built-in themes beyond `jilva-dark` / `jilva-light` (+ automatic mono).
- No code, no API freeze: exact names and shapes remain sketch-grade until
  implementation RFCs.

## Alternatives considered

1. **Stringly-typed tokens (`theme["accent"]`).** Maximum flexibility, dynamic
   theme loading, trivial serialization. Rejected: typos become silent
   unstyled widgets, exhaustive matching is impossible, and every lookup pays
   hashing cost in the hot path. The closed enum + extension map gives 95% of
   the flexibility with compile-time safety for the framework set.
2. **Full CSS-like cascade with specificity.** Familiar to web developers,
   powerful overrides. Rejected: specificity resolution is notoriously hard to
   predict and snapshot-test; in a cell grid with ~15 tokens it is pure
   overhead. Ordered deltas (last-writer-wins) cover real use cases
   (rebrand, dark/light, a11y high-contrast delta) with trivial mental model.
3. **Resolution in the backend (`jilva-terminal` sees tokens).** Would let
   backends "optimise" output per terminal. Rejected (ADR-0003 decision 2):
   couples backend upgrades to app-facing style types and makes test doubles
   replicate app semantics. Resolution stays a pure core function; the backend
   receives fully-resolved attributes only. This is the decision behind the
   answer to shared question D (§"Shared questions" below).
4. **Guessing light/dark on unknown background.** Rejected (§5): a wrong guess
   is catastrophic (invisible text), a conservative default is merely
   suboptimal. Explicitness beats cleverness for readability.

## Open questions

1. Exact default palette values (the actual RGB numbers for `jilva-dark` /
   `jilva-light`)? Deferred to implementation with contrast-test evidence; do
   we need a design-review step for taste ("beauty as a feature") beyond the
   numeric checks?
2. Should `ThemeKind` (Dark/Light/Mono advisory) drive any automatic behaviour
   (e.g. warn when a dark theme is used with a `Light` background hint), or
   stay purely documentary?
3. Extension-token fallback: is `Text`-style-plus-debug-marker the right
   missing-key behaviour, or should missing keys resolve to a loud,
   impossible-to-miss magenta (fail-loud in dev, configurable in release)?
4. ~~Single-winner overlays~~ **RESOLVED in Round 2 (position taken):**
   overlays stack in precedence order with `Disabled` absorbing (§3). The
   load-bearing evidence is RFC-2's alpha widgets: a selected row in a focused
   vs. unfocused list must differ, which single-winner cannot express.
   **Residual escalated to maintainer:** is snapshot coverage of just the
   `Selected + Focused` pair (plus composition property tests for the rest)
   sufficient, or must every pair be golden-filed? If the widgets RFC shows a
   third pair matters visually (e.g. `Selected + Active` during drag), the
   matrix extends by amendment.
5. Quantization metric: Euclidean RGB distance vs. perceptual (Oklab-ish)
   distance for the 256-palette mapping — is the extra math (still zero-dep,
   hand-rolled) worth it, or is nearest-RGB fine in practice? Needs snapshot
   comparison evidence.
6. **(Shared B)** Is a single global `theme_version` counter sufficient, or do
   large apps need scoped themes (subtree with its own theme)? Scoped themes
   are currently NOT proposed; if RFC-2 needs dialog-scoped theming, this RFC
   must be amended.
7. **(Shared D)** Capability profile refresh cadence: RESOLVED — detection
   once at startup, cached; event-driven refresh at frame boundaries with
   **snapshot-at-start** semantics (a frame completes against its start
   snapshot; changes land on the next boundary — never a torn frame).
   Queue mechanics owned by the lifecycle RFC. Profile shape follows the
   typed-domain hybrid (§5): detect in terminal, consume per-domain.
   Combined frame-commit invariant (owned by the lifecycle RFC): a committed
   frame resolves all cells against exactly one start snapshot AND reaches
   paint only through error-free describe/layout; anything else retains the
   previous buffer and emits nothing.
8. **(Shared I — Unicode-hard choices)** Style attaches attributes to grapheme
   clusters, never splits a cluster across differing attributes, and never
   measures width — but truncation of a styled span at a cell boundary (e.g.
   cutting an emoji + variation selector, or an underline spanning a
   double-width cell) needs an authority. Proposed: the width/segmentation
   table source is chosen by the layout/renderer RFCs; this RFC constrains only
   that (a) style resolution output is width-neutral, (b) attribute spans align
   to cluster boundaries supplied by that authority. Flagged as conflict-prone:
   if the renderer RFC assigns cluster-splitting duties to style, this RFC
   must be amended.
9. High-contrast / colorblind-safe built-in theme: third built-in or a
   documented delta recipe? (Suggest recipe first, promote on demand.)
10. `blink`: RESOLVED — refuse entirely (fail-loud on authoring). Rationale:
   no legacy users exist to migrate, and a degrade-shim would bless an
   attribute no theme may depend on. The closed attribute set (§7) simply
   has no `blink`; parsers reject it with a loud error pointing at
   emphasis alternatives.

## Drawbacks

- **Closed enum rigidity:** promoting a new first-class token is a breaking
  change (new variant breaks exhaustive matches downstream). Mitigated by the
  extension map and by keeping the set small; still a real semver cost.
- **Stacked overlays enlarge the test surface.** Singles × tokens plus the
  `Selected + Focused` pair is the bounded matrix; if widget evidence demands
  more snapshotted pairs (open question 4 residual), the matrix grows
  multiplicatively. The Disabled-absorbing rule and property-test coverage
  for the rest are the containment.
- **Dual-background-legible defaults** constrain palette beauty: colours that
  sing on dark backgrounds and stay legible on light ones are a narrower set.
  Accepted deliberately — correctness underneath, taste within constraints.
- **Resolution cache invalidation** adds state (theme + profile versions) to
  the render path; a bug here causes stale colours. Mitigated by version
  counters checked per frame (cheap integer compare, §10).
- **No serialization** will disappoint theme-file enthusiasts on day one;
  the stance is "deferred with a path", not "never".

## Testing strategy (across capability tiers)

**Answers RFC-1 Q8 (which harness asserts Q12c "distinguishable without
hue").** This section *is* that harness, named concretely: the tier snapshot
matrix (item 1) plus the mono distinguishability test (item 2) run headless
(test double injects `CapabilityProfile` values directly, no TTY), with golden
files living under `tests/` per the ARCHITECTURE.md §7 workspace contract
(`tests/` import the facade only — so the harness exercises resolution
exclusively through the public theme surface, which doubles as the RFC-1 Q11
"beautiful by default" enforcement: any taste change is a reviewed snapshot
diff). Items 2–4 are the direct operationalization of Q12c's "colour absence
never breaks information".

0. **Delta-merge test (F3.5 testability):** a one-token delta over a base must
   leave every other token's resolved snapshot byte-identical to the base, at
   every tier — proving "no all-or-nothing authoring" is mechanical, not
   aspirational.
1. **Tier snapshot matrix:** for each built-in theme × each `ColorTier`
   (4) × each `BackgroundHint` (3) × representative `AttrSupport` profiles
   (full, minimal, none): resolve every token × every variant and snapshot the
   resulting attributes. Golden files, headless, no TTY (test double injects
   `CapabilityProfile` values directly). Matrix also covers the load-bearing
   stacked pair `Selected + Focused` for every token; remaining combos are
   covered by composition property tests (stacking-order determinism), not
   exhaustive snapshots — see open question 4.
2. **Mono distinguishability test:** at `Mono`, assert every feedback token
   differs from `Text` by ≥ 1 attribute step; assert `Selected`/`Focused` differ
   from `Normal` by reverse-video or equivalent — including the stacked
   `Selected + Focused` pair explicitly (the a11y-critical combination;
   remaining pairs covered by composition property tests per open
   question 4).
3. **Contrast check (truecolor tier):** automated ratio assertions (≥ 4.5:1
   body, ≥ 3:1 chrome) for built-ins; same check exposed as a test helper for
   custom themes.
4. **`NO_COLOR` test:** profile with mono + stripped attrs; assert no colour
   escapes and selection remains visible via reverse-video.
5. **Unknown-background test:** with `Background: Unknown`, assert the active
   theme is unchanged (no auto-switch) and all tokens resolve against the
   theme's own surfaces.
6. **Invalidation test:** theme switch on a laid-out frame → assert layout
   cache untouched, paint damage covers the frame, component identity and
   focus preserved (RFC-2 test-double cooperation).
7. **Widget contract test (for widgets RFC):** assert no widget output contains
   raw ANSI/SGR values — only resolved attributes from the style module
   (enforced by type boundaries, verified by a lint/grep test).
8. **Per-attribute degradation tests:** profiles with each single attribute
   disabled; assert the documented per-attribute fallback (§5), especially
   `italic` (dropped, never colour-simulated) and `dim` (→ `Muted` colour).

## Performance implications

- Resolution is **cached per `(theme_version, profile_version)`**: steady
  state pays one integer-compare per frame; token+variant lookups are enum-
  indexed array accesses (O(1), no hashing) in the per-cell path.
- Quantization (nearest-palette search) runs at most once per theme/profile
  change (~15 tokens × 256 entries — trivial) and is memoized; never per cell.
- Extension-map lookups (string-keyed) are opt-in per widget and documented as
  slower; framework widgets never use them in hot paths.
- Full-screen theme-switch repaint is attribute-diff churn (worst case:
  every cell's attributes change, symbols identical). The renderer RFC must
  budget for this; style's contribution is guaranteeing symbols/positions are
  untouched so the diff layer can fast-path attribute-only runs.

## Shared-question ownership (for sprint handoff)

- **B — theme propagation without full rebuild:** owned here. Answer:
  version-counter + paint-only damage (§6). Needs RFC-2 (component damage
  plumbing) and renderer-RFC (attribute-only diff fast path) cooperation;
  flagged in open questions 6.
- **D — where capabilities enter render/style:** owned here. Answer: detection
  in `jilva-terminal` → plain-data `CapabilityProfile` (defined in core) →
  pure resolution in core style module → resolved attributes cross the backend
  seam. Backend never sees tokens (ADR-0003). Refresh cadence is open
  question 7, shared with the lifecycle RFC.
- **I — Unicode-hard choices:** style's stance recorded (§"Open questions" 8):
  style is width-neutral and cluster-aligned, width authority belongs to
  layout/renderer RFCs. Any assignment of segmentation duties to style is a
  conflict requiring amendment — flagged, not resolved unilaterally.

## Round-2 cross-review record (2026-10-16)

Responses to the Round-1 feasibility findings F3.1–F3.5 and to sibling-RFC
conflicts. Each item states **aligned / resolved / contested**.

- **F3.1 (tokens + tier chain + snapshots; no raw ANSI) — aligned, with one
  hardening.** Token vocabulary (§2), fallback chain (§5), and per-tier
  snapshots (§"Testing strategy") were already present; R1 now cites RFC-1 Q9
  to make the raw-ANSI ban a *permanent refusal with no escape hatch*, not
  just a type-boundary rule. Comparison points (Textual predefined themes,
  Ratatui `Style`/`Stylize`) were already in Prior art.
- **F3.2 (profile as plain data; no backend types in theme APIs) — aligned.**
  §5 profile crosses the seam as data; the style module imports nothing from
  `jilva-terminal`; detection is cached at startup per ARCHITECTURE.md §8.14.
  Refresh cadence stays open question 7 (shared with the lifecycle RFC).
- **F3.3 (serde additive, never default) — aligned.** §4 now records the
  surveyed posture explicitly (Ratatui feature-gated style serde, iocraft
  optional serde): zero serde in core Phase 1; any future impl is additive,
  non-default, §6.6-justified.
- **F3.4 (palette interop additive) — aligned.** §4: hand-rolled quantization
  in Phase 1 (zero-dep); `palette` admissible license-wise (MIT-or-Apache-2.0)
  but needs an RFC + §6.6 justification; never a core dependency by default.
- **F3.5 (merge semantics must be testable) — resolved by addition.** Draft 1
  implied last-writer-wins only in Prior art; §4 now states normative merge
  order (app deltas innermost-first → base → built-in default, per-token
  last-writer-wins, overlays after merge, quantization last) with the
  delta-merge test (Testing item 0) as its mechanical proof.
- **RFC-1 amendment rule — resolved by addition.** §2 records token names as
  Q14 measure-twice API: post-acceptance changes need an amendment RFC even
  in `0.x`; the extension map is the pressure valve.
- **RFC-1 Q8 (which harness asserts Q12c) — resolved by addition.** The
  Testing strategy preamble names the harness (tier matrix + mono
  distinguishability, headless, golden files under `tests/`, facade-only) and
  ties it to Q11 taste-enforcement.
- **RFC-1 matrix dithering row — resolved by addition.** §4 states the
  RFC-3-owned policy: no dithering (atomic cells); quantization only.
- **RFC-1 Q8 portability + Should-seam — resolved by addition.** §2 notes
  token names/semantics as portable spec and `ExtensionMap` as the
  Should-seam fulfillment.
- **Layout-invalidation authority (RFC-2 §5) — resolved, no disagreement.**
  RFC-3 classifies (all Phase-1 style changes are paint-only); RFC-2's loop
  applies the max rule and its debug assertions police under-reporting (§6).
  Terminology mapped explicitly (both RFCs: none / repaint / layout;
  RFC-2 §5 adopts RFC-1 Q13 verbatim).
- **Single-overlay vs RFC-2 widget needs (my Q4) — CONTESTED, position
  taken.** Draft-1 single-winner is withdrawn: it cannot express
  selected-in-focused vs. selected-in-unfocused, which alpha list/table/input
  widgets require. Position: ordered stacking with Disabled absorbing (§3).
  Residual disagreement recorded in open question 4 (snapshot coverage scope
  for non-load-bearing pairs) — escalated to maintainer / widgets-RFC
  evidence. Impact on RFC-1 Q13: none (stacking changes do not change the
  change class; still paint-only).

## Amendment history

- 2026-10-16: Draft created (Phase 1.5 RFC sprint, Subagent 3).
- 2026-10-16 (Round 2): overlay stacking replaces single-winner (§3, Q4
  position); normative merge semantics (§4, F3.5); serde/palette additive
  posture + dithering policy (§4, F3.3/F3.4, RFC-1 matrix); amendment-gating
  + portability notes (§2); invalidation authority + terminology map + Context
  threading (§6); Q8 harness naming (Testing); R1 no-escape-hatch hardening;
  this cross-review record. Status remains Draft.
- 2026-10-16 (Phase 1.7): Review amendments — consumption rule (P1),
  describe-acknowledgment (P2), frame-commit invariant (P3); domain model
  + Ansi16 default + NO_COLOR policy + mono S+F + dim order + metric-to-Q5
  + contrast matrix + links + asymmetry + blink refusal + serde wording
  (Phase 1.6 items, traceability). Evidence:
  `docs/reviews/RFC3_TERMINAL_READINESS.md`. Status stays Draft.
- 2026-10-16 (Phase 1.6): Resolution amendments — typed-domain capability
  model (§5: color/input/screen domains + aggregate transport rule);
  Ansi16 conservative default; capability-independent merge + describe note
  (§4); snapshot-at-start refresh (Q7 resolved); NO_COLOR policy + citation;
  mono S+F explicit (§Testing); dim→Muted order; metric deferred to Q5;
  contrast matrix incl. Unknown; prior-art links; 16/256 asymmetry;
  blink refused (Q10 resolved, §7 updated); serde wording (§4). Evidence:
  `docs/research/CAPABILITY_MODEL_RESOLUTION.md`. Status stays Draft.
