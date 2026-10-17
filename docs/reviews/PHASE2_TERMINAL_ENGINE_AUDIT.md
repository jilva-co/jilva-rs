# Phase 2 terminal engine — independent audit

Date: 2026-10-17 (Asia/Tashkent). Auditor: the fifth specialized engineering
subagent, separate from the implementation owners. Scope: the actual working
tree on `phase2-terminal-engine`, the maintainer's supplied Phase 2 directive,
the recorded backend contract, and the resolved Crossterm 0.29.0 source.

**PASS for the reviewed implementation and local Windows quality gates.**
Unresolved findings: **0 BLOCKER, 0 HIGH, 0 MEDIUM, 0 LOW**. Eight findings
were corrected and verified: 1 BLOCKER, 5 MEDIUM, 2 LOW. This is the independent
code-audit gate; it does not declare the entire milestone complete or certify
an unexecuted cross-platform CI job or every physical terminal.

Post-audit integration note by the orchestrator: Ubuntu, Windows, macOS and
formatting subsequently completed with `success` for source commit `9fef589`
in [CI run 39000026013](https://github.com/jilva-co/jilva-rs/actions/runs/39000026013).
This remote observation is separate from the auditor's independent local
executions below. Additional PTY shortcut checks and milestone status are
recorded in [the continuation report](PHASE2_CONTINUATION_REPORT.md).

The final source review covers implementation commit
`3b29fb3e4d90e030b2f6548dba9809b50ce3e744` plus the inspected integration
documentation and formatting corrections present before report delivery.

## Independence and evidence

The auditor edits only this report. Implementation corrections are requested
from the relevant file owner and then independently inspected. Earlier RFC
review files are historical inputs; their presence does not prove a newly
independent review, and their historical compiler results are not counted as
this audit's executions. The current authorization is dated 2026-10-17; no
community approval or backdated decision is inferred.

## Findings recorded during this audit

Severity describes the defect before its correction. A resolved finding is
not an outstanding risk. Locations below identify the corrected source after
formatting; the evidence describes the earlier behavior observed during this
audit.

### A1 — MEDIUM: partially restored headless sessions remained usable

- Location: `crates/jilva-terminal/src/test_double.rs:1034` (`leave_inner`).
- Evidence: an injected cleanup failure retained `entered = true` even after
  the remaining cleanup steps restored raw mode and alternate screen. The
  five live-session I/O methods therefore accepted work on a partially
  dismantled session, masking invalid application recovery behavior.
- Required correction: invalidate the usable session before restoration;
  preserve failed-resource flags separately for `leave`/Drop retry and reject
  new entry until pending cleanup finishes.
- Verification: **RESOLVED**. Independent inspection confirmed invalidation
  before restoration. The auditor's no-default-feature workspace test run
  passed `failed_cleanup_attempts_later_modes_and_retains_the_failed_mode_for_retry`,
  including rejection of all five live I/O methods after partial cleanup.

### A2 — MEDIUM: injected rollback failure was hidden from the returned error

- Location: `crates/jilva-terminal/src/test_double.rs:1103` (partial-failure
  branch of `enter`).
- Evidence: the return value retained the primary error kind but only said
  rollback was attempted. A simultaneous rollback failure was visible solely
  through the concrete double's private-style inspection log, unlike the
  backend error contract's requirement for secondary cleanup context.
- Required correction: preserve the primary kind and include explicit
  rollback failure context in the returned error.
- Verification: **RESOLVED**. Returned errors retain the original kind and
  report the rollback kind. The independently run
  `failed_rollback_blocks_reenter_until_cleanup_retries_the_dirty_mode` passed
  assertions for both errors, blocked re-entry, and successful cleanup retry.

### A3 — MEDIUM: newly scripted input required an extra explicit poll

- Location: `crates/jilva-terminal/src/test_double.rs:1233` (staged script
  promotion in `read`); `tests/backend_io.rs:138`.
- Evidence: scripts injected into an active session were promoted only by
  `poll`. A direct `read` could not observe them, although the real backend's
  read performs its own readiness check and the contract promises the next
  poll/read observation. This made construction-edge substitution depend on
  an undocumented application polling pattern.
- Required correction: promote newly scripted input at a successful read
  observation as well, after the injected-error check so a failing read
  consumes or promotes nothing.
- Verification: **RESOLVED**. Source promotes staged input after the session
  and injected-error checks. The independently run
  `input_added_during_a_session_is_visible_to_a_successful_poll_or_read`
  passed direct-read delivery, failure nonconsumption, and FIFO assertions.

### A4 — BLOCKER: the retained version example imported a removed profile

- Location: `examples/version.rs:11`.
- Evidence: the auditor's `cargo check --workspace --all-targets
  --no-default-features --locked` exited 1 with E0432: the example imported
  `jilva::CapabilityProfile`, which the canonical profile migration removed.
  Library-only tests could not establish the required all-target build gate.
- Required correction: migrate the existing facade example to canonical
  `TerminalCapabilities` data and rerun the all-target quality gates.
- Verification: **RESOLVED**. `examples/version.rs:11` now imports canonical
  `TerminalCapabilities`; its constructor is used at line 31. Independent
  all-target check and test reruns with default features disabled exited 0.

### A5 — MEDIUM: real and scripted sessions exposed different error kinds

- Locations: `crates/jilva-terminal/src/test_double.rs:1081`,
  `crates/jilva-terminal/src/test_double.rs:1378`,
  `crates/jilva-terminal/src/crossterm_backend.rs:394`,
  `docs/PHASE2_BACKEND_CONTRACT.md:329`.
- Evidence: real entry used `AlreadyExists` for double entry, `WouldBlock`
  for pending cleanup, and `NotConnected` for inactive I/O. The double returned
  `Other` for all three; application recovery branches therefore changed with
  the chosen backend despite the documented common error contract.
- Required correction: choose one precise mapping, align the double and its
  assertions, and record the actual public contract.
- Verification: **RESOLVED**. The double, production
  backend, contract, and integration assertions now use the same three kinds.
  Final integrated rerun is recorded below.

### A6 — LOW: the first production snapshot counted an unobserved baseline

- Location: `crates/jilva-terminal/src/crossterm_backend.rs:495`.
- Evidence: first entry refreshed `conservative(0)`, making changed domains
  start at version 2, while the fresh-detection contract and scripted backend
  start at 1. Later deltas worked, but source semantics and substitution differed.
- Required correction: construct the first successful snapshot as fresh data;
  preserve monotonic refresh counters for later sessions and consumed resizes.
- Verification: **RESOLVED**. First entry now uses
  `TerminalCapabilities::new(..., 1)`;
  `snapshots_are_per_instance_cached_and_advance_only_for_session_or_resize`
  explicitly asserts all initial domain versions are 1. Final execution below.

### A7 — MEDIUM: basic terminal hints implied unrelated optional protocols

- Location: `crates/jilva-terminal/src/crossterm_backend.rs:196`.
- Evidence: the initial common allowlist treated basic `ansi`, `vt100` and
  `vt220` hints as evidence for alternate screen, mouse, focus, and paste.
  Basic terminal identity alone does not establish xterm optional protocols.
- Required correction: narrow per-mode policy and retain the distinction
  between inferred/requested mode support and actual protocol negotiation.
- Verification: **RESOLVED**. The optional-mode list now
  contains modern terminal-family hints; non-Windows `TERM=linux` only gains its separate cursor
  hint. Unknown/dumb terminals have no optional setup. Windows native support
  and paste's ANSI hint are treated separately. No probe or protocol-support
  guarantee was introduced; final tests are recorded below. Source inspection
  establishes the allowlist change; no legacy-emulator session was run.

### A8 — LOW: production input cloned paste before checking its mode

- Location: `crates/jilva-terminal/src/crossterm_backend.rs:556`.
- Evidence: `read` owned a Crossterm paste string but passed it through the
  borrowed decoder, cloning the complete payload before checking whether
  paste was enabled. Disabled paste paid for a copy solely to discard it.
- Required correction: gate optional raw events before decoding and move
  delivered paste payloads into the framework event.
- Verification: **RESOLVED by source inspection**. The owned production path
  now gates first and moves `String`; the public borrowed pure decoder still
  clones when its caller explicitly requests that conversion. No allocation
  benchmark or latency improvement is claimed.

The orchestrator additionally aligned modeled cleanup of alternate screen
before raw mode, matching production's raw-last restoration discipline. The
double models injected modes rather than proving native terminal state.

## Architecture and scope inspection

`cargo metadata --no-deps --format-version 1 --locked` exited 0. The workspace
still has three crates. The facade depends on terminal and core; terminal
depends on core; core has no dependencies. Shared event, capability, geometry
and command types have one canonical owner in core. No dependency cycle,
async runtime, platform I/O in core, or new crate was found.

The existing `Cell.symbol: char` placeholder is unchanged. Output commands
carry complete `String` segments and do not create a one-scalar-per-cell
contract. Color adaptation maps already resolved colors to supported tiers;
it introduces no semantic theme resolver, renderer, diff, layout engine,
component reconciliation, widgets, or application runtime. Grapheme shaping,
width and clipping authority remain a Phase 3 prerequisite.

## Production mechanism examined

- `crossterm_backend.rs:378` / `:454`: the same internal `Session` drives
  production and injected fault tests. Setup marks a requested mode dirty
  before its I/O, covering an uncertain partially written command. Restoration
  invalidates the session, discards pending output, attempts all dirty modes
  in reverse order, and clears only successful flags. Input/output stays
  rejected until a new clean session enters.
- `crossterm_backend.rs:91` / `:117`: contextual errors preserve the primary
  `io::Error` and kind; secondary cleanup errors remain in the wrapper and
  display. Setup rollback and multi-error teardown are tested independently
  of the headless backend's simplified mode simulation.
- `crossterm_backend.rs:371` / `:419`: the documented atomic lease protects
  Crossterm's process-global resources. Preflight rejects redirected standard
  handles, detectable existing raw mode and zero geometry before mutation.
  Every clean preflight/rollback/leave releases ownership; dirty cleanup
  retains it. A non-owner Drop cannot release another session's lease.
- `crossterm_backend.rs:542`: read performs zero-time readiness checks and
  examines at most 64 source units. Unsupported and disabled reports are
  counted; actual poll/read errors preserve kind and source. Resize consumption
  updates cached size and screen causality only. Snapshot reads and size reads
  do not repeat environment detection or terminal size queries.
- `jilva-core/src/output.rs:106` / `:133`, production `:589` / `:600`:
  all commands are validated before any batch append and again before
  emission. Direct enum construction cannot bypass C0/DEL/C1 rejection.
  Geometry uses wider arithmetic; a consumed shrink rejects the entire stale
  batch. Text width remains explicitly the caller's responsibility.
- `crossterm_backend.rs:316` / `:600`: output batches are buffered until an
  explicit flush. An attempted batch is consumed on failure; `BufWriter`
  is dismantled with `into_parts` so its Drop cannot silently replay an
  unknown prefix. Native Windows commands retain their platform path and
  ordering. Pure test sinks call `write_ansi` directly and cannot mutate the
  host console through Crossterm's Windows fallback.
- `crossterm_backend.rs:886` / `:995`: ANSI16 uses classic SGR codes, and
  rectangular erasure reaching the final column avoids printing the bottom
  right cell, preventing clear-induced scroll. Monochrome suppresses direct
  RGB/palette output and decorative attributes, retaining only permitted
  structural reverse. Other colors must arrive resolved for the profile tier;
  the backend does not perform semantic theme quantization.
- `crossterm_backend.rs:637`: `PhantomData<Rc<()>>` makes the public backend
  neither `Send` nor `Sync`, keeping polling and reading on the same thread;
  a compile-fail doctest covers the transfer restriction. The final production
  section has no `unsafe`, `unwrap`, `expect`, or intentional panic.

The tests inspect every setup-failure stage and reverse rollback, several
simultaneous rollback/cleanup errors, dirty-only retry, lease contention,
Drop retry, panic unwind, source errors, input filtering, output injection,
stale geometry, real buffering partial-write behavior and encoder goldens.
No ordinary automated test opens a real terminal. The native smoke test is
explicitly ignored by default.

## Verification log

Commands run by this auditor on Windows `x86_64-pc-windows-msvc`, pinned Rust
1.98.1 (`48a229cea`, LLVM 22.1.8):

| Command | Exit | Result |
|---|---:|---|
| `rustc --version --verbose` | 0 | Pinned toolchain and host confirmed |
| `cargo metadata --no-deps --format-version 1 --locked` | 0 | Three-crate acyclic ownership inspected |
| `cargo check --workspace --all-targets --no-default-features --locked` | 1 | E0432 in the stale version example (A4); correction requested |
| `cargo check --workspace --all-targets --no-default-features --locked` (after A4) | 0 | All targets compile; `CARGO_INCREMENTAL=0` |
| `cargo test --workspace --all-targets --no-default-features --locked` | 0 | 84 passed: 34 core units, 13 terminal units, 37 facade integrations; 0 failed, 0 ignored |
| `RUSTDOCFLAGS="-D warnings" cargo test --workspace --doc --no-default-features --locked` | 0 | 26 passed (2 facade, 23 core, 1 terminal); 0 failed, 0 ignored |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-default-features --no-deps --locked` | 0 | Strict docs for all three crates |
| `cargo check --workspace --all-targets --locked` | 0 | All default-feature targets compile, including the real demo |
| `cargo test --workspace --all-targets --locked` | 0 | 111 passed: 34 core units, 40 terminal units, 37 facade integrations; 0 failed, 1 intentionally ignored native test |
| `cargo fmt --all -- --check` | 0 final | First sandbox attempt: exit 1, OS access denied. Elevated check then found two unformatted assertions; owner formatted them. Final elevated read-only check exited 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 final | Sandbox configuration discovery first exited 1 (access denied); elevated gate exited 0 |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` | 0 | Strict default-feature docs for all three crates |
| `RUSTDOCFLAGS="-D warnings" cargo test --workspace --doc --locked` | 0 | 27 passed: 2 facade, 23 core, 2 terminal including the non-Send compile-fail test; 0 failed, 0 ignored |
| `cargo check --workspace --all-targets --all-features --locked` | 0 | All implemented features compile |
| `cargo clippy --workspace --all-targets --no-default-features --locked -- -D warnings` | 0 | Elevated gate; headless feature configuration |
| `cargo test --workspace --all-targets --no-default-features --locked` (final aligned source) | 0 | 84 passed; 0 failed, 0 ignored; same 34 + 13 + 37 breakdown |
| `cargo run -p jilva --example headless_demo --no-default-features --locked` | 0 | Scripted `q` decoded and typed output captured without a terminal library |
| `cargo tree -p jilva --no-default-features --locked` | 0 | Only the three workspace crates, no external dependency |
| `git diff --check` | 0 | No whitespace errors |

Final compiler/test/lint commands used `CARGO_INCREMENTAL=0`, avoiding the
cache hard-link warnings seen in earlier sandbox incremental builds without
changing test semantics. Rustfmt and Clippy escalation was needed for local tool
configuration access; no approval rejection or source diagnostic was hidden.
The default test count plus doctests is **138 passed, 0 failed, 1 ignored**.
The no-default configuration repeats many of those tests; its 84 passing tests
are not added to claim additional unique test cases. Example test harnesses
contain 0 tests and are build coverage, not extra behavioral tests.

## Separately attributed terminal and CI evidence

The orchestrator reported the following actual Windows ConPTY executions on
2026-10-17. The auditor inspected the implementation and native test source,
but did not personally launch these live sessions:

- `TERM=xterm-256color cargo run -p jilva --example backend_demo --locked`,
  launched with an interactive PTY: an initial 80×24 resize was delivered;
  injected `h` appeared as typed key input; injected `q` exited 0. Captured
  protocol output included alternate-screen entry/cursor hide and cursor
  show/alternate-screen exit. Default optional input modes were off.
- Explicit execution of ignored
  `real_terminal_enter_leave_and_panic_unwind_smoke` in ConPTY: 1 passed,
  exit 0. The test asserts raw mode false → true → false across explicit
  leave, false after panic-unwind Drop, and successful lease reacquisition.

These are limited native PTY and state-query results. They do not prove GUI
appearance, a physical window resize, mouse/focus/paste integration, or every
emulator's interpretation of protocol bytes.

`.github/workflows/ci.yml` configures Ubuntu, Windows and macOS headless
quality jobs, all/default/no-default builds, strict lint/docs/doc tests and a
separate formatting gate. At audit delivery, remote job outcomes are
**UNVERIFIED by this auditor**. The orchestrator must report observed URLs,
commit IDs and conclusions separately; a configured matrix is not a pass.

## Dependency and performance assessment

Crossterm 0.29.0 is the only direct external dependency, optional and confined
to `jilva-terminal`. Its resolved local source confirms MIT licensing, Rust
1.63 minimum, synchronous methods and the selected `events`, `windows` and
`bracketed-paste` features. The lockfile pins the resolved transitive graph.
No event-stream, async executor, clipboard, serialization or Unicode-width
dependency was introduced. The workspace forbids unsafe code; dependencies
can use platform implementation code. This is a dependency-surface review,
not a claim of an exhaustive transitive vulnerability audit.

Session capability detection occurs once per successful entry. Input performs
bounded source work, and optional events are gated before payload conversion.
Output preserves FIFO commands, batches writes and exposes explicit flush.
Pending commands intentionally own cloned strings; clear-region generation
allocates one row of spaces per rectangle. These are visible implementation
costs, without an invented throughput, allocation, or latency measurement.

## Limits already established

- No interactive emulator or PTY has been exercised by this auditor. A fake
  proves state transitions and attempted restoration, not physical terminal
  restoration or visual quality.
- A final Drop that still cannot restore every mode deliberately retains the
  global lease. Subsequent real-backend entry is refused for that process's
  lifetime; cleanup uncertainty is not silently reassigned. Explicit `leave`
  retries while the backend remains available are the recovery path.
- Restoration after failed OS output, terminal disappearance, forced kill,
  abort, power loss or double panic cannot be guaranteed. Drop is best effort;
  explicit shutdown is necessary to inspect cleanup errors.
- A universally queryable prior cursor visibility is unavailable; restoring
  a documented visible baseline is different from claiming exact prior state.
- Crossterm's Unix `is_raw_mode_enabled` checks its own saved-termios state,
  not arbitrary external termios changes (`terminal/sys/unix.rs:30`). Windows
  raw disable sets line/echo/processed bits on rather than restoring a saved
  exact input-mode snapshot (`terminal/sys/windows.rs:43`). Other terminal
  owners must cooperate; the backend lease cannot police other libraries or
  processes. These are inspected upstream limitations, not successful native
  platform experiments.
- Capability environment hints and successfully written protocol commands
  do not prove emulator support. Conservative defaults, explicit options and
  evidence labels must remain distinct from protocol negotiation.
- No throughput or latency benchmark was run; no performance number is
  claimed. Source inspection can establish batching and bounded filtering,
  not a measured speedup.
- Linux and macOS have not been executed locally. Final GitHub Actions
  outcomes must be linked separately and cannot be inferred from the matrix
  configuration or from Windows results.
