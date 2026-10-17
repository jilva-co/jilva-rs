# Phase-2 Backend Contract — Terminal Engine (normative)

- Status: **normative for Phase-2 implementation work**. Accepted RFC-1/2/3
  are the design authority; this document is their executable subset for the
  backend track. Where this contract and an accepted RFC disagree, the RFC
  wins and this contract must be amended.
- Author: Subagent 1 — Architecture & RFC Governance Engineer (Jilva Phase 2).
- Date: 2026-10-16; revalidated and clarified 2026-10-17 (see decision tracker).
- Hard constraints: workspace MIT license, edition 2024, pinned toolchain
  1.98.1, no unsafe workspace code, and no unrelated dependencies. Crossterm
  is the approved terminal library; version/features must be recorded with an
  ARCHITECTURE.md §6 rule 6 justification. This contract authorizes Phase-2
  production implementation; renderer/widgets/runtime remain later work.
- Conventions: `MUST`/`MUST NOT` are contract requirements verified by
  inspection or test. `PROPOSED` marks exact identifier spellings that are
  normative for Phase 2 (implementers use these names so later phases have a
  stable target); deeper semantics (diff algorithms, palettes, ladder depth)
  stay with their owning RFCs/phases.

## 0. What Phase 2 owns (and what it does not)

Phase 2 builds the backend seam only (ARCHITECTURE.md §3.5 + §3.10,
RFC-1 Q14 J-list "backend seam interface", API_DESIGN.md §9):

- Backend trait with the full method set (§1).
- Capability domain types + detection plumbing (§4).
- Typed input events crossing the seam (§2) and typed output commands (§3).
- Error-boundary mapping (§5), session/reentrancy rules (§6), test-double
  substitution rules (§7).

Phase 2 does NOT own: the run loop (Phase 5), buffer/diff/damage (Phase 3),
layout solving (Phase 4), theme schema/palettes (Phase 6), the keyboard
ladder depth decision (lifecycle RFC, RFC-1 Q13 still-open), symbol-type /
width authorities (renderer RFC ≤ Phase 3, tracker D5). The
`char`-non-entrenchment rule is in force throughout: no new API may name
`char`-indexed cells beyond what Phase-1 code already contains, and no
golden test may entrench scalar-cell indexing (RFC-1 OQ12, ROADMAP.md
Phase-3 exit obligation).

## 1. Backend trait — responsibilities and method semantics

Location: `crates/jilva-terminal/src/backend.rs`. Trait name: `Backend`
(kept — existing public item, facade re-exports it).

Responsibility: the ONLY path by which framework bytes reach or leave a real
terminal. The framework drives; the backend obeys. The backend MUST NOT
consume components, app state, style tokens, or layout intent (ADR-0003
decision 2); it receives fully-resolved draw instructions and emits
decoded input events. Plain cell geometry such as `Rect` is permitted.

Method set (all required; existing `name`/`enter`/`leave` signatures are
preserved byte-for-byte to avoid churning tested code):

