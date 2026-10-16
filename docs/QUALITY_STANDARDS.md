# Quality Standards — jilva-rs (Phase 1)

> Status: Phase 1 foundation. Normative for all implementation work on
> `phase1-foundation` and later, until amended by RFC.
> Related: `docs/ARCHITECTURE.md` (§3.9, §11), `docs/API_DESIGN.md`,
> `docs/SECURITY_CONSIDERATIONS.md`, `SECURITY.md`.

## 1. Gates (all must pass; no merge otherwise)

| Gate | Command (representative) | Bar |
|---|---|---|
| Format | `cargo fmt --all -- --check` | Clean. No unformatted code is merged, no exceptions. |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | Zero warnings. Clippy pedantic/nursery lints are advisory unless adopted by RFC; `correctness` and `suspicious` groups must be clean at minimum, and the workspace adopts `-D warnings` so any enabled lint fails the build. |
| Tests | `cargo test --workspace` | All pass. New behaviour ships with tests; bug fixes ship with a regression test. |
| Docs | `cargo doc --workspace --no-deps` (warnings denied) | No rustdoc warnings; every public item has docs (see §4). |
| No-TTy | `cargo test --workspace` on a headless machine / CI without TTY | `jilva-core` tests must pass with no terminal attached (ARCHITECTURE.md §11). |

CI shape itself is a later concern; these gates are what CI must eventually enforce.

## 2. No-unwrap rule

- `unwrap()`, `expect()`, `panic!()`, `unreachable!()`, and `todo!()` are
  **forbidden in non-test code**, except:
  1. `panic!` for a violated internal invariant per ARCHITECTURE.md §3.9
     (unreachable-by-construction states), with a comment stating the invariant;
  2. Code paths covered by `#[cfg(test)]` or `debug_assert!` where the panic
     cannot reach production (tests, examples, benches may use `expect`/`unwrap`
     sparingly and idiomatically).
- Fallible operations return `Result` with the crate error type; conversions go
  through `From` impls inward per §3.9. Recoverable runtime conditions
  (I/O failure, bad terminal state, layout overflow) are never panics.
- Enforcement: code review + `clippy::missing_panics_doc` /
  `clippy::unwrap_used` style restriction lints where the toolchain supports
  them; until lint configuration lands, reviewers reject violations manually.

## 3. Unsafe policy

- **Default: no `unsafe` in Phase 1.** The framework has no performance or FFI
  justification for unsafe code at the foundation stage.
- Any future `unsafe` block requires: (a) a written justification in the PR/RFC
  stating why safe Rust cannot express it, (b) a `// SAFETY:` comment stating
  the upheld invariant, (c) review by a second reviewer, (d) Miri coverage if
  it touches memory semantics. Backend crates inheriting `unsafe` from a
  third-party dependency (e.g. Crossterm internals) do not count against this
  rule but must be noted in the dependency justification.

## 4. Documentation bar

- Every public item (`pub fn/struct/enum/trait`, re-exports in `jilva`) has a
  rustdoc comment: what it does, ownership/error behaviour, and one example for
  user-facing entry points.
- `#![deny(missing_docs)]` on all three Phase 1 crates (`jilva`, `jilva-core`,
  `jilva-terminal`).
- Internal invariants referenced by `panic!` (see §2) and by `// SAFETY:`
  comments are documented at the site, not in a separate wiki.
- Doctests are real tests: they compile and run under `cargo test`.

## 5. MSRV policy

- Until the MSRV RFC lands (ARCHITECTURE.md §1: toolchain stable 1.98.1,
  edition 2024), **stable 1.98.1 is the de-facto floor**: code must build on it,
  must not use nightly features, and must not use features stabilised after
  1.98.1 without an RFC amendment.
- When the MSRV RFC is accepted, this section is amended with the declared
  number and a `rust-version` floor; until then reviewers treat "builds on
  stable 1.98.1, no nightly" as the binding rule.

## 6. Testing expectations

- `jilva-core`: unit tests per module boundary (§3.2–§3.9); renderer tests are
  golden-output (frame in → expected instruction stream out), headless.
- `jilva-terminal`: backend seam tests run against the in-memory test double;
  no test may require a real TTY.
- `tests/`: workspace integration tests import only the `jilva` facade, use
  scripted event sequences and fixed-size virtual screens.
- Coverage: no hard percentage gate in Phase 1; reviewers require tests for
  every new branch of lifecycle, error-taxonomy, and diff behaviour, and note
  untested branches explicitly in the PR.

## 7. Review bar

1. Conformance checklist (ARCHITECTURE.md §11) loops through every PR.
2. Dependency rules (§6): no cycles, core names no backend, facade has no
   logic. Verified by crate dependency inspection.
3. No superiority claims over other frameworks in docs, comments, or messages.
4. Error-taxonomy impact stated for any PR touching rendering, input, or
   lifecycle; platform behaviour stated for any PR touching the backend.
5. Two-reviewer rule for: `unsafe` additions, error-type changes, facade
   surface changes, backend seam changes.
