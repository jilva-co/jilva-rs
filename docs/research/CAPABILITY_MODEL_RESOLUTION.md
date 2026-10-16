# Capability Model Resolution — Terminal Capabilities & Theming Signals

- Status: Resolution (design-only, Phase 1.6)
- Author: Subagent 3 — Terminal Capabilities & Theming System Architect
- Date: 2026-10-16
- Branch: `phase1.6-architecture-resolution` (design only — no production code changed)
- Inputs: `docs/RFC/0003-theming-architecture.md` (Draft, Round-2), `docs/ARCHITECTURE.md`
  (§3.5, §3.8, §3.10, §6, §8.6, §8.14), `docs/API_DESIGN.md` (§7, §9),
  `docs/adr/0003-terminal-backend-strategy.md`, `docs/research/ARCHITECTURE_FEASIBILITY.md`
  (R3-D2 input/screen homelessness, X-blockers), `crates/jilva-terminal/src/capabilities.rs`,
  `crates/jilva-core/src/style.rs`
- Scope: capability **model** (shapes, ownership, lifecycle, invalidation, test strategy).
  Token vocabulary, palettes, quantization metrics, and overlay composition stay owned by RFC-3.
  No code changes to production crates; no new dependencies (MIT-only constraint is moot —
  this resolution adds zero deps).

## 1. Problem statement

RFC-3 defines a visual-heavy profile (`CapabilityProfile { color, attrs, background,
reduced_motion }`, RFC-3 §5). RFC-1 Q12 promises a key-encoding ladder, gated mouse, and
alt-screen fallback; RFC-2 gates mouse handling on capability. None of the three drafts owns
**input capability** (keyboard ladder level, mouse mode) or **screen capability**
(alt-screen, viewport/size-source, resize signal path). Feasibility finding R3-D2 names this
exactly: input/screen capability is homeless. The current code confirms the gap from both
sides: `jilva-core::style::ColorDepth` knows only color tiers, and
`jilva-terminal::capabilities::CapabilityProfile { truecolor, mouse, alternate_screen }`
is three bools with no tier, no attrs, no background, no keyboard, no version, no
`Unknown` state.

This resolution closes the gap by deciding the profile shape strategy (Approach A vs B vs
hybrid), detection home, crate owners, consumers, unknown handling, timing/propagation,
atomicity, effects classification, headless injection, and degradation — then hands RFC-3 a
bounded amendment list (RFC-3 itself is NOT edited here).

## 2. Options compared

### 2.1 Definitions

- **Approach A — unified `TerminalCapabilities { color, input, screen }`.** One struct,
  one version counter, one threading path. Every consumer receives the whole terminal
  truth even if it needs one field.
- **Approach B — separate profiles per subsystem.** `ColorProfile` → style resolution;
  `InputProfile` → event decoding/dispatch; `ScreenProfile` → renderer/lifecycle. No
  aggregate type; each travels its own path with its own version.
- **Hybrid (typed-domain + aggregate snapshot, RECOMMENDED).** Each domain is a separate
  `Copy` plain-data struct defined in core (§4). A thin aggregate `TerminalCapabilities`
  composes them **by value** (plus a snapshot id) and is the unit threaded per frame.
  Consumers destructure only their domain. Detection constructs domains independently in
  `jilva-terminal` and assembles the snapshot at the frame edge.

### 2.2 Comparison matrix