```rust
// PROPOSED — normative identifier set for Phase 2 (implementer writes this).
pub trait Backend {
    // — identity (existing, unchanged) —
    fn name(&self) -> &str;

    // — session (existing signatures, sharpened semantics in §6) —
    // enter  ≡ init:    enter terminal mode + run detection + cache profile.
    // leave  ≡ teardown: restore everything enter did, on every exit path.
    fn enter(&mut self) -> std::io::Result<()>;
    fn leave(&mut self) -> std::io::Result<()>;

    // — observation —
    // Cached terminal size in cells, refreshed by consumed resize events.
    // MUST return a non-degenerate size:
    // a zero width or height is reported as an error, never as (0, 0).
    fn size(&self) -> std::io::Result<(u16, u16)>;

    // Source readiness, not a promise of a supported InputEvent: read may
    // filter unsupported source events and return None. Duration::ZERO
    // requests a non-blocking readiness check. Requested waiting is bounded
    // by timeout; scheduler/syscall overhead cannot be bounded here.
    fn poll(&mut self, timeout: std::time::Duration) -> std::io::Result<bool>;

    // Return one supported decoded input event, or Ok(None) for an empty
    // queue or bounded filtering of unsupported events. Raw decode failures
    // are not exposed by Crossterm as a recoverable raw
    // sequence: propagate decoder I/O errors. Unsupported decoded events may
    // be skipped and counted without closing the session. A read call must
    // perform bounded filtering work so unsupported traffic cannot starve it.
    fn read(&mut self) -> std::io::Result<Option<InputEvent>>;

    // — emission —
    // Buffer output commands in order. Buffering MUST be FIFO and MUST NOT
    // write to the terminal yet. Validate the whole batch before appending:
    // an invalid batch leaves pending commands unchanged.
    fn submit(&mut self, commands: &[OutputCommand]) -> std::io::Result<()>;

    // Commit everything buffered by `submit` since the last `flush` to the
    // terminal, in order. After `Ok(())` the buffer is empty. A failed
    // `flush` MUST report the error and MUST retain or discard the buffer
    // deterministically. Policy: discard after each attempted flush, avoiding
    // replay of an already-written prefix. Partial terminal writes cannot be
    // rolled back; report the error and require explicit caller recovery.
    fn flush(&mut self) -> std::io::Result<()>;

    // The cached capability snapshot taken at `enter` (see §4). Takes
    // `&self`, returns by value (`Copy`). MUST NOT re-probe the terminal:
    // detection runs at `enter` only.
    fn capabilities(&self) -> TerminalCapabilities;
}
```

Referenced types (all core-owned, §4): `InputEvent`
(`jilva-core/src/events.rs`), `OutputCommand`
(`jilva-core/src/output.rs`, new module), `TerminalCapabilities`
(`jilva-core/src/capabilities.rs`, new module).

File placement for the real backend:
`crates/jilva-terminal/src/crossterm_backend.rs` holding the
crossterm-based `Backend` implementor (ADR-0003 planned default; library
choice + version/features recorded under ARCHITECTURE.md §6 rule 6). Tests,
example and implementation notes may be added within the existing workspace
layout without introducing extra crates.

## 2. Typed input events

Location: `jilva-core/src/events.rs` (extends the existing `Event` —
core ownership of the taxonomy is existing fact, kept).

Contract:

- `InputEvent` (`pub enum`, `#[non_exhaustive]`,
  `Clone + Debug + PartialEq + Eq`): the backend-decoded subset of the
  RFC-2 §4 taxonomy — `Key(KeyEvent)`, `Mouse(MouseEvent)`, `Resize{cols,
  rows}`, plus optional terminal-origin `Focus(bool)` and `Paste(String)`.
  It carries NO `Message`, NO `Tick`, NO component-focus synthesis: those
  are loop-synthesised. Terminal focus is a notification from the terminal,
  distinct from a focused widget/path. Paste payloads remain untrusted and
  must pass ordinary output validation before display. (The existing
  `Event::{Tick, Resized, Custom}` grows into the RFC-2 taxonomy —
  `#[non_exhaustive]` + `Message(M)` wrapper, tick-as-injected-value —
  under the lifecycle/component track; the backend track MUST NOT pre-empt
  that shape beyond producing the terminal-origin variants above.)
- `KeyEvent` / `MouseEvent` (PROPOSED names): plain-data structs/enums in
  the same module. `KeyEvent` MUST record the encoding source
  (legacy vs enhanced, per `InputProfile`) so the loop can apply the
  documented degradation ladder. Preserve press/repeat/release and source
  modifiers when provided; event kind alone is not enhanced-encoding evidence.
  `MouseEvent` MUST record modifiers,
  click/drag/scroll + position in cells, gated by capability (absence of
  mouse support MUST yield no mouse events, never synthetic ones). Focus and
  paste likewise require explicitly enabled corresponding terminal modes;
  disabled modes never appear as supported in the cached profile.
- `Backend::read` returns `Option<InputEvent>`; the loop promotes
  `InputEvent` into the framework `Event` (wrapping, resize-broadcast, and
  focus routing are loop concerns — RFC-2 §4 — not backend concerns).
- Existing `Event::affects_layout` (resize ⇒ true) is the Phase-2
  invalidation floor: `Resize` MUST invalidate layout; key/mouse MUST NOT
  by backend assertion alone (widgets report their own change class —
  RFC-2 §5).

## 3. Typed output commands

