# RFC Acceptance Checklist (consolidated, Subagent 4 — Phase 1.7)

All three RFCs: Status **Draft**. Acceptance = maintainer decision after ≥1-week Discussion per `docs/RFC/README.md`. `ready-pending-discussion` means: no rework demanded, Discussion decides contested items. Details: `docs/reviews/CROSS_RFC_ACCEPTANCE_REVIEW.md`.

## RFC-0001 — Framework Scope

- [ ] 14 scope questions (Q1–Q14) answered; framework-vs-library line (owns the loop) intact.
- [ ] Alpha Must list (Q4: loop+restoration, flex/grid, tokens+default+tier chain, 6 widgets, keyboard/resize/tick/message, gated mouse, test double, golden+scripted tests) achievable; slip plan = cut Should/Could, never Must.
- [ ] Must/Should/Could/Out matrix enforced: Must rows have headless tests; Should adds no alpha API beyond seam; Could cited as commitment nowhere.
- [ ] Permanent refusals (GUI, app-framework, async-in-core, raw ANSI, globals, backend-aware core, CSS leakage, macro-primary, `cfg(windows)` in core, 3D, sound, GPU) vs deferred (Q5 list) — split correct, no reclassification.
- [ ] Change taxonomy `none`/`repaint`/`layout`, reconstruct = identity output — adopted verbatim by RFC-2 (R2-C1 retired).
- [ ] §8 mapping: all 14 considerations owned/scoped/deferred; none silently dropped (§11 conformance).
- [ ] Open items tracked to owners: remainder rule + min-size (layout RFC); tick source + ladder depth + teardown pairing + lifecycle mechanics (lifecycle RFC); symbol type + authorities (buffer/diff RFC ≤ Phase 3); error-visibility wording (Discussion confirm).
- [ ] Verdict: **ready-pending-discussion**.

## RFC-0002 — Component Model

- [ ] Hybrid (MVU loop + paths/keys identity + pure view + data-only effects) justified; rejected alternatives (immediate-only, retained-callback, signals, Xilem-fidelity) recorded with reasons.
- [ ] Identity contract (§2 misuse table) + generational slot storage (§8 F2.3, hand-rolled, zero-dep) + `&mut` update / `&`-only view + owned frame descriptions, no `&mut Frame` in trait (F2.1/F2.4, no `unsafe`).
- [ ] Heterogeneous children: `dyn`-compatible seam + homogeneous fast path + mount-once boxing (F2.2) — shared-G acknowledgement satisfied.
- [ ] Event extensibility (`#[non_exhaustive]` + `Message(M)`) and tick-as-injected-value positions hold; Send-at-boundary-only (no `Send`/`Sync` on models/slots).
- [ ] `Effect` stays opaque routed data, zero async deps in core (OQ14 condition); core names no thread/executor/spawn.
- [ ] Component text in `&str`/clusters, width-supplied offsets; no `char`-cell entrenchment (renderer RFC ≤ Phase 3 dependency recorded).
- [ ] Contested for Discussion (not blockers): keys compile-time vs runtime-advisory; `Outcome::Observed` survival (lean drop); effect-cap value/overflow semantics.
- [ ] Verdict: **ready-pending-discussion**.

## RFC-0003 — Theming Architecture

- [ ] Closed token enum (~15, per-token justification) + orthogonal stacked variants (`Hovered<Active<Focused<Selected`, `Disabled` absorbing) + R1–R5 rules; raw-ANSI ban is permanent with no escape hatch.
- [ ] Merge order (deltas→base→default, per-token last-writer-wins, overlays after merge, quantization last, capability-independent) + delta-merge test.
- [ ] Fallback chain truecolor→256→16→Mono; 16-by-role asymmetry; Mono attribute-only meaning; `NO_COLOR`→Mono policy; Unknown background→no auto-switch; conservative default Ansi16.
- [ ] Capability typed-domain model (Color/Input/Screen + aggregate transport, per-domain consumption); detection in terminal, pure resolution in core, backend sees resolved attrs only (ADR-0003).
- [ ] Refresh = snapshot-at-start + per-domain versions; queue mechanics owned by lifecycle RFC.
- [ ] Test harness: tier matrix + mono S+F + contrast + `NO_COLOR` + Unknown-bg + invalidation + widget no-ANSI + per-attribute degradation; goldens under `tests/`, facade-only.
- [ ] Authority splits hold: style classifies (all Phase-1 changes paint-only), loop applies max rule + debug asserts; style width-neutral, truncation authority → layout/renderer.
- [ ] Contested for Discussion (not blockers): overlay-pair snapshot scope (OQ4 residual); quantization metric (OQ5); missing-extension-key behaviour (OQ3); scoped themes (OQ6, not proposed); high-contrast built-in vs recipe (OQ9); palette values + taste review (OQ1).
- [ ] Verdict: **ready-pending-discussion**.

## Shared items (cross-RFC)

- [ ] A (change classes): `none`/`repaint`/`layout` — resolved, both RFCs aligned; fourth class needs RFC-1 amendment.
- [ ] B (theme propagation): version-counter + paint-only damage (RFC-3 §6); scoped-themes watch trigger set.
- [ ] D (capability entry): detect-in-terminal → plain-data profile → pure core resolution; refresh cadence → lifecycle RFC.
- [ ] G (smallest API): construct→run→describe→handle→style→observe path intact (~8–15-line hello bar); heterogeneous mechanism answered, no scope amendment needed.
- [ ] I (Unicode): style width-neutral/cluster-aligned; width/segmentation authority → layout/renderer RFCs; `char` replacement ≤ Phase 3 tracked as exit obligation; no permanent `char` dependence.
- [ ] J (expensive-to-change seven): identity, event extensibility, constraint vocab, token vocab, profile shape, error taxonomy, backend seam — all flagged load-bearing for Discussion.
- [ ] Phase-2 entry: Discussion week done; seam shape + error skeleton + profile domain split agreed; `char`-non-entrenchment rule in force. Phase-3 content (symbol type, diff bounds, damage flow) must NOT be demanded pre-Phase-2.
- [ ] MIT-only: no proposal introduces a non-MIT dependency; `serde`/`palette`/width/segmentation crates admissible only additive + §6.6-justified via future RFC.