| Criterion | A (unified) | B (separate) | Hybrid (recommended) |
|---|---|---|---|
| Separation of concerns | Weak. Style resolution can observe (and, by accident, branch on) mouse/keyboard fields; input dispatch can observe color tier. Type system enforces nothing; reviews must. | Strongest. Each consumer's signature names exactly its domain. Impossible to couple accidentally. | Strong. Function signatures take domain structs, not the aggregate. The aggregate is transport only; a lint/test rule ("resolution takes `&ColorProfile`, never `&TerminalCapabilities`") keeps it honest. |
| API stability (Q14 measure-twice cost) | Worst. Any new signal (e.g. sixel/images later, synchronized-output flag) changes THE profile type every consumer touches. Adding a field is source-stable but every consumer recompiles and re-reasons. | Best per domain. Keyboard-ladder evolution touches `InputProfile` only; color-tier evolution touches `ColorProfile` only. Unrelated consumers are unaffected. | Same as B for evolution (domains evolve independently); aggregate gains a field only when a new domain appears, which is rare and explicit. |
| Runtime updates | Coarse. Any signal change (resize, kitty-flag loss, `NO_COLOR` export) bumps one version → whole-frame re-resolve even when only one domain changed. | Fine-grained. Mouse-mode change re-drives input only; theme-relevant color change re-resolves style only. | Fine-grained where it matters: per-domain versions ride inside the snapshot; consumers compare only their domain version (§6). Aggregate id changes every snapshot for identity, domain versions gate work. |
| Extensibility | Add-field-to-one-struct. Cheap today, congested tomorrow: the struct becomes the junk drawer (color + keys + mouse + viewport + motion + …). Non-exhaustive helps but does not organize. | Add-a-domain or extend-a-domain. New concerns (e.g. future graphics tier) get their own struct with their own consumers — no existing signature changes. | Same as B, plus the aggregate documents the full terminal truth in one place for diagnostics and the test double. |
| Testability | Poor. Headless style tests must construct a full terminal truth (keyboard, viewport…) to test color degradation. Fixture noise; tests over-specify. | Excellent. Style tests construct `ColorProfile` only. Input tests construct `InputProfile` only. Minimal fixtures. | Same as B: tests construct domains directly. Aggregate constructors exist for frame-level tests only. |
| Dependency direction | One shared type risks gravity: every module imports it, and pressure grows to put helpers/methods on it (detection helpers creeping toward core). | Each domain imported only where needed. `InputProfile` need never appear in style code at all. | Same as B, enforced by rule: domains live in core as data-only; aggregate is data-only; ALL constructors-from-environment and detection live in `jilva-terminal`. Core methods on profiles are limited to `const` baselines (`basic()`, `full()`); no env sniffing, no I/O, no escapes. |
| Version tracking | Single counter conflates unrelated changes; "what changed" requires field diffing. | Per-domain counters answer "what changed" directly. | Per-domain counters + aggregate snapshot id: identity (`snapshot_id`) vs causality (`color_version`, `input_version`, `screen_version`) are distinct concepts with distinct fields. |
| Invalidation cost | Over-invalidation by construction: mouse-mode flap re-resolves every token; theme switch re-arms input decoding. Bounded but wasteful; worst case is full-screen attribute churn already budgeted by RFC-3 §10, made more frequent. | Minimal: only affected domain's cache invalidates. | Minimal, same as B: resolution cache keyed on `(theme_version, color_version)`; input decoder keyed on `input_version`; viewport/layout keyed on `screen_version`. |
| Platform compat | One profile shape must satisfy Unix + Windows + test double simultaneously; Windows-console gaps (no kitty, limited attrs) force `Unknown`/degraded values into fields style code must still read. | Each domain degrades on its own terms with its own `Unknown`/conservative default; Windows gaps in keyboard never touch style logic. | Same as B. Additionally the aggregate makes the platform matrix testable: one snapshot per (platform × terminal) row, domains asserted independently. |

### 2.3 Decision

**Adopt the typed-domain hybrid.** Rationale in one paragraph: A optimizes for fewer type
names and pays for it in coupling, over-invalidation, noisy fixtures, and a junk-drawer
future — the exact failure mode R3-D2 warns about, since stuffing `keys`/`mouse`/`viewport`
into RFC-3's style profile would let style code observe input state. B alone solves the
coupling but leaves "the terminal truth at frame N" unnamed, which the frame-atomicity rule
(§7), diagnostics, and the test double all need. The hybrid keeps B's per-domain signatures
(the load-bearing property) and adds a by-value aggregate as transport + identity only.

## 3. Recommended structure (design — not code)

Ownership and placement (§4 is normative for placement; field lists are proposed shapes for
the implementing RFC, not frozen API):

```rust
// PROPOSED — NOT IMPLEMENTED (shape sketch for the implementing RFC; does not compile
// against current tree, creates no API commitment)
// Crate: jilva-core. Plain data only: Copy, Eq/Hash, no I/O, no env, no escapes.

/// Color + attribute + background signals. The ONLY profile style resolution may read.
pub struct ColorProfile {
    pub tier: ColorTier,          // Truecolor | Palette256 | Ansi16 | Mono
    pub attrs: AttrSupport,       // per-attribute flags (bold/italic/underline/…)
    pub background: BackgroundHint, // Dark | Light | Unknown
    pub no_color: bool,           // NO_COLOR env present (see §9: convention vs policy)
}

/// Keyboard + mouse signals. The ONLY profile input decode/dispatch may read.
pub struct InputProfile {
    pub key_encoding: KeyEncoding, // Legacy | Enhanced(KittyFlagsNegotiated)
    pub mouse_mode: MouseMode,     // Off | Click | Drag | Full (negotiated, not requested)
}

/// Screen + viewport signals. Renderer/lifecycle read this; style never does.
pub struct ScreenProfile {
    pub alternate_screen: bool,  // negotiated, not assumed
    pub size: (u16, u16),        // columns, rows at snapshot time
    pub resize_pending: bool,    // size changed since last committed frame
}

/// Transport + identity only. Consumers destructure; they never branch on the whole.
pub struct TerminalCapabilities {
    pub color: ColorProfile,
    pub input: InputProfile,
    pub screen: ScreenProfile,
    pub snapshot_id: u64,        // unique per assembled snapshot (identity)
    pub color_version: u64,      // bumped only when `color` differs from last snapshot
    pub input_version: u64,
    pub screen_version: u64,
    pub reduced_motion: bool,    // cross-cutting user/OS signal; see §3.1
}
```

### 3.1 Notes on the shape

- `reduced_motion` placement: it is genuinely cross-cutting (future motion crate reads it;
  resolution threads it per RFC-3 §8). Keeping it on the aggregate rather than duplicating
  it per domain avoids version-bump ambiguity (a motion-preference change bumps nothing
  domain-specific; consumers of motion read the snapshot field). Alternative — a fourth
  `UserProfile` domain — is rejected as premature: one bool does not justify a domain, and
  the future motion RFC can promote it if more preference signals arrive.