Location: `jilva-core/src/output.rs` (new module; plain data, zero deps).

Contract — `OutputCommand` (PROPOSED name, `pub enum`,
`#[non_exhaustive]`, `Clone + Debug + PartialEq + Eq`): the instruction
vocabulary the backend executes. Minimum Phase-2 set (renderer RFC
extends it in Phase 3; additive variants only):

- `MoveTo { x: u16, y: u16 }` — place the cursor (0-based cell coords).
- `PrintSegment { text: String }` — write a Unicode string at the cursor.
  Segmentation/width remain renderer responsibilities: Phase 2 does not assert
  that the payload is exactly one grapheme or one cell. Control characters
  C0, DEL and C1 are rejected (including ESC, newline and tab); ordinary text
  cannot inject terminal sequences. Validate every command in the batch
  before appending any; the direct enum constructor does not bypass validation.
- `SetAttributes(ResolvedAttributes)` — apply fully-resolved visual
  attributes from this point on. `ResolvedAttributes` (PROPOSED name, same
  module): plain-data `{ fg, bg, bold, dim, italic, underline, reverse,
  strikethrough }` over core-owned colour/attr types — NEVER tokens, NEVER
  ANSI bytes (RFC-3 R1/R4: resolution is a pure core function; the backend
  receives resolved attributes only).
- `ResetAttributes` — return to terminal default.
- `HideCursor` / `ShowCursor` — cursor visibility (single-owner rule rides
  with the lifecycle RFC; Phase 2 provides the verbs, not the policy).
- `ClearRegion { rect: Rect }` — reset a rectangle to blank (uses the
  existing core `Rect`; blank = space, per `Cell::blank` ground state).

Rules: commands are data — the backend executes, never interprets style.
Ordering is significant (FIFO from `submit` to `flush`). Cursor
minimisation and diffing are Phase-3 renderer concerns; Phase-2 backends
execute the stream literally and correctly.

## 4. Capability domain types + ownership (core-vs-terminal resolved)

### 4.1 Decision: core owns the types, terminal populates them

Shared plain-data types MUST live in exactly one crate: **`jilva-core`**.
`jilva-terminal` depends on `jilva-core` for those definitions and pure
validation/profile value helpers;
`jilva-core` NEVER depends on `jilva-terminal`, names no terminal library,
touches no TTY, and keeps no `cfg(windows)` branching. The facade
re-exports the public surface; examples/tests import the facade only
(unchanged rule).

Why this direction (and not the reverse, and not duplication):

1. ADR-0003 decision 2 requires the seam to be decoupled: if the backend
   crate owned the profile types, every detection-side change would ripple
   into core resolution signatures and user-visible capability queries —
   coupling unrelated rates of change. Core-owned data + terminal-owned
   detection keeps each side's evolution local.
2. Single definition prevents detector/consumer drift: two parallel
   profile structs (one per crate) would need perpetual mirroring tests;
   one canonical definition makes drift a compile error, not a test gap.
3. Core stays hermetic and TTY-free testable: the types are data + pure
   functions (resolution, quantization tables), so core tests run anywhere
   with zero terminal machinery — the ARCHITECTURE.md §3.10 rule survives
   contact with real types.
4. Acyclicity is preserved: the edge is `jilva-terminal → jilva-core`
   (shared data/pure helpers), same direction as the existing `jilva → jilva-core` /
   `jilva → jilva-terminal` edges. No cycle is introduced; core still
   depends on nothing in-workspace.
5. Test-double fidelity is structural: the double and the real backend
   construct the SAME core types, so substitution stays a
   construction-edge choice (§7), never a translation layer.

ARCHITECTURE.md §6 rule 1 permits this terminal→core edge; core has no terminal
dependency. Pure interpretation of injected env strings is permitted in core;
reading process env and TTY/probe integration remain terminal-owned.

### 4.2 The domain model (normative shapes, core-owned)

Location: `jilva-core/src/capabilities.rs` (new module). All items
`Clone + Copy + Debug + PartialEq + Eq + Hash` value types,
`#[non_exhaustive]` on enums/structs that detection may extend
(additive fields with documented defaults only — profile shape is on the
RFC-1 Q14 measure-twice list; removing/retyping a field needs an
amendment RFC).

