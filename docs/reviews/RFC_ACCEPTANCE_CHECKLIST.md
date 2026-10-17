# RFC Acceptance Checklist — current revalidation 2026-10-17

This checklist supersedes the Phase-1.7 draft checklist. The project owner's
2026-10-17 continuation directive authorizes maintainer-driven acceptance and
waives the external Discussion wait. RFC-0001/0002/0003 are **accepted as design
authority for Phase 2**; acceptance does not mean their future implementations
or test harnesses exist. Decisions and historical records are in
`RFC_DECISION_TRACKER.md`.

Evidence method: read the current RFC text and existing review documents;
check the named sections below. Existing review files are repository evidence,
not newly proven independent authorship or reproduced compiler experiments.
The orchestrator separately verified the current GitHub Discussion facts on
2026-10-17. No community approval is inferred from silent threads.

`[x]` means the design/triage was checked in the source text. Unchecked items
are actual implementation/verification obligations with an owning phase;
they are not silently counted as completed tests.

## RFC-0001 — Framework Scope

- [x] Proposal §1 answers Q1–Q14; Q1 retains framework ownership of the loop.
- [x] Q4 and the Must/Should/Could/Out matrix define the alpha floor and the
  cut order (Should/Could before Must). Achievability remains a planning risk,
  as recorded in `CROSS_RFC_ACCEPTANCE_REVIEW.md` §A.
- [x] Q5/Q9 distinguish deferred features from permanent refusals; backend
  I/O, raw ANSI style, async runtime, globals and platform-aware core stay out
  of the core/component contract. No new framework scope was added.
- [x] Q13 and RFC-2 §5 agree on `none`/`repaint`/`layout`; reconstruction is an
  identity output, not a fourth invalidation class.
- [x] The consideration mapping covers the architecture's fourteen concerns.
- [x] Open questions have owners in tracker D1–D9: layout, lifecycle, renderer
  or theme work. The error-visibility ambiguity is explicitly resolved for
  the current backend: public `io::Result`, shared future runtime `Error`.
- [ ] Alpha Must rows have implementation and substantive headless tests —
  **deferred to Phases 3–7**, not a Phase-2 acceptance claim.
- [x] Verdict: accepted for scope; later phase obligations remain mandatory.

## RFC-0002 — Component Model

- [x] Summary, alternatives and §8 justify hybrid MVU, retained identity,
  pure owned views and data effects; alternative approaches have reasons.
- [x] §2 names identity/misuse rules; §8 F2.1/F2.3/F2.4 names immutable view,
  owned descriptions and generational slot storage without unsafe code.
- [x] §8 F2.2 names an erased `dyn` seam, homogeneous fast path and mount-once
  boxing. This closes the design mechanism, not implementation benchmarks.
- [x] §4 and §8 F2.5/F2.6 retain event extensibility, injected ticks and
  Send-at-boundary-only; core has no mandated executor/runtime.
- [x] §5 requires both consumption (`Outcome`) and invalidation (`Change`)
  outputs from every update. The exact Rust carrier is left to the component
  implementation RFC; illustrative sketches do not override this rule.
- [x] Unicode dependencies keep component strings cluster-oriented and assign
  width/segmentation to the renderer/layout RFC by Phase 3.
- [x] Acceptance dispositions are explicit: drop `Outcome::Observed`; retain
  runtime-required stable keys with loud misuse detection; defer effect cap
  and describe/layout fusion with owners (tracker D1/D2).
- [ ] Component state slots, routing, effects, builders and runtime tests —
  **deferred to Phases 4–5**. Historical prototype/compiler measurements were
  not re-executed during this checklist review.
- [x] Verdict: accepted as the future component design; Phase 2 implements
  terminal-origin data only.

## RFC-0003 — Theming Architecture

- [x] §§2–3 and §7 justify the closed token/attribute vocabulary, overlay
  precedence, absorbing Disabled state and R1–R5. Raw ANSI style stays banned.
- [x] §4 states delta/base/default merge, per-token last writer, overlays after
  merge and final quantization; the merge is capability-independent.
- [x] §5 specifies truecolor → 256 → 16 → mono, role-aware degradation,
  Unknown-background no-auto-switch and conservative Ansi16.