- `no_color: bool` alongside `tier: Mono`: deliberately redundant. `tier` drives
  quantization; `no_color` records WHY (convention compliance audit + the
  bold-stripping/reverse-retaining policy distinction in §9). A snapshot with
  `tier == Mono && !no_color` (genuinely mono terminal) vs `no_color == true` must resolve
  identically in color but may differ in documented attribute policy — the flag makes that
  testable.
- `(u16, u16)` size lives in `ScreenProfile`, not in style or input: style is width-neutral
  (RFC-3 R2, Q8 stance), input needs coordinates for mouse decode but reads them from the
  event stream, and layout owns geometry. Size in the snapshot exists so the frame has one
  agreed viewport (§7).
- All structs `#[non_exhaustive]` at construction boundary OR `Copy` with `basic()`/`full()`
  constructors — pick one in implementation (recommendation: `#[non_exhaustive]` + `basic()`
  / `full()` const constructors mirroring the existing `capabilities.rs` pattern, so
  adding a field cannot silently break downstream struct literals while tests keep terse
  fixtures). Either way, "fields may be added with defaults; removing/retyping needs an
  amendment RFC" per RFC-1 Q14 carries over to every domain struct AND the aggregate.

## 4. Lifecycle and ownership

| Responsibility | Owner | Notes |
|---|---|---|
| Domain + aggregate **data-type definitions** | `jilva-core` (style-adjacent module for `ColorProfile`; events/input module for `InputProfile`; renderer/lifecycle module for `ScreenProfile`; aggregate alongside them) | Data-only, TTY-free, no env reads, no escapes. Core tests construct them freely. |
| **Detection** (env sniffing, `COLORTERM`/`TERM`/`COLORFGBG`/`NO_COLOR` ladder, OS queries, kitty-flag negotiation, alt-screen negotiation, Windows-console gaps) | `jilva-terminal` only | Backend reports; core never names a backend, env var, or console API (ARCHITECTURE §6.2, ADR-0003 decision 2). |
| **Assembly** (domains → snapshot + versions) | `jilva-terminal` (produces), frame loop (holds current) | Versions computed by comparison with last snapshot (§6), not by detection guessing. |
| **Consumption: color** | core style resolution (`ColorProfile` only) | Backend never sees tokens; style never sees input/screen (ADR-0003 + §5 rule below). |
| **Consumption: input** | input decode + dispatch (`InputProfile` only) | Owns the RFC-1 Q12 ladder depth decision (kitty vs legacy) as a pure function of `InputProfile`. |
| **Consumption: screen** | renderer + lifecycle (`ScreenProfile` only) | Resize ordering per ARCHITECTURE §8.5; style provably unaffected (R2). |
| **Test double** | `jilva-terminal` test double | Injects arbitrary snapshots headless (§8); the ONLY backend a test ever names. |

Consumption rule (normative for the implementing RFC): **a module takes its domain struct
by value or shared reference, never the aggregate.** The aggregate crosses exactly two
seams: terminal→loop (production of snapshots) and loop→frame-commit (one snapshot per
frame, §7). Any function taking `&TerminalCapabilities` outside those two seams is a review
flag. This is what makes the hybrid behave like B where it counts.

Dependency direction restated: `jilva-core` defines data, depends on nothing;
`jilva-terminal` depends on core's data types for detection output (allowed: data flows
upward, detection does not flow downward — core still never names terminal, TTY, env, or
backend). Facade re-exports the data types; examples/tests import facade only.

## 5. Unknown-info handling