```rust
// PROPOSED — normative names for Phase 2.
pub enum ColorTier { Truecolor, Palette256, Ansi16, Mono }
pub struct AttrSupport { /* per-attribute bool flags:
    bold, dim, italic, underline, reverse, strikethrough */ }
pub enum BackgroundHint { Dark, Light, Unknown }
pub struct ColorProfile  { pub color: ColorTier, pub attrs: AttrSupport,
    pub background: BackgroundHint }
pub struct InputProfile  { /* key encoding offered, mouse mode offered —
    exact fields owned by the lifecycle/event RFC; Phase 2 provides the
    struct + conservative defaults (legacy/off) */ }
pub struct ScreenProfile { /* size, resize semantics, alternate-screen —
    exact fields owned by the lifecycle/renderer RFCs; Phase 2 provides
    the struct + conservative defaults */ }
pub struct TerminalCapabilities {
    pub color: ColorProfile, pub input: InputProfile,
    pub screen: ScreenProfile, pub reduced_motion: bool,
    pub snapshot_id: u64,           // identity: which detection this is
    pub color_version: u64,         // per-domain causality counters (§4.3)
    pub input_version: u64,
    pub screen_version: u64,
}
```

Consumption rule (normative, from RFC-3 P1): every consumer takes its own
domain struct (`ColorProfile` for style, `InputProfile` for input,
`ScreenProfile` for renderer/lifecycle), NEVER the aggregate — except at
the two seams where the aggregate travels: terminal→loop and loop→frame.

Migration notes (implementer executes, contract authorises): the existing
`jilva-terminal::CapabilityProfile { truecolor, mouse, alternate_screen }`
placeholder is SUPERSEDED by this model (implementer migrates the struct,
its `basic()`/`full()` constructors, the test double, and the facade
re-export — conservative default becomes Ansi16-not-Mono per RFC-3 §5);
the existing `jilva-core::ColorDepth` (Truecolor/Ansi256/Ansi16, no Mono)
is SUPERSEDED by `ColorTier` (Mono added, `NO_COLOR`→Mono mapping).
Supersession MUST be total within Phase 2: no two canonical profile types
may coexist past the Phase-2 exit.

### 4.3 Detection, caching, refresh

- Detection lives in `jilva-terminal` (function `detect()` — PROPOSED name,
  free function or associated constructor, implementer places it; MUST be
  callable headless for tests with injected env/probe stubs, never
  hard-wired to the process environment in a way tests cannot override).
- `Backend::enter` runs detection ONCE, caches the snapshot; `capabilities()`
  returns the cached value. No ad-hoc mid-frame sniffing (RFC-1 Q12b,
  ARCHITECTURE.md §8.14).
- Conservative defaults on uncertainty: Ansi16 (not Mono) unless positive
  evidence; `NO_COLOR` (present, any value incl. empty) ⇒ Mono with
  decorative attributes stripped, structural reverse-video retained
  (RFC-3 §5 Jilva policy); `BackgroundHint::Unknown` ⇒ NO auto-switch,
  keep app theme (rejected alternative recorded in RFC-3 Alt-4).
- Refresh cadence for the future runtime: snapshot-at-start (RFC-3 Q7
  resolved). Phase 2 caches one detection per entered session; explicit
  detection helpers may build values headlessly, but live mode reconfiguration
  and a refresh queue are deferred to the lifecycle RFC (tracker D9).
  Resize events may update screen geometry/screen_version only. The future
  loop applies queued changes at frame boundaries; a committed frame resolves against exactly one start
  snapshot AND reaches paint only through error-free describe/layout,
  otherwise the previous buffer is retained and nothing is emitted
  (combined frame-commit invariant, owned by the lifecycle RFC — Phase 2
  does not implement that queue or loop policy).
- Unknown/degraded values are explicit everywhere; guessing is forbidden
  (typed-domain hybrid, RFC-3 §5 note). Resolution is total over all
  domain values: no `Result`, no panic on unknown input.

## 5. Error boundaries

