# Security Considerations — jilva-rs (Phase 1, architectural level)

> Status: Phase 1 foundation — **risk register, not mitigations**.
> Each entry states the risk, its impact, which future phase addresses it,
> what Phase 1 must already guarantee, and how that guarantee is verified.
> Implementation mitigations belong to later phases; do not read this file
> as claiming any mitigation exists in code (there is no code yet).
>
> Related: `SECURITY.md` (vulnerability reporting — not owned by this file),
> `docs/ARCHITECTURE.md` (§8 considerations), `docs/QUALITY_STANDARDS.md`,
> ADR-0003 (backend seam strategy).

## Risk register

### R1. Terminal state not restored on exit (alternate screen, raw mode, cursor, mouse reporting)

- **Risk:** a crash, propagated error, or forgotten teardown path leaves the
  user's terminal in raw mode / alternate screen / hidden cursor, rendering
  their shell unusable until manual `reset`.
- **Impact:** High — every user of every app built on the framework is one
  mishandled exit path away from a damaged terminal session.
- **Owner phase:** implementation phase that builds the lifecycle
  (ARCHITECTURE.md §3.2, consideration §8.7) — full panic-hook and teardown
  design lands there.
- **Phase 1 must already guarantee:** the lifecycle contract in §3.2 is treated
  as normative (restore-on-every-exit-path: normal, `?`-error, panic-unwind
  where feasible), no design is accepted that narrows it, and every RFC
  touching lifecycle enumerates its exit paths.
- **Verification:** RFC review against §3.2/§8.7; later, fault-injection tests
  (error mid-frame, panic mid-frame) asserting restoration via the test double.

### R2. Raw mode left enabled (input echo / line discipline broken)

- **Risk:** subset of R1 with distinct symptom — shell stops echoing input or
  stops processing line discipline because raw mode was not exited.
- **Impact:** High — user cannot interact with their shell normally.
- **Owner phase:** same lifecycle implementation phase as R1.
- **Phase 1 must already guarantee:** raw-mode enter/exit is owned exclusively
  by the backend seam (§3.5) behind the app-construction edge, never by
  component or user code; designs must not expose raw-mode handles publicly.
- **Verification:** API review — no public raw-mode handle in any accepted RFC;
  later, test-double tests asserting exit-raw-mode is emitted on all teardown paths.

### R3. Event-loop cancellation / shutdown races

- **Risk:** shutdown signal races in-flight frames: a resize or tick arriving
  during teardown is processed against half-torn-down state, or shutdown hangs
  waiting on an event source that never yields.
- **Impact:** Medium — hangs on exit, torn frames, or use of invalidated state.
- **Owner phase:** lifecycle + event-propagation implementation
  (§3.2, §3.4; backpressure/queueing policy).
- **Phase 1 must already guarantee:** the shutdown-signalling and
  invalidation contract (§3.4: events → update → render, unidirectional) stays
  normative; no RFC may introduce component-to-terminal writes or sibling-state
  mutation that would widen the race surface.
- **Verification:** design review of dispatch-order and shutdown-signal
  semantics in each relevant RFC; later, scripted racy sequences
  (resize-during-teardown, tick-during-error) in headless tests.

### R4. ANSI escape injection from untrusted content

- **Risk:** application renders untrusted strings (filenames, chat messages,
  log lines, user input echo) containing escape sequences; if passed through
  to the terminal unfiltered, they can spoof UI, rewrite scrollback, or abuse
  terminal features (e.g. OSC sequences) on vulnerable terminals.
- **Impact:** High — content-spoofing and terminal abuse; the framework is the
  last line of defence because app authors will assume rendered text is safe.
- **Owner phase:** renderer + backend implementation phases (§3.7, §3.5):
  sanitisation/encoding policy for text-to-cell paths and capability-gated
  sequence emission.
- **Phase 1 must already guarantee:** the seam direction is fixed — the backend
  receives fully-resolved draw instructions and emits only capability-gated
  sequences (§3.5, §3.7, §8.14); components reference semantic tokens, never raw
  ANSI values (§3.8); no RFC may introduce a raw-passthrough text path.
- **Verification:** RFC review rejecting raw-ANSI passthrough proposals; later,
  adversarial-content golden tests (control characters, OSC/DCS sequences in
  input text → expected neutralised output).

### R5. Unbounded buffers (event queue, cell buffer, scrollback)

- **Risk:** bursty input (paste floods, mouse-move storms), huge terminals, or
  runaway producers grow the event queue or cell buffers without bound,
  exhausting memory.
- **Impact:** Medium — local DoS of the application process.
- **Owner phase:** event-propagation (backpressure/queueing policy, §3.4) and
  renderer (buffer ownership, §8.1) implementation phases.