- [x] §5 explicitly defines the stronger Jilva `NO_COLOR` policy: any presence,
  including an empty value, means mono with decorative attributes stripped;
  structural reverse video remains available. This is Jilva policy, not a
  claim that the upstream informal convention requires empty-value handling
  or suppressing all text decoration.
- [x] §5 names color/input/screen domains and aggregate transport; terminal
  collects evidence, core owns plain data/pure helpers, backend sees resolved
  attributes. The domain-consumption wording repair is present.
- [x] §4 says owned descriptions may still be visited on repaint; unchanged
  geometry is reused. Q7 states snapshot-at-start and error-free frame commit.
- [x] §6 and the Unicode open question preserve style's width-neutrality and
  classify-versus-loop-mechanism split.
- [x] Residuals are triaged: default palette/taste and quantization metric D3/D4
  (Phase 6); overlay coverage remains a widget-driven amendment trigger;
  extension misses use Text plus debug marker; high contrast is recipe-first;
  ThemeKind is advisory; scoped themes require a scope amendment.
- [ ] Theme implementation, merge/contrast/overlay/attribute goldens and
  invalidation harness in the testing section — **deferred to Phase 6**.
- [ ] Dynamic refresh queue and frame-boundary application — **deferred to
  Phase 5**, not an existing Phase-2 hook.
- [x] Verdict: accepted for theme architecture; Phase 2 supplies capability and
  resolved-output data, not theme resolution or widgets.

## Shared acceptance and Phase-2 gate

- [x] Shared A/B/D/G/I/J decisions are recorded: three change classes; explicit
  versioning; terminal evidence collection; small facade; grapheme replacement
  deadline; identity/event/constraint/token/profile/error/backend seams.
- [x] Stable backend method names and shared type ownership are agreed in
  `../PHASE2_BACKEND_CONTRACT.md`; no dependency cycle is permitted.
- [x] Current seam corrections are recorded: source readiness does not promise
  a mapped event; bounded unsupported-event filtering; decoder I/O propagates;
  full-batch text validation; flush error cannot roll back emitted bytes;
  standard I/O errors are public at the low-level seam.
- [x] Terminal focus differs from component focus synthesis. Optional paste
  remains untrusted text. Key kind/modifiers are preserved when provided;
  repeat/release alone does not establish an enhanced encoding.
- [x] External elapsed-time gate is waived by the current user directive.
  A zero-comment Discussion is neither an approval nor a technical blocker.
- [x] Workspace source remains MIT; external dependencies require necessity,
  compatible licensing and recorded version/features under architecture §6
  rule 6. This does not require every dependency to use only the MIT license.
- [x] **Phase-2 production continuation authorized**. Renderer, runtime,
  components and themes stay within their existing roadmap phases.
- [ ] Phase-2 implementation completion: required Cargo gates, headless failure
  coverage, independent terminal audit and actual platform results. These are
  **Phase-2 exit gates**, not RFC acceptance requirements already satisfied.
- [ ] Grapheme-capable cell replacement and width/segmentation authorities —
  **Phase-3 exit obligation**. `Cell.symbol: char` remains provisional and
  must not spread into terminal output APIs or scalar-cell goldens.

## Existing review evidence inspected

| Review | Current evidence used | What is not claimed |
|---|---|---|
| `RFC_DESIGN_REVIEW.md` §§6–7 | Scope review and original blockers | New independent authorship or prototype rerun |
| `RFC2_FINAL_TECHNICAL_REVIEW.md` §§0/6/8 | I1/I3 requirements and decision alternatives; current RFC-2 §§1/5/8 inspected | Fully implemented component API |
| `RFC3_TERMINAL_READINESS.md` final amendment section | P1/P2/P3 requirements; current RFC-3 §§4/5/Q7 inspected | Existing theme or runtime test harness |
| `CROSS_RFC_ACCEPTANCE_REVIEW.md` §E | Cross-RFC readiness and owned residuals | External maintainer approval |

The earlier all-Draft/pending-Discussion checklist was a historical Phase-1.7
state. Its implementation demands remain open under the owners above; today's
dated source check replaces its stale status, without backdating completion.