Every signal that detection can fail to determine gets an explicit unknown/degraded value;
**guessing is forbidden** (extends RFC-3 §5's background rule to all domains):

- `BackgroundHint::Unknown` (already in RFC-3): keep app-selected base theme, dual-legible
  defaults, no auto-switch. Unchanged by this resolution.
- `KeyEncoding::Legacy` as the conservative baseline (NOT an `Unknown` variant): behavior
  under unknown is identical to legacy (no enhanced keys assumed, no kitty push attempted),
  so a separate unknown state adds no information. Detection upgrades to `Enhanced` only on
  positive negotiation evidence. Rationale documented: conservative degradation, not
  guessed capability.
- `MouseMode::Off` baseline: widgets requiring mouse declare it; dispatch gates on
  `mouse_mode != Off` (RFC-2's gate reads this field). Terminals without mouse get fully
  functional keyboard paths — mouse is enhancement, never requirement.
- `alternate_screen: false` baseline: app runs inline (scrollback-respecting) rather than
  assuming alt-screen. Matches existing `CapabilityProfile::basic()` conservatism.
- Attr flags default false individually: each unsupported attribute degrades per RFC-3 §5's
  per-attribute table (italic dropped never color-simulated; dim→Muted; underline-styles
  stepwise). No wholesale "assume modern" path.
- `tier` baseline is `Ansi16`, NOT `Mono`: `basic()` today assumes nothing-advanced but the
  style chain's conservative default must still render distinguishable feedback tokens;
  `Mono` is reserved for positive evidence (genuinely mono terminal OR `NO_COLOR`). This
  tightens the current `truecolor: false` bool into a tier with a documented default and
  should be reflected in the RFC-3 amendment (§10, item 2).

Unknown propagation: unknown values flow INTO resolution as ordinary inputs (resolution is
total over all domain values — no `Result`, no panic, never invisible text). Unknown never
propagates as an error to app code; apps may read the snapshot fields to adapt content
(API_DESIGN §9 opt-in manual handling) but degradation is automatic.

## 6. Change timing, versions, and invalidation policy

- **Detection cadence:** once at startup, cached (ARCHITECTURE §8.14). Refresh is
  **event-driven at frame boundaries only**: env change (`NO_COLOR` export mid-run),
  pipe-redirect, SIGWINCH/resize notification, kitty-flag loss (e.g. SSH reattach
  dropping `COLORTERM` forwarding). No polling in the hot path; no mid-frame sniffing.
- **Version computation:** the assembler compares each new domain against the last
  committed snapshot field-wise; differing domain → bump that domain's version.
  `snapshot_id` bumps on EVERY assembled snapshot (monotonic counter, u64, wraps only in
  theory — document wrap as impossible-in-practice, no handling). Domain versions bump
  ONLY on actual difference (cheap derive of `PartialEq` comparison; domains are tiny
  `Copy` structs).
- **Invalidation policy (normative):**
  - `color_version` change → style resolution cache keyed `(theme_version,
    color_version)` invalidates → **repaint** (never layout; RFC-3 R2/R5). Frames record
    both versions; mismatch on clean tree → re-resolve + repaint, skip measure/arrange.
  - `input_version` change → decoder re-arms (kitty push/pop flags, mouse-mode
    renegotiation at the backend edge) → affects FUTURE event decode only; committed
    frames and layout untouched. Class: `none` w.r.t. rendering (no damage at all)
    unless the change alters focus-relevant dispatch (then ordinary event-driven
    invalidation via RFC-2 paths, not a special capability path).
  - `screen_version` change (size/resize_pending/alt-screen) → **layout + repaint**
    via the resize path (ARCHITECTURE §8.5 ordering: observe → re-layout → re-render).
    Style resolution output is untouched (width-neutral guarantee); only geometry flows.
  - `reduced_motion` change → no invalidation in Phase 1 (no motion crate reads it yet);
    recorded so the future motion RFC inherits a versioned signal instead of inventing one.
  - Theme switch (`theme_version` bump, RFC-3 §6) is orthogonal: re-resolve + repaint,
    same as today. Theme and capability versions compose conjunctively in the cache key;
    either changing invalidates resolution, neither ever invalidates layout.

## 7. Frame atomicity: snapshot-at-start (DECIDED)

Three candidates: (a) complete-next-frame (apply mid-frame change to the following frame),
(b) abort-uncommitted (drop the in-flight frame on change), (c) snapshot-at-start (frame
commits against the snapshot it started with; changes queue for the next frame).

**Decide (c) snapshot-at-start**, with correctness constraint: **a committed frame MUST
resolve all cells against exactly one snapshot; profiles from two snapshots MUST never mix
within one committed frame** (never partially apply incompatible profiles — e.g. quantize
half the tokens at 256 and half at 16, or decode mouse coordinates under a stale viewport).

Reasoning:

- (a) complete-next-frame is under-specified: "next frame" of WHAT — the in-flight frame
  still needs SOME profile to complete against, which collapses into (c) with vaguer
  language. As a distinct policy (finish frame against the NEW profile mid-flight) it
  violates the constraint directly.
- (b) abort-uncommitted is correct but wasteful and complicates the loop contract
  (half-built component output discarded, resize-during-every-frame pathologies,
  interaction with RFC-2's transactional per-event update rule in API_DESIGN §5). Aborts
  also risk livelock under rapid resize. Rejected except as the lifecycle RFC's prerogative
  for catastrophic backend loss (backend disconnect ≠ capability change; that path owns its
  own abort semantics).
- (c) is the only policy that is both correct and cheap: clone a ~5-word `Copy` snapshot
  at frame start (no allocation, no lock — `Copy` by construction), run decode → update →
  resolve → diff → emit against it, queue any mid-frame detection event for assembly into
  the NEXT snapshot at the next frame boundary. Constraint enforcement is structural:
  resolution functions take `ColorProfile` by value (a field of the frame's snapshot copy),
  so mixing requires deliberately threading two snapshots — a review-visible error, plus a
  debug assertion (`debug_assert_eq!(frame.snapshot_id, color_source_snapshot_id)`) at the
  resolve call site in debug builds, mirroring RFC-2's damage-vs-diff assertion philosophy.

Consequences: a `NO_COLOR` export or resize landing mid-frame takes effect on frame N+1,
never tears frame N. Latency cost is one frame — imperceptible and already the norm for
resize handling. The open RFC-3 Q7 ("does a profile change mid-frame drop or complete the
frame?") is answered: **complete the frame against the start snapshot; the change lands on
the next boundary.** Lifecycle-RFC agreement needed only on the event-queue mechanics, not
the policy.

## 8. Headless injection and test double

- Domains are `Copy` plain data: tests construct `ColorProfile { … }` / `InputProfile`
  directly — no TTY, no env manipulation, no backend. This preserves and extends the
  current `CapabilityProfile::basic()` / `full()` pattern per domain
  (`ColorProfile::basic/full`, `InputProfile::basic/full`, `ScreenProfile::basic/full`)
  plus `TerminalCapabilities::basic/full` aggregates for frame-level tests.
- The test double accepts scripted snapshots: `test_double.push_snapshot(caps)` queues a
  snapshot delivered at the next frame boundary (exercising the version/assembly path), and
  `test_double.set_snapshot(caps)` replaces the current snapshot immediately between
  frames. Mid-frame injection is refused (returns the frame's snapshot unchanged or a
  descriptive error per the facade error taxonomy — implementation detail, but the refusal
  itself is normative: tests must not be able to create the mixed-profile frames §7
  forbids production from creating).
- Golden-output tests (ARCHITECTURE §8.13) assert instruction streams per injected
  snapshot; tier-matrix tests inject `ColorProfile`s without ever naming input/screen.

## 9. RFC-3 token-system validation (as capability consumer)

RFC-3's token design is validated against this capability model; findings are framed as
amendment items (§10), not edits:

- **Closed enum vs extensible:** ENDORSE closed `Token` enum + `ExtensionMap` pressure
  valve (RFC-3 §2/§4). The capability model strengthens the case: quantization snapshots
  are exhaustive over a closed token set (tokens × tiers is finite and golden-fileable);
  stringly-typed tokens would make the snapshot matrix unbounded. Extension keys are
  explicitly EXCLUDED from tier snapshots (they resolve via the Text-fallback rule, tested
  once per tier, not per key). No change requested — endorsement only.
- **Merge/override:** ENDORSE per-token last-writer-wins with overlays-after-merge and
  quantization-last (RFC-3 §4). Ordering interaction with capability versions: merge
  output is cached per `theme_version`; quantization output per `(theme_version,
  color_version)`. Merge need not re-run on capability change (it is capability-independent
  — worth stating explicitly in the amendment; currently implied). Delta-merge test
  (RFC-3 Testing item 0) runs at every tier by injecting `ColorProfile`s — unchanged.
- **Focus/selected/disabled states:** ENDORSE Round-2 stacking with Disabled absorbing
  (RFC-3 §3). Capability-model note: focus visibility MUST survive to `Mono`
  (reverse-video/underline, never hue — RFC-3 §9 already requires this; the mono
  distinguishability test is the enforcement). The load-bearing pair `Selected + Focused`
  must be snapshotted at `Mono` specifically, not just truecolor — add to amendment.
- **Switching:** theme switching stays version-bump + paint-only (RFC-3 §4/§6). No
  capability-model change; the cache key extension (`color_version` alongside
  `theme_version`, §6) is the only touchpoint.
- **NO_COLOR:** RFC-3 §5.5 strips to mono + bold-off/plain with reverse-video retained
  for selection. Finding R3-D7 applies: no-color.org suppresses *color* only — bold
  stripping and reverse retention are Jilva policy. The amendment must cite the convention
  and label the two extensions as policy (one sentence each). The `no_color` flag (§3.1)
  is the mechanical carrier. Behavior itself is endorsed.
- **Mono readability:** ENDORSE the mono-distinguishability test as the Q12c harness
  core, with the §10 snapshot strategy deciding its exact coverage. Accessibility posture
  (§9 RFC-3: never color-alone, Disabled gates events, focus always visible) is compatible
  with this model — no capability signal weakens any a11y guarantee, because degradation
  only ever ADDS constraint (fewer colors/attrs), and the a11y rules are written against
  the most-constrained tier.

## 10. Snapshot strategy: representative + property (DECIDED)

Exhaustive (every token × every variant-stack × every tier × every background × every
attr-profile) vs pairwise vs representative+property.

**Decide representative + property**, with explicitly exhaustive islands. Coverage/
maintainability reasoning: the full cross product (15 tokens × 6 single overlays × 20+
pairs × 4 tiers × 3 backgrounds × 3 attr-profiles ≈ 30k+ golden rows) is write-once,
read-never — failures pin the diff but nobody reviews 30k rows, golden files rot, and
every palette-taste tweak becomes a 30k-line diff that teaches nothing. Representative
goldens pin INTENT (taste, role-mapping, distinguishability); property tests pin
INVARIANTS (determinism, ordering, monotonicity) across the whole space without golden
files. This matches RFC-3's existing direction (matrix + composition properties, Testing
§11) and extends it to the capability model.

**Exhaustive (golden-file every cell of the sub-matrix):**

1. Tier matrix: every token × `Normal` × every `ColorTier` (4) × every `BackgroundHint`
   (3) for each built-in theme (2). This is the taste + role-mapping contract
   (Danger→red-family at 16, dual-legible accents under Unknown). ≈ 15×4×3×2 = 360 rows.
2. Mono distinguishability: every feedback token vs `Text`/`Background` at `Mono`,
   attribute-step assertion (not just golden — assert ≥1 step difference mechanically).
   Includes the load-bearing stacked pair `Selected + Focused` for every token at `Mono`.
3. `NO_COLOR` profile: full token set × Normal + Selected/Focused/Disabled at mono +
   stripped attrs; assert zero color escapes + selection visible via reverse.
4. Unknown-background: full token set × Normal, assert theme unchanged + all tokens
   resolve against theme surfaces (no auto-switch artifact).
5. Per-attribute degradation: each single disabled attribute × representative token subset
   (Text, Accent, Danger, Selection) asserting the documented fallback
   (italic→none, dim→Muted, underline-styles stepwise). The SUBSET is exhaustive (every
   attribute × 4 tokens); the full token set is not repeated here.

**Representative + property (no new goldens per combination):**

- Variant stacking beyond singles + `Selected + Focused`: property tests over all pairs —
  determinism (same input → same output), order-equivalence (application order fixed by
  precedence regardless of set iteration order), Disabled-absorption (any set containing
  Disabled ≡ Disabled alone), and no-panic/totality over all token×variant-set inputs.
- Attr-support cross product (every attribute × every tier × every background): sampled
  representative profiles (full/minimal/none per RFC-3 Testing item 1) as goldens;
  off-sample combinations covered by the per-attribute fallback property (resolution is a
  pure function of flags; flags compose independently by construction).
- Input/screen domains: NO golden snapshots of rendered output per input/screen value
  (they don't affect styling by construction — that non-interference IS the property:
  style-golden equality across varying `InputProfile`/`ScreenProfile` with fixed
  `ColorProfile`, asserting the consumption rule §4 mechanically).
- Contrast ratios: property/assertion (≥4.5:1 body, ≥3:1 chrome at truecolor per
  built-in × declared background), exposed as helper for custom themes — not golden
  (numeric assertion, RFC-3 Testing item 3 unchanged).

**What needs exhaustive checking (summary):** tier×background×theme for base tokens;
mono distinguishability incl. the load-bearing pair; NO_COLOR; Unknown-background;
per-attribute singles on the 4-token subset. Everything else is property-tested.
If the widgets RFC demonstrates a third pair matters visually (RFC-3 Q4 residual), THAT
pair graduates to the exhaustive mono+truecolor islands by amendment — the mechanism for
extension is defined here so the argument stays evidence-driven.

Accessibility is included in the exhaustive islands (mono distinguishability, NO_COLOR
selection visibility, focus-never-hue-alone at every tier for Focused overlays), not
relegated to properties.

## 11. Handoff

### 11.1 Recommended structure

Typed-domain hybrid: `ColorProfile` / `InputProfile` / `ScreenProfile` data structs in
`jilva-core`, thin `TerminalCapabilities` aggregate (domains by value + `snapshot_id` +
per-domain versions + `reduced_motion`), detection + assembly + test double in
`jilva-terminal`, facade re-exports only.

### 11.2 Lifecycle

Detect once at startup → cache → event-driven refresh at frame boundaries → assembler
diffs domains → bumps per-domain versions → frame snapshots at start (`Copy`) → consumers
read only their domain → single-snapshot-per-committed-frame invariant enforced
structurally + by debug assertion.

### 11.3 Invalidation policy

`color_version` → repaint-only re-resolve; `input_version` → decoder re-arm, no render
damage; `screen_version` → layout+repaint via resize path; theme switch orthogonal
paint-only; `reduced_motion` no-op in Phase 1. Cache keys: merge per `theme_version`,
quantization per `(theme_version, color_version)`.

### 11.4 Snapshot strategy

Representative goldens + property tests (§10); exhaustive islands named explicitly
(tier×background matrix, mono distinguishability incl. S+F pair, NO_COLOR,
Unknown-background, per-attribute singles subset). Non-interference property proves
input/screen never affect styling.

### 11.5 RFC-3 amendment list (DO NOT edit RFC-3 here — maintainer/implementer applies)

1. Profile section (§5): split the visual-heavy `CapabilityProfile` into the domain
   model — `ColorProfile { tier, attrs, background, no_color }` as the style-consumed
   projection, with `InputProfile`/`ScreenProfile` named as sibling domains owned
   elsewhere (input/lifecycle RFCs), and the aggregate snapshot rule referenced. Closes
   R3-D2 homelessness without RFC-3 owning input semantics.
2. Baseline default: name `Ansi16` (not `Mono`) as the conservative color default;
   reserve `Mono` for positive evidence or `NO_COLOR`. One sentence + basic()/full()
   note.
3. Merge section (§4): state explicitly that token-merge output is capability-independent
   (cached per `theme_version`; only quantization re-runs per `color_version`).
4. Q7 (open question 7): resolve as snapshot-at-start — complete the frame against the
   start snapshot; changes land on the next boundary. Point at the lifecycle RFC for
   queue mechanics.
5. NO_COLOR (§5.5): cite no-color.org; label bold-stripping and reverse-retention as
   Jilva policy (R3-D7). One sentence each.
6. Mono snapshots: add `Selected + Focused` per token at `Mono` to the required matrix
   (currently implied at all tiers — make mono explicit, it is the a11y-critical tier).
7. Per-attribute fallback table: add the `dim → Muted` resolution-order note (Muted
   resolved first; non-text elements unaffected) per R3-D9.
8. Quantization metric (§5.2 vs Q5): move "minimum perceptual distance" into Q5's scope;
   normative text requires "nearest-palette by a documented metric" (R3-D5). One-line
   change.
9. Contrast checks (§11.3): specify the background matrix the checks run against incl.
   `Unknown` handling (R3-D4) — or explicitly downgrade the Unknown guarantee. Must not
   stay ambiguous.
10. Citations (R3-D10): add `ratatui-themes`, Cursive TOML-theme, Textual
    design-tokens/`watch` links to Prior art.
11. 16-vs-256 asymmetry (R3-D11): one sentence why distance fits 256 but roles win at 16.
12. `blink` (Q10/R3-D9): recommend refuse-entirely/fail-loud over deprecated-at-birth
    shim (no legacy users exist). RFC-3 owner's call; recorded here as recommendation.
13. Serde wording (§4/R3-D3): rephrase "zero-dep constraint" to "minimal-and-justified
    per ARCHITECTURE §6.6; serde excluded for compile-time/MSRV-surface cost, not license"
    so the text cannot be read as a core-takes-no-external-deps precedent (which would
    endanger the future segmentation/width deps in X2/X3).

### 11.6 Cross-review recall

For the Phase 1.6 cross-review: contested points are (a) `reduced_motion` on the aggregate
vs a future `UserProfile` domain (§3.1 — promotion path defined, no action now); (b) the
`no_color`-alongside-`Mono` redundancy (§3.1 — deliberate, audit + policy distinction);
(c) `Legacy`-as-baseline instead of `KeyEncoding::Unknown` (§5 — same behavior, less
state); (d) snapshot coverage scope for non-load-bearing variant pairs (§10 — property
coverage + evidence-driven graduation rule; escalate to widgets-RFC evidence per RFC-3
Q4). No production code touched; R3-D1 (stacking) and X-blockers are out of this
resolution's scope except where referenced.

## 12. Cross-review (Phase 1.6, Agent 3 reviews Agents 1+2)

Read: `docs/research/RENDERING_MODEL_RESOLUTION.md` (Subagent 1, §§1–10; no Round-2
section present — reviewed main body) and `docs/research/COMPONENT_FEASIBILITY.md`
(Subagent 2, Parts A–C + handoff; no Round-2 section present — reviewed main body).
Scope per task §6: do capability/profile changes integrate correctly with component
updates + rendering? Four checks below.

### 12.1 Theme/profile version bumps vs repaint-only rule against the owned-View pipeline

Agent 1 decides pipeline C: `describe (&Model,&Slot,&Context) -> Desc (owned)` →
`layout (&Desc + viewport) -> Placed` → `paint (sole &mut CellFrame, reused double
buffer)`; change classes gate stages (`none` skips all, `repaint` skips layout —
Rendering §5 + patch item 5). My §6 says a theme/capability change means "re-resolve +
repaint, skip measure/arrange" with "no subtree walk, no re-layout, no re-instantiation"
(inherited from RFC-3 §6).

- AGREEMENT (load-bearing): repaint-skips-layout holds on both sides. A `color_version`
  or `theme_version` bump never runs layout; a `screen_version` change runs the full
  resize path. Neither resolution lets style/capability changes move cells.
- CLARIFICATION (not a contradiction, but my §6 / RFC-3 §6 "no subtree walk" overclaims):
  Rendering patch item 5 implies `repaint` still runs stage 1 (`describe`) — only `none`
  skips describe. So on a theme switch, views DO re-run (all nodes, next frame), producing
  a fresh `Desc`, while layout is skipped and only resolution/paint redo work. Agent 2's
  allocation map dissolves any efficiency worry here: per-frame descriptions are already
  rebuilt every frame (caller-owned `O(children)`, bounded) — a theme switch costs exactly
  one ordinary frame's describe, not a special walk. The version counters gate
  merge/quantize caches and layout/paint skipping; they do not gate describe.
- INTEGRATION ANSWER (who re-runs view on theme switch): all nodes, on the next frame,
  via the normal per-frame describe — no dedicated propagation walk, no targeted
  subtree re-run. Version check decides what is SKIPPED (layout for repaint), never who
  is visited.
- DELTA: amendment item 3 is touched up (see §12.5): merge/quantize cache keys stand, but
  the text must acknowledge describe-still-runs (only layout/paint are skipped). One
  forwarded question for the renderer RFC, not RFC-3: whether `Desc` carries tokens
  (enabling a future re-resolve-without-redescribe fast path) or resolved attributes
  (re-describe mandatory on theme switch). Either is contract-compatible today; the
  encoding decision (Rendering §8 uncertainty 1) settles it. No RFC-3 amendment needed
  beyond the item-3 wording fix.

### 12.2 Snapshot-at-start vs fail-clean buffer

Agent 1 §5 rule 5: any `Err` in describe/layout aborts before paint; previous buffer
retained; diff emits nothing (measured: B1 orphans, B2 half-painted buffers rejected;
C keeps no-partial-output). My §7: mid-frame profile change queues to the next boundary;
the in-flight frame completes against its start snapshot.

- AGREEMENT: the two rules compose without conflict because they govern disjoint events.
  Capability change is NOT an error — it completes-then-applies; describe/layout failure
  is an error — it aborts-before-paint. Both preserve the same invariant (never emit a
  torn frame) from opposite directions.
- Combined cases walk clean: (a) profile change arrives mid-frame AND describe later
  fails → frame aborts, buffer untouched, next frame starts from the NEW snapshot. No
  lost update (the change was queued, not consumed) and no torn output. (b) Profile
  change arrives between frames → next frame's start snapshot includes it; versions gate
  caches normally. (c) Rapid resize per frame → each frame commits against its own start
  viewport; no livelock, at most one frame of staleness — consistent with Agent 1's
  rejection of abort-uncommitted for the same livelock reason (my §7 reasoning (b)).
- DELTA: amendment item 4 gains one sentence (see §12.5): the snapshot-at-start rule and
  the fail-clean buffer rule are stated as a single combined frame-commit invariant for
  the lifecycle RFC to own: "a committed frame resolves all cells against exactly one
  start snapshot AND reaches paint only through an error-free describe/layout; anything
  else retains the previous buffer and emits nothing."

### 12.3 Per-domain versions vs generational slots (shared-mutability / global-state audit)

Agent 2 (exp03): framework-owned generational `SlotTable` (`Vec` + free list,
`(parent,Key)→SlotId` map), safe indexed access, `forbid(unsafe_code)`, no globals; stale
handles return `None` on generation bump. My model: loop-owned monotonic counters,
`Copy` snapshots cloned at frame start, field-diff versioning, no locks, no statics.

- AGREEMENT, no smell found: versions are VALUES (u64 integers compared per frame), not
  handles into shared tables — there is nothing to borrow, lock, or invalidate. The
  snapshot is `Copy`, so frame-start capture cannot alias loop state; the `SlotTable`'s
  `&mut`-on-one-thread discipline (Agent 2 Part C) and my snapshot discipline are the
  same pattern applied to different data (identity storage vs terminal truth). Both
  designs satisfy ARCHITECTURE §2.3 (no ambient global state) structurally, not by
  convention.
- Structural parallel (mutual reinforcement, not a change): slot stale-handle→`None`
  and my test-double mid-frame-injection refusal (§8) are the same rule — "stale identity
  never resolves" — applied to component identity and frame identity respectively.
  Reviewers can check one to understand the other.
- No shared-mutability vector: the assembler (terminal edge) is the sole writer of new
  snapshots; the frame holds a copy; consumers hold sub-copies. Single writer, value
  semantics, no interior mutability anywhere. DELTA: none.

### 12.4 Input/screen non-interference property vs Effect bridge

Agent 2 Part C: policy (b) — no `Send`/`Sync` on models/slots; `Send` only on bridge
job closures and completion values; `Effect::Task` is opaque routed data core never
executes (test 5 `route_without_executing`); R2-C9 executor-by-stealth closed. My §10:
non-interference property — style goldens invariant across varying
`InputProfile`/`ScreenProfile` with fixed `ColorProfile`.

- AGREEMENT, no break: capability flows terminal→loop→frame only; it never enters
  `Effect::Task` (which carries app messages `M`, never capability writes), and effect
  completions cannot bypass the version path — if a completion triggers a theme switch,
  that is an ordinary update→re-render with a `theme_version` bump and repaint, gated by
  the same caches. The bridge's opacity (core routes without executing) and my "core
  never sniffs env" rule are the same boundary from two sides: environment truth enters
  exclusively at the terminal edge.
- Checked bypass paths, all closed: (a) input re-arm (kitty push/pop, mouse-mode
  renegotiation) happens at the backend edge on `input_version` change, never via effects;
  (b) resize arrives as a loop-level event through the existing taxonomy, never as effect
  data; (c) `reduced_motion` is read by the future motion crate from the snapshot, not
  posted through the bridge. An effect closure capturing a stale snapshot copy is harmless
  (values, not handles — worst case it reads last frame's truth for diagnostics).
- DELTA: none. The non-interference property stands as stated; Agent 2's P6 patch
  (constrain `Effect::Task` to opaque routed data) is compatible and needs no
  capability-model counterpart.

### 12.5 Handoff — integration points and amendment-list deltas

Confirmed working (no action): snapshot-at-start + fail-clean buffer composition (§12.2);
versions-vs-slots value-semantics agreement (§12.3); Effect-bridge non-bypass (§12.4);
repaint-skips-layout on both sides (§12.1 agreement half).

Needs wording-level repair (my doc / RFC-3 amendment list, NOT the sibling docs):

1. Amendment item 3 DELTA — append: "describe still runs every frame (per-frame
   descriptions are rebuilt regardless); the cache keys gate merge/quantize redo and
   layout/paint skipping only." Fixes the §6 "no subtree walk" overclaim without
   changing any decision.
2. Amendment item 4 DELTA — append the combined frame-commit invariant (error-free
   describe/layout + single start snapshot, else retain-and-emit-nothing) and name the
   lifecycle RFC as its owner. Fixes Q7 with the buffer rule folded in.
3. FORWARDED (renderer RFC, not RFC-3, no amendment item): `Desc`-carries-tokens vs
   resolved-attributes encoding decides whether a future re-resolve-without-redescribe
   optimization is possible (Rendering §8 uncertainty 1). Recorded here so the renderer
   author sees the capability-model stake; default path (re-describe every frame) works
   regardless.

Broken: none found. No contradiction in any of the four checks rises above
clarification; no new RFC-3 amendment item is created (two items gain a sentence each);
sibling resolutions require zero changes from this review.
