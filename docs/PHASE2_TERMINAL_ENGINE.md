# Phase 2 — Terminal Engine

This milestone implements a synchronous backend boundary. It does not provide
an application runtime, renderer, layout solver, widgets, or theme system.
The workspace remains unreleased at version `0.0.0`, edition 2024, with
Rust 1.98.1 pinned. The continuation and acceptance decision is dated
2026-10-17; earlier Discussion and draft history remains in the RFC records.

## Crate ownership

```text
application -> jilva -> jilva-terminal -> jilva-core
                  \----------------------^
```

`jilva-core` owns terminal-independent event, capability, geometry, and output
data. It has no external dependencies or OS branches. `jilva-terminal` owns
environment collection, terminal-handle observation, Crossterm integration,
resource cleanup, and the scripted backend. `jilva` only re-exports the
supported surface. Examples and integration tests import that facade.

The default `crossterm` feature enables the real backend. With
`--no-default-features`, the workspace and headless example use only standard
library code and workspace crates. There is no async runtime requirement.

## Backend contract

`Backend` exposes `name`, `enter`, `leave`, `size`, `poll`, `read`, `submit`,
`flush`, and `capabilities`. Fallible operations return `std::io::Result`;
their errors can be converted into the shared framework `Error::Io`.

Each backend owns one session. A second `enter` fails. Leaving a fully
inactive backend succeeds without terminal commands. Observation, input,
and output require an active session. Failed cleanup retains pending
resource state for retry; an incomplete setup is not a usable session.

`poll(timeout)` reports source readiness, which may include input later
filtered by `read`. A ready source can therefore yield `Ok(None)`. Reads
use a zero-time readiness probe and bounded filtering; actual I/O errors
propagate. Crossterm input must not be consumed concurrently outside the
backend. The headless backend never waits for a timeout.

## Input

`InputEvent` represents keys, mouse reports, nonzero resize dimensions,
terminal focus changes, and bracketed paste. These are terminal-origin data;
component focus ownership, event routing, ticking, and messages belong to
the future application runtime.

Keys preserve character strings, special/function keys, modifiers, and
press/repeat/release kinds when reported by the terminal library. Legacy
input cannot reliably distinguish Ctrl+I from Tab or provide release events.
A repeat or release report does not establish that an enhanced keyboard
protocol was negotiated. This milestone does not negotiate Kitty keyboard
enhancements. Unsupported codes are counted and skipped.

Mouse, focus, and paste reporting depend on enabled session options and
terminal support. Mouse reports preserve position, buttons, scrolling,
dragging, and reported modifiers. Keyboard use remains available without
mouse reporting. Paste is untrusted input: it may contain terminal control
characters and must pass ordinary output validation before being displayed.

Consumed resize events update cached geometry and screen causality counters.
They mark layout-relevant data; this milestone does not recalculate layout.

## Capabilities

Core owns `ColorProfile`, `InputProfile`, `ScreenProfile`, and the aggregate
`TerminalCapabilities`. The aggregate carries snapshot identity and per-domain
versions. Detection runs at entry and the backend returns cached snapshots;
resize changes screen geometry/version. Input-mode changes require a new
session. Frame-boundary refresh queues and repaint scheduling are Phase 5
work, not an implemented runtime feature.

`EnvSnapshot` makes environment evidence injectable. `DetectionReport`
distinguishes observations, conservative defaults, environment policy,
environment inference, explicit overrides, and redirected output. No active
escape probes or background-response reads are used. A terminal-program name
alone is insufficient evidence for truecolor.

- Direct `COLORTERM`/`TERM` hints can infer truecolor or a 256-color tier.
  Otherwise interactive color defaults to ANSI16.
- Any presence of `NO_COLOR`, including an empty value, selects monochrome,
  removes decorative attributes, and permits structural reverse video. This
  stronger behavior is explicit Jilva RFC-0003 policy.
- Redirected output forces monochrome and removes every attribute, including
  reverse video. Color overrides cannot bypass redirected-output safety.
- An explicit interactive color override may supersede environment policy.
  Unknown background does not trigger a theme change.

Standalone injected detection is deterministic. Snapshot identities advance
within a backend instance; they are not globally comparable across instances.
Color metadata does not prove mouse, focus, paste, or screen-mode support.

## Typed output

Commands move the cursor, print string segments, set/reset resolved
attributes, change cursor visibility, and clear rectangles. Commands contain
no component state, theme tokens, or application-facing raw ANSI escape hatch.
Strings preserve multi-scalar text; the backend never imposes one scalar per
terminal cell. Grapheme segmentation and width/clipping remain Phase 3 work.