- **Phase 1 must already guarantee:** backpressure/queueing policy and buffer
  ownership are recorded as required design outputs (§3.4 owns the policy;
  §8.1 requires designs to state ownership and bounds discipline); RFCs that
  defer them must do so with explicit tracking, never silently.
- **Verification:** conformance checklist (§11: no §8 consideration silently
  dropped); later, bounded-queue tests and large-terminal allocation benchmarks.

### R6. Unicode width errors (grapheme clusters, double-width, emoji)

- **Risk:** width miscalculation (counting bytes/`char`s instead of shaped
  grapheme cells; mishandling CJK/emoji double width) drifts columns, breaks
  truncation/wrapping, and misplaces the cursor — corrupting the display and
  potentially misaligning security-relevant content (e.g. confirmation prompts).
- **Impact:** Medium — display corruption; integrity of what the user *thinks*
  they see.
- **Owner phase:** renderer + layout implementation (§8.3, §8.4; shaping
  authority and width-table source decisions).
- **Phase 1 must already guarantee:** the shaping-then-layout ordering rule
  (§3.6: layout consumes measured sizes, never measures text) and the
  requirement that designs name their width authority (§8.3/§8.4) stay
  normative; no layout design may measure text itself.
- **Verification:** RFC review against §3.6/§8.3/§8.4; later, golden tests with
  adversarial strings (ZWJ sequences, combining marks, CJK, emoji) at fixed
  widths.

### R7. Tiny / zero-size terminals

- **Risk:** zero- or one-cell dimensions (piped output, collapsed panes) cause
  division by zero, underflowing subtractions, or panics in layout math.
- **Impact:** Medium — crash on degenerate but entirely reachable environments.
- **Owner phase:** layout implementation (overflow and minimum-size policy, §3.6).
- **Phase 1 must already guarantee:** minimum-size/overflow policy is a required
  layout design output (§3.6 owns it; §8.9 requires the dirty/invalidation rule
  alongside); `unwrap`/arithmetic-panic discipline (QUALITY_STANDARDS §2)
  applies from the first line of layout code.
- **Verification:** later, degenerate-geometry tests (0×0, 1×1, 0×N, N×0) for
  every layout path; RFC review confirming the policy is specified, not assumed.

### R8. Resize races (size change mid-frame)

- **Risk:** terminal resized between measure and flush: frame laid out for size
  A is drawn into size B, producing torn output, out-of-bounds writes, or
  panics on stale regions.
- **Impact:** Medium — torn or crashing display on an everyday user action.
- **Owner phase:** lifecycle + renderer implementation (§8.5: who observes
  resize, drop-vs-complete policy, re-layout → re-render ordering).
- **Phase 1 must already guarantee:** §8.5's three questions (observer,
  drop/complete, ordering guarantee) are mandatory content for any lifecycle or
  renderer RFC; no implementation may assume a fixed terminal size across a frame.
- **Verification:** RFC review against §8.5; later, scripted resize-mid-frame
  sequences against the test double asserting either a clean drop or a
  complete-then-relayout frame, never a torn write.

### R9. Unsupported features / capability overreach

- **Risk:** framework emits sequences the terminal does not support (truecolor
  on 16-colour terminals, Kitty-protocol keys where unsupported, mouse modes
  never requested), producing garbage output or capturing input the user never
  consented to (mouse reporting hijack).
- **Impact:** Medium — garbled display; unexpected input capture is a mild
  integrity/consent issue.
- **Owner phase:** platform integration + styling implementation
  (§3.10, §3.8, §8.6, §8.14): capability detection, degradation ladder,
  token-resolution fallback chain.
- **Phase 1 must already guarantee:** capability profiles are plain data
  threaded explicitly into style resolution and rendering, never sniffed ad hoc
  mid-frame (§8.14); the truecolor → 256 → 16 fallback chain and the key/mouse
  encoding ladder with degradation (§8.6) are required design outputs; mouse
  reporting modes are opt-in per design, never ambient.
- **Verification:** RFC review against §8.6/§8.14/§3.8; later, per-tier snapshot
  tests (token resolution at each capability tier) and tests asserting no
  mouse-reporting enable sequence without explicit opt-in.

## Phase-1 guarantee summary

Phase 1 writes no mitigations (no code exists). Its security contribution is
entirely structural: keep the seams that make later mitigation possible
narrow and normative — backend seam direction (R2, R4, R9), lifecycle restore
contract (R1, R3, R8), shaping-before-layout ordering (R6), explicit capability
profiles (R9), bounded-buffer and minimum-size policy slots (R5, R7) — and
reject any RFC that narrows, bypasses, or silently defers them.

## Out of scope (noted, not owned here)

Vulnerability reporting, supported-version policy, and disclosure timelines
belong to `SECURITY.md`. Proposals to change that file are offered in the
phase handoff, never edited in place.