The backend deliberately exposes `std::io::Error` through `io::Result`.
The facade also re-exports `jilva_core::error::Error` (existing,
`#[non_exhaustive]`: `Io / Backend / Lifecycle / Layout / Render /
Callback`) as the future runtime aggregate. It is not the only public error
type at every seam. Phase 2 introduces no backend-library-specific error type;
the runtime may convert `io::Error` via the existing `From` implementation.

Mapping at the seam (normative):

| Seam outcome | Facade surface | Carried context |
|---|---|---|
| OS/terminal I/O failure (`io::Error` from syscalls) | `Error::Io` (via existing `From<io::Error>`) | source preserved |
| Backend validation/session failure (double-`enter`, degenerate size, invalid text) | `io::Error` at the seam; `Error::Io` if converted above it | operation + useful reason; no automatic `Error::Backend` mapping |
| Skipped unsupported decoded input event (§1 `read`) | NOT an error — counted, stream continues | counter exposed on the concrete backend for tests |
| User-callback failure above the seam | `Error::Callback` (loop concern, not backend) | component-path context (RFC-2 §6) |

Rules: backend-specific types MUST NEVER appear in `Error` payloads or in
facade-visible messages (existing `Error` docs already promise this —
kept). `panic!` is reserved for violated internal invariants with
developer-aimed messages; lifecycle teardown still attempts restoration
(API_DESIGN.md §6). `Drop` impls MUST NEVER panic (a failing best-effort
`leave` in `Drop` is swallowed and debug-logged, never unwound).

## 6. Session ownership + reentrancy rules

- One live session per backend instance. `enter` on an entered backend ⇒
  `ErrorKind::AlreadyExists`; entry with dirty cleanup state ⇒
  `ErrorKind::WouldBlock`. Inactive input/output/size operations ⇒
  `ErrorKind::NotConnected`. `leave` on a
  fully inactive backend ⇒ `Ok(())` (idempotent, so `Drop` guards and
  double-teardown paths are safe). `size` / `poll` / `read` / `submit` /
  `flush` outside a live session ⇒ error (fail-loud, never operate on a
  dead terminal). A failed setup/leave may retain dirty resource flags while
  `is_entered` is false; `leave` retries those flags and may still fail.
- `enter` is a tracked setup transaction: detect + acquire raw mode and the
  configured supported modes (alternate screen, mouse, optional input modes).
  Each successful step and potentially partial terminal command must be
  tracked for rollback. `leave` attempts inverse operations in reverse order,
  restoring every flag `enter` pushed
  (teardown pairing — the lifecycle RFC owns Kitty-flag specifics; Phase 2
  owns the pairing discipline: every push has a recorded pop). Existing raw
  mode must not be silently acquired as if owned; concurrent terminal owners
  must be rejected where detectable.
- No reentrancy by construction: all mutating methods take `&mut self`
  (the borrow checker is the enforcement); the backend holds NO callbacks
  into the loop, invokes NO user code, and spawns NO threads. Anything that
  would recurse (e.g. a resize observed mid-`flush`) is queued as data and
  surfaced at the next `poll`/`read` boundary, never handled inline.
- Teardown guarantee: restoration MUST be attempted on recoverable exit paths (normal,
  `?`-error, panic-unwind where feasible — ARCHITECTURE.md §8.7). Phase 2
  provides a best-effort `Drop` guard attempting `leave`; the panic-guard
  mechanism itself is lifecycle-RFC detail (tracker D-queue). Phase 2 tests
  setup rollback, I/O failure, explicit leave, Drop and panic-unwind behavior
  headlessly; these verify the backend mechanism, not the future app loop.
  Cleanup errors retain failed-resource state for a later explicit retry and
  preserve the original failure context. Drop must not panic. OS I/O failure,
  abort, process kill or terminal disappearance can prevent successful
  restoration; an attempt is not proof that the terminal was restored.

## 7. Test-double substitution rules

Location: `crates/jilva-terminal/src/test_double.rs` — `TestDoubleBackend`
(existing; implementer extends to the full §1 trait — the current
enter/leave/log core is kept and its existing test keeps passing).

- Full-fidelity double: implements EVERY `Backend` method. Scripted
  `Vec<InputEvent>` in (FIFO; `poll`/`read` drain it), recorded
  `Vec<OutputCommand>` out (every `submit` appended verbatim;
  `flush` boundaries recorded as markers). Configurable size
  (constructor parameter, default 80×24) and configurable
  `TerminalCapabilities` snapshot at construction (per-tier determinism).
