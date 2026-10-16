# ADR-0001: Rust-first implementation strategy

- Status: Accepted
- Date: 2026-10-16
- Context: Phase 1 foundation, `phase1-foundation` branch.
- Related: `docs/ROADMAP.md` (Stage 3), `docs/ARCHITECTURE.md` §2.

## Context

Jilva is a family ambition: beautiful TUI frameworks for several languages,
starting with Rust (`jilva-rs`), with siblings such as `jilva-go` and `jilva-ts`
envisioned only after the Rust alpha is genuinely usable (ROADMAP Stage 3).
We must decide now what "family" means technically: shared core with language
wrappers, code generation, or independent per-language implementations — because
the answer shapes every boundary we draw in Phase 1.

## Decision

**Rust first, as an independent native implementation.** Each Jilva language
family member will be an independent implementation in its host language,
sharing *principles, API philosophy, theme tokens, and documentation craft* —
not code, not FFI wrappers, not generated bindings.

Concretely for `jilva-rs` Phase 1:

- No abstraction is introduced "for future Go/TS parity" unless Rust itself
  needs it. Boundaries follow Rust idioms (traits, ownership, `Result`).
- The portable artefacts across future siblings are: the design principles
  (ARCHITECTURE.md §2), the semantic theme-token vocabulary (API_DESIGN.md §7),
  the event taxonomy shape, and the docs/RFC culture — all language-neutral
  specifications, none requiring shared code.
- Cross-language work is explicitly gated: sibling implementations start only
  after the Rust alpha proves the design in real apps (ROADMAP Stage 3).

## Consequences

- Positive: the Rust API can be idiomatic without compromise (no lowest-common-
  denominator design); no FFI/toolchain coupling; each future sibling adopts
  only what survived contact with real Rust users.
- Positive: failure is contained — if a future sibling never happens, nothing
  in `jilva-rs` is wasted scaffolding.
- Negative: duplicated design effort per language later; theme-token and
  behaviour drift between siblings is possible and must be managed by
  specification discipline (shared token lists, conformance-style examples)
  rather than by code sharing.
- Negative: contributors hoping for one-codebase-everywhere may be
  disappointed; the README and ROADMAP already set this expectation and must
  keep doing so.

## Alternatives considered

1. **Shared core (Rust) + FFI wrappers for other languages.** Rejected: FFI
   boundaries would distort the Rust API (C-compatible seams, manual memory
   management at edges), complicate distribution (native builds per platform),
   and contradict the non-goal of tying users to machinery they did not ask for.
   Prior art (e.g. Textual remaining Python-native, Bubble Tea Go-native) shows
   TUI frameworks succeed as idiomatic natives, not as bindings.
2. **Code generation / DSL for multi-target output.** Rejected: premature while
   the design itself is unproven; a generator would freeze immature decisions
   and add a whole second product to maintain before the first one works.
3. **Design all languages up front in parallel.** Rejected: multiplies
   uncertainty at the exact moment uncertainty is highest (pre-RFC). The
   ROADMAP staging exists precisely to sequence this risk.
