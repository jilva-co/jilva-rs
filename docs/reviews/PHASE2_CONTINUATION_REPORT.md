# Phase 2 continuation report — 2026-10-17

## A. Executive summary

Status: **READY FOR MAINTAINER REVIEW** on `phase2-terminal-engine`,
[PR #8](https://github.com/jilva-co/jilva-rs/pull/8). No merge, release,
publication, version increase, or Phase 3 implementation was performed.

The continuation completed an existing uncommitted implementation. At entry,
the branch pointed at main `30ea907`; prior work already contained draft
governance acceptance, shared data, a Crossterm backend, a large scripted
double, and tests. It did not pass an all-target build: the facade imported a
private `KeyEncoding`, tests/examples still imported removed
`CapabilityProfile`, the demo used only a fake, and lifecycle/error/output
safety paths were incomplete. README, CI toolchain selection, acceptance
checklist and development instructions were stale.

Those gaps are corrected. The result is a real synchronous terminal backend,
typed input/output, conservative capabilities, tracked setup/restoration,
scripted failure testing, separate real/headless demos, and actual three-OS
CI evidence. Independent audit has zero unresolved findings.

## B. Governance and RFC decisions

The owner's supplied plan explicitly authorized expedited maintainer-driven
acceptance without a seven-day external wait. The 2026-10-17 decision records
that authorization, technical source checks and evidence limitations. Existing
2026-10-16 entries and Discussion timestamps remain historical records;
neither community consensus nor previously unverified review authorship is
invented. GitHub API confirmed Discussions #4/#5/#6 exist with zero comments
at inspection, main's SHA, and merged PRs #1/#2/#7.

RFC-0001/0002/0003 remain accepted as design authority, with explicit current
revalidation. The acceptance checklist distinguishes checked design decisions
from future implementation obligations. Core owns shared data; terminal may
depend on core; core has no terminal library, environment access or OS branches.
The seam now accurately specifies standard I/O errors, source readiness,
bounded filtering, whole-batch validation, non-atomic writes and discard on
attempted flush. Terminal focus is distinct from component focus.

Deferred items retain existing phase owners: grapheme cell/width authority
(Phase 3), layout policies (Phase 4), runtime/effects/keyboard enhancement
negotiation/refresh queue (Phase 5), and palette/theme quantization/contrast
harness (Phase 6). Speculative RFC sketches are not implemented contracts.
See [decision tracker](RFC_DECISION_TRACKER.md), [checklist](RFC_ACCEPTANCE_CHECKLIST.md)
and [backend contract](../PHASE2_BACKEND_CONTRACT.md).

## C. Five-subagent report

Exactly **five real specialized subagents** were launched. Independent work
overlapped in waves within the session's concurrency limit. File ownership
was exclusive during each agent's assignment; root integrated released files,
fixed cross-file issues and ran final verification. This was not simulated
delegation.

| Agent | Responsibility and owned files | Completed work and verification | Limits |
|---|---|---|---|
| 1 — architecture/governance | RFC documents; decision tracker/checklist; architecture/API/contract/roadmap | Recorded current authorization, revalidated acceptance, classified deferrals, reconciled dependencies and error/output semantics; scoped diff check passed | Did not reproduce old prototype timings or certify old review independence |
| 2 — Crossterm/lifecycle | `crates/jilva-terminal/src/crossterm_backend.rs`, terminal manifest | Real backend, injectable production state machine, resource flags/lease, mode policy, buffering/decoder/ANSI16, fault tests and ignored PTY smoke; targeted 40 tests + 2 doctests passed before integration | Final message was interrupted by an account limit after saving; root and auditor independently reran the integrated gates. Physical terminal matrix remains limited |
| 3 — events/capabilities | Core capabilities/events/output/lib; terminal detection | Key kinds/modifiers, mouse/focus/paste data, resize versions, deterministic detection/report/overrides, control-text validation; 33 core unit + 23 doctests passed at its handoff | Root added the final geometry test/helper, yielding 34 core tests; detector tests verified during integration |
| 4 — headless/CI | Test double, backend input/session integration tests, testing guide, CI workflow | Typed recording, scripted FIFO, bounded filtering, retryable cleanup, validation, injected errors and pinned three-OS workflow; focused run passed 50 tests (38 owned) | Rustfmt/Clippy required root's sandbox escalation; later audit corrections were integrated and retested |
| 5 — independent auditor | `PHASE2_TERMINAL_ENGINE_AUDIT.md` only | Challenged implementation, requested eight corrections and verified their final source; independently ran check/test/fmt/Clippy/strict docs/features/demo | PTY observations attributed to root; remote CI observed separately by root |

Root additionally migrated the facade and retained version example, added
feature wiring and demos, shared geometry validation, canonical error kinds,
final documentation, commits, PR and remote CI verification. No additional
engineering subagents or extra workspace crates were created.

## D. Relevant repository structure

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml
.github/workflows/ci.yml
crates/
  jilva/src/lib.rs                  facade, feature-gated real backend
  jilva-core/src/
    capabilities.rs                pure profiles, versions and helpers
    events.rs / output.rs          typed input and validated output
    component.rs / layout.rs / lifecycle.rs / renderer.rs / style.rs / error.rs
  jilva-terminal/src/
    backend.rs                     shared trait
    capabilities.rs                detection and evidence
    crossterm_backend.rs           terminal operations and tested state machine
    test_double.rs                 deterministic headless backend
examples/backend_demo.rs / headless_demo.rs / version.rs
tests/backend_io.rs / backend_session.rs / facade_smoke.rs
docs/PHASE2_BACKEND_CONTRACT.md / PHASE2_TERMINAL_ENGINE.md / PHASE2_TESTING.md
docs/reviews/PHASE2_TERMINAL_ENGINE_AUDIT.md / PHASE2_CONTINUATION_REPORT.md
```

The existing three-crate topology is preserved; no renderer or widget module
was implemented as part of this milestone.

## E. Terminal engine architecture

`Backend` exposes name, enter, leave, cached size/capabilities, poll/read,
submit and flush. Both implementations use core-owned types and standard
`io::Result`. The real backend owns standard handles and an internal
injectable I/O interface used by the production lifecycle tests. A documented
atomic lease prevents simultaneous cooperating Jilva owners of Crossterm's
process resources. The real backend is `!Send`/`!Sync` so input/cleanup stay
on one thread; no async runtime is required.

Capabilities distinguish color/input/screen domains and per-domain versions.
Detection sources label defaults, environment policy/inference, explicit
overrides and redirected output. `NO_COLOR` includes empty values under
explicit Jilva policy. Redirection disables all attributes and interactive
entry is rejected. Unknown hosts do not enable optional modes. Mouse, focus
and paste are explicit options filtered by host compatibility hints; no
active probing or enhanced keyboard negotiation is claimed.

Input preserves supported key identities, press/repeat/release and reported
modifiers, mouse reports, valid resize, terminal focus and untrusted paste.
Reads filter at most 64 units, propagate actual I/O errors, and cache consumed
resize geometry without doing layout. Output carries complete strings and
resolved attributes; text rejects ESC/C0/DEL/C1. Whole batches and screen
bounds are validated before buffering and again after resize before emission.
Classic ANSI16 commands are emitted for explicit ANSI16 values. Callers
supply colors resolved for the selected tier; semantic theme resolution is
future work. Output is buffered and attempted batches are discarded on failure
because an OS write may already have delivered an unknown prefix.

## F. Terminal restoration

The tested production state machine tracks attempted/acquired resources,
including uncertain partially written modes. Initialization failure rolls
back; cleanup attempts all dirty modes in reverse order and preserves failed
flags for retry. Cleanup immediately invalidates the usable session. Primary
error kind/source remains available alongside secondary cleanup context.

Tests cover every setup-stage failure, rollback failure, cleanup failure and
retry, input poll/read errors, output/flush failure, ordinary explicit leave,
Drop and panic unwinding. Headless tests verify the actual production state
machine, not only the double's log. Separate Windows ConPTY tests checked raw
mode restoration on explicit leave and panic Drop, and reacquisition afterward.

Drop is best effort. Forced kill/abort/power loss/double panic and failed OS
output cannot guarantee restoration. Prior cursor visibility is not queried;
cleanup establishes a visible baseline. Upstream raw-mode restoration has
documented platform limits. If final Drop cleanup fails, the real terminal
lease stays held for that process, refusing uncertain future acquisitions.
Other terminal libraries/processes must cooperate with ownership.

## G. Actual testing results

| Configuration | Unit tests | Integration tests | Doctests | Ignored | Failed |
|---|---:|---:|---:|---:|---:|
| Default/all features, ordinary headless suite | 74 (34 core, 40 terminal) | 37 (17 input/output, 13 session, 7 facade) | 27 (2 facade, 23 core, 2 terminal) | 1 real-terminal test | 0 |
| No default features, repeated overlapping suite | 47 (34 core, 13 terminal) | 37 | 26 | 0 | 0 |
| Explicit Windows ConPTY smoke | 1 opt-in terminal test | — | — | 0 in this invocation | 0 |

The default result is **138 passed + 1 ignored**, counting doctests; the
ignored test also passed when explicitly invoked under a PTY. Repeated feature
runs and independent reruns are not added as new distinct tests. Both demos
build; the headless demo runs with and without the terminal dependency.

Actual Windows ConPTY sessions received an initial 80×24 resize and the `h`
key. Exit via `q`, Escape (with mouse/focus/paste options enabled), and Ctrl+C
returned exit 0; the enclosing shell resumed. Setup/teardown output was
observed. Physical window resizing, mouse gestures, real focus/paste actions,
visual styling and an interactive Linux/macOS emulator matrix were **NOT RUN**.

## H. Verification

| Gate | Result and evidence |
|---|---|
| `cargo check --workspace --all-targets --locked` | PASS, Windows local and three-OS CI |
| `cargo test --workspace --all-targets --locked` | PASS, 111 local tests; one opt-in test ignored normally |
| `cargo fmt --all -- --check` | PASS after final assertion formatting correction |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS; all/no-default features also checked |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --locked` | PASS |
| Strict `cargo test --workspace --doc --locked` | PASS, 27 doctests |
| Feature builds/tests | PASS, all features and no default features |
| Linux CI | PASS, Ubuntu quality job in run 39000026013 |
| Windows CI | PASS, Windows quality job in run 39000026013 |
| macOS CI | PASS, macOS quality job in run 39000026013 |
| Real terminal safety smoke | PASS, Windows ConPTY explicit leave + panic unwind + reacquire |
| Physical terminal visual matrix | NOT RUN; limitations remain documented |

[CI run 39000026013](https://github.com/jilva-co/jilva-rs/actions/runs/39000026013)
completed successfully for source commit
`9fef589ce5d069409e784edd96b9025a42a8b7ae`: formatting and all three quality
jobs succeeded. Each quality job exercised default/all/no-default checks,
Clippy, headless tests, demos, strict docs and doctests. This is observed
completion, not merely configured YAML. A subsequent documentation-only
commit carries this report and is checked again on the PR.

The earlier [run 38999885012](https://github.com/jilva-co/jilva-rs/actions/runs/38999885012)
passed every OS quality job but failed formatting for two test assertions.
The actual failure log matched the local Rustfmt correction committed in
`9fef589`; later CI above passed. Sandbox-only Rustfmt/Clippy access errors
were resolved by authorized escalation; toolchain and lint settings were not
weakened.

## I. Security and quality audit

[Independent audit](PHASE2_TERMINAL_ENGINE_AUDIT.md): **PASS** with zero
unresolved BLOCKER/HIGH/MEDIUM/LOW findings. Eight defects were corrected:
one BLOCKER, five MEDIUM and two LOW. They covered the stale version example,
cleanup usability, rollback diagnostics, staged-read parity, error-kind
parity, fresh snapshot versions, modern protocol inference and paste allocation.

Control injection, validation atomicity, size overflow/stale resize, real
production rollback, source preservation, ownership, bounded reads, batch
ordering, partial writes and non-panicking Drop have substantive tests. The
workspace forbids unsafe Rust. No speedup or throughput number is claimed;
source/tests verify batching, bounded work, caching and avoiding redundant
paste clones. Upstream platform operations may internally use unsafe code.

## J. Dependency changes

One new direct third-party dependency: optional **Crossterm 0.29.0**, MIT,
confined to terminal. The actual resolved API/features and Rust minimum were
inspected; `events`, `windows`, and `bracketed-paste` support synchronous
input, native console integration and explicit paste mode. Event-stream,
serialization, clipboard and derive convenience features are disabled.
Core has no dependencies. The no-default graph contains only the three
workspace crates. `Cargo.lock` is committed for reproducible demos/CI;
library consumers retain their own lockfiles. No unrelated dependency or
workspace crate was introduced. Project source license and version stay MIT
and `0.0.0`.

## K. Pull request and commits

- PR: [#8](https://github.com/jilva-co/jilva-rs/pull/8).
- Branch: `phase2-terminal-engine`; base: `main`.
- `85c4c0d`: governance acceptance revalidation and backend contract.
- `008708e`: core event/capability/output data and validation.
- `3b29fb3`: real backend, headless backend, facade integration, tests, demos,
  lockfile and CI.
- `9fef589`: final formatting correction and inactive-session documentation.
- The final documentation commit contains this report, implementation/testing
  guides, independent audit, current roadmap and user-facing status.

Main review areas: production state machine, resource flags/lease and failure
policy; output validation and partial delivery; capability inference and
platform limitations; facade API and no-default behavior. The maintainer must
review the PR and actual current-head checks before merging. No automatic
merge is performed.

## L. Roadmap progress

Phase 2 implementation and verification exit criteria are met on this branch;
the PR is ready for maintainer review. Foundation and RFC history are preserved.
Interactive evidence is limited and separately labeled, rather than being
inferred from headless CI. No unrelated phase is marked complete.

## M. Next existing milestone

**Phase 3 — Rendering Engine** remains planned and was not started. Its
prerequisites include replacing provisional `Cell.symbol: char` with a
grapheme-capable model, naming width/segmentation authorities, defining
clipping/wide-cell behavior, and agreeing buffer/diff/damage semantics before
implementation. Phase 2 string-based output does not entrench scalar cells.