- Determinism rule: same script + same profile + same size ⇒
  bit-identical recorded command stream. Input injection while entered models
  newly arriving terminal input at the next poll/read observation; Phase 2
  has no frames, so it must not claim to enforce frame snapshot policy.
- Substitution is construction-edge only: backend choice happens at
  explicit backend construction (a future `App::builder().backend(...)` is
  still a proposal); NO production code branches on backend
  type, NO `cfg(test)` backend selection, NO global backend switch
  (ARCHITECTURE.md principle 3 — no ambient state).
- Facade honesty: tests and examples import the double SOLELY through the
  facade's test surface (`jilva::…` test path — implementer wires it;
  `examples/` and `tests/` MUST NOT import `jilva-terminal` directly,
  per ARCHITECTURE.md §7). Golden-output tests (typed command stream in →
  expected recording out) run with no TTY — this is the Phase-2 exit
  criterion (ROADMAP.md).
- Failure injection (for restoration/error-path tests): the double
  supports scripted `io::Error` at chosen `enter`/`flush`/`read` call
  indices (PROPOSED: `fail_at: Vec<(Op, io::ErrorKind)>` or equivalent —
  exact shape implementer-chosen, capability required). A failing double
  MUST still honour the §6 leave-idempotency rule.

## 8. Phase-2 exit checklist (implementer proves each)

- [x] `Backend` trait carries the full §1 set; both backends (real +
  double) implement it; `forbid(unsafe_code)` holds workspace-wide.
- [x] Core owns `capabilities.rs` / `events.rs` (`InputEvent`) /
  `output.rs`; terminal depends on core for types only
  (`cargo tree` shows no `jilva-core → jilva-terminal` edge, no cycle);
  old placeholder types fully superseded (§4.2 migration notes done).
- [x] `Terminal → loop` seam carries only `InputEvent` /
  `TerminalCapabilities` data; `loop → terminal` carries only
  `&[OutputCommand]`; backend sees no tokens, no components, no state.
- [x] Golden-output + scripted-event tests green with no TTY; determinism
  rule (§7) asserted by at least one replay-twice test.
- [x] Backend error paths (§5) covered, including unsupported-event filtering,
  decoder I/O propagation, validation, flush discard and leave-idempotency.
  Runtime callback wrapping remains Phase 5, not a Phase-2 test claim.
- [x] New dependencies justified under ARCHITECTURE.md §6 rule 6, compatible
  licensing reviewed; workspace source MIT; edition 2024; stable 1.98.1,
  no nightly. Lockfile records the selected dependency graph.
- [x] Facade-only imports in `examples/`/`tests/`; facade re-exports the
  new public types under flat paths (API_DESIGN.md §2).

## 9. Traceability (contract → authority)

| Contract section | Authority |
|---|---|
| §1 seam shape (setup/teardown/read/write/size/capabilities) | RFC-1 Q14 J-list; API_DESIGN.md §9; ARCHITECTURE.md §3.5 |
| §2 event subset + promotion rule | RFC-2 §4 (taxonomy, dispatch, extensibility); RFC-1 OQ4 |
| §3 command vocabulary + resolved-attrs-only | RFC-3 R1/R4/R5; ADR-0003 decision 2 |
| §4 ownership model + domain split + snapshot-at-start | RFC-3 §5 + P1/P2/P3; `CAPABILITY_MODEL_RESOLUTION.md` §§3–7; ADR-0003; RFC-1 Q12b/Q14 |
| §5 error taxonomy + mapping | ARCHITECTURE.md §3.9; API_DESIGN.md §6; RFC-2 §6 |
| §6 session + teardown + no-reentrancy | ARCHITECTURE.md §§3.2/8.7; RFC-2 §1/§4 (no reentrancy, FIFO) |
| §7 double fidelity + facade honesty | ARCHITECTURE.md §§3.5/7/11; RFC-1 Q3/Q10–Q11; feasibility §6-Q5 (scripted sequences suffice) |
| Standing prohibitions (§0) | RFC-1 Q9 permanent refusals; cross-RFC review §D |