`submit` validates every command before appending any part of a batch.
Printing rejects ESC, C0, DEL, and C1 control characters, including newline,
tab, and carriage return. This applies even when callers construct enum
variants directly. Use typed commands for positioning and formatting;
`OutputCommand::try_print_segment` can validate untrusted text earlier.

Output stays pending until `flush`. A flush attempt discards the attempted
batch even on failure: an OS write can deliver an unknown prefix, so retrying
the same pending buffer automatically would duplicate output. Callers may
explicitly submit a fresh recovery batch. Submission is atomic with respect
to validation; physical terminal output is not atomic.

This boundary executes commands. It does not implement cell buffers, diffing,
damage tracking, automatic Unicode width handling, or theme resolution.
For non-monochrome output, callers supply colors already resolved to the
snapshot's tier. The pure `ResolvedAttributes::downgraded_to` helper is
available for low-level resolved-color fallback; it does not implement the
future role-aware theme quantization policy. The backend masks unsupported
attribute flags and enforces monochrome policy, but otherwise emits resolved
colors literally. ANSI16 values use classic 16-color SGR instructions.

## Restoration and ownership

Production setup and reverse-order cleanup share a tested resource state
machine. Only acquired or potentially partially applied modes are restored.
Cleanup attempts remaining steps even if an earlier restoration fails and
keeps failed resource flags for a later explicit retry or Drop attempt.
Setup failures include rollback diagnostics without hiding the primary error.

Normal shutdown should explicitly call `leave` to inspect failures. Returning
with an error and panic unwinding drop the backend and attempt best-effort
cleanup. Drop does not panic. Cursor restoration uses a documented visible
baseline; a universally queryable prior cursor state is unavailable.

Crossterm terminal modes and input are process resources. A small documented
lease prevents simultaneous Jilva real-backend owners; it contains no
application state. Crossterm-detectable existing raw mode and redirected
input/output are rejected before session mutation. On Unix, Crossterm's raw
mode query tracks its own saved termios state, so unrelated external termios
changes cannot always be detected. Windows raw restoration follows Crossterm's
console-mode semantics rather than preserving every prior custom input flag.
Other libraries and processes must cooperate with terminal ownership.
If the final Drop attempt still fails, the lease deliberately stays held for
the rest of that process: new real backend sessions are refused because the
terminal state is uncertain. Explicit `leave` allows recovery attempts while
the owner and its error details still exist.

Restoration cannot be guaranteed after failed OS output, terminal
disappearance, abort, forced process termination, power loss, or a double
panic. A headless fake verifies attempted operations and resource state, not
the physical state of every terminal emulator.

## Run and verify

```bash
cargo run -p jilva --example backend_demo --locked
cargo run -p jilva --no-default-features --example headless_demo --locked
```

The real demo displays a small diagnostic, reads keys, reacts to resize, and
exits on `q`, Escape, or Ctrl+C. It uses ASCII-escaped input diagnostics so it
does not invent Unicode width rules. The separate headless demo runs in CI.

See [DEVELOPMENT.md](DEVELOPMENT.md) for exact quality gates and restoration
recovery, [PHASE2_TESTING.md](PHASE2_TESTING.md) for failure injection and the
platform matrix, and the [independent audit](reviews/PHASE2_TERMINAL_ENGINE_AUDIT.md)
for actual findings and verification limits.

## Dependency selection

Crossterm `0.29.0` is the only direct external dependency, optional and
confined to `jilva-terminal`. Its MIT license, Rust 1.63 minimum, actual
synchronous APIs, and `events`, `windows`, and `bracketed-paste` feature
definitions were inspected in the resolved crate source and
[upstream API documentation](https://docs.rs/crossterm/0.29.0/crossterm/).
The lockfile records the exact transitive versions used for verification.
No event-stream/async, serialization, clipboard, or Unicode-width dependency
is introduced. Transitive platform support and synchronization remain
Crossterm's implementation responsibility; our workspace forbids unsafe Rust.

## Remaining roadmap boundary

The next existing milestone is **Phase 3 — Rendering Engine**. It must replace
the provisional `Cell.symbol: char` with a grapheme-capable symbol model and
name width/segmentation authorities before implementing buffers and diffs.
That work has not been started here. The required Cargo, three-platform CI,
PTY and independent-audit evidence is recorded in the
[continuation report](reviews/PHASE2_CONTINUATION_REPORT.md). Phase 2 is ready
for maintainer review; PR creation alone was not used as completion evidence.
