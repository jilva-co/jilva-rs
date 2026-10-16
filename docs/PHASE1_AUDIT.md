# Phase 1 Audit — Foundation (final pass)

> Branch: `phase1-foundation`. Scope: whole working tree as inspected on
> 2026-10-16. Method: direct file inspection by the auditor (listed below);
> test/fmt/clippy/doc execution was verified by the orchestrator just before
> this pass (all exit 0) and is **not** re-claimed here — this report covers
> only what the auditor personally inspected.
>
> Normative bars: `docs/ARCHITECTURE.md` (incl. §7 wiring-note amendment),
> `docs/API_DESIGN.md`, `docs/QUALITY_STANDARDS.md`,
> `docs/SECURITY_CONSIDERATIONS.md`, MIT-only constraint.

## 1. Conformance summary (checked, passing)

- **Workspace architecture vs ARCHITECTURE.md §4/§7:** layout matches exactly —
  virtual root manifest, `crates/jilva`, `crates/jilva-core` (7 modules mirroring
  §3 boundaries), `crates/jilva-terminal` (`backend.rs`, `capabilities.rs`,
  `test_double.rs`), root `examples/` + `tests/`. No root `src/`, no extra
  top-level crates, no reserved future crates. The §7 wiring-note amendment is
  implemented exactly as written: `crates/jilva/Cargo.toml` declares
  `[[example]] version → ../../examples/version.rs` and `[[test]] facade_smoke
  → ../../tests/facade_smoke.rs`; contract paths stay at repo root.
- **Dependency selection:** zero external dependencies. `Cargo.lock` contains
  only the 3 workspace packages; no `[dependencies]` beyond workspace paths in
  any of the 4 manifests. Nothing to justify per the §6 minimal-deps rule.
- **Dependency direction (§6):** `jilva-core` imports nothing workspace-external
  (only doc-mentions of `jilva-terminal`); `jilva-terminal` imports nothing from
  `jilva-core` (only doc-mentions); no `cfg(windows)`/`cfg(unix)` anywhere;
  facade contains only re-exports plus a trivial `version()` const accessor
  (accepted as "no logic" — it returns `env!("CARGO_PKG_VERSION")`, no behaviour).
- **Facade-only rule:** `examples/version.rs` and `tests/facade_smoke.rs` import
  `jilva::…` exclusively (grep for `jilva_core`/`jilva_terminal` in both: zero
  hits). Anything they use is reachable through the facade.
- **Error handling / no-unwrap:** zero `unwrap`/`panic!`/`unreachable!`/`todo!`
  in lib code. The only `.expect()` calls are inside `#[cfg(test)]` unit tests
  and the example, each with a written infallibility justification — explicitly
  permitted by `QUALITY_STANDARDS.md` §2.
- **Unsafe:** `#![forbid(unsafe_code)]` in all three crates plus
  `unsafe_code = "forbid"` in workspace lints — stronger than the policy minimum.
- **CI (`.github/workflows/ci.yml`):** `permissions: contents: read` (least
  privilege, no secrets/publishing in Phase 1); check/clippy/test matrix covers
  ubuntu + windows + macos; commands match `docs/DEVELOPMENT.md` 1:1
  (`fmt --check`, `check --all-targets`, `clippy -D warnings`,
  `test --all-targets`, `doc` with `RUSTDOCFLAGS=-D warnings`).
- **Terminal safety:** no executable terminal code exists. Grep over `crates/`
  finds no escape bytes, no raw-mode/alternate-screen syscalls, no terminal
  library — only doc comments *naming* those concepts and the infallible
  in-memory test double. Nothing can leave a real terminal in a broken state.
- **Licensing (MIT-only):** `LICENSE` is the standard MIT text naming "Jilva
  contributors". `license = "MIT"` in workspace + inherited by all 3 crates.
  README badge + §License say MIT. Grep for
  `Apache|LICENSE-APACHE|LICENSE-MIT|dual` finds only ARCHITECTURE.md's "no
  dual-license headers to maintain" (a prohibition, not a remnant). The part-1
  README dual-license conflict is **resolved**.
- **Cross-platform:** `.editorconfig` enforces LF; CI tests Windows; DEVELOPMENT
  documents PowerShell equivalents and long-path guidance; no filesystem,
  path-manipulation, or OS-gated code exists to be Windows-hostile.

## 2. Findings

### MEDIUM-1 — CI actions use mutable tags, not SHA pins
- **File:** `.github/workflows/ci.yml`
- **Problem:** `actions/checkout@v5`, `dtolnay/rust-toolchain@stable`,
  `Swatinem/rust-cache@v2` float on moving tags; a compromised tag update flows
  silently into CI.
- **Impact:** Supply-chain: CI could execute untrusted code with (read-only, but
  still trusted-context) permissions.
- **Recommended solution:** pin each action to a full commit SHA with a
  trailing version comment (e.g. `# v5.0.0`) and enable Dependabot for automated
  bump PRs. Non-blocking for Phase 1 (no secrets, no releases), but do it before
  any publishing job exists.
- **Verification:** `grep -E 'uses:.*@[0-9a-f]{40}' .github/workflows/ci.yml`
  returns all three actions; Dependabot config present.

### MEDIUM-2 — Toolchain floats past the declared 1.98.1 floor
- **File:** `rust-toolchain.toml`
- **Problem:** `channel = "stable"` tracks whatever stable is newest, while
  ARCHITECTURE.md / README / GETTING_STARTED declare stable **1.98.1** as the
  de-facto MSRV floor ("must build on 1.98.1, no nightly"). CI therefore cannot
  catch use of post-1.98.1 features, and local builds drift from the documented
  floor.
- **Impact:** Silent MSRV violation: code may rely on newer std/cargo features
  while docs promise a 1.98.1 floor.
- **Recommended solution:** pin `channel = "1.98.1"` (keep rustfmt/clippy
  components) until the MSRV RFC lands; optionally add a second CI job on
  latest stable as an early-warning, not the gate.
- **Verification:** `rustc --version` reports 1.98.1 under a clean
  `rustup`-managed checkout; CI log shows the pinned toolchain.

### LOW-1 — `missing_docs` is `warn`, policy says `deny`
- **Files:** `Cargo.toml` (`[workspace.lints.rust] missing_docs = "warn"`),
  all three `src/lib.rs` (`#![warn(missing_docs)]`); cf. `QUALITY_STANDARDS.md` §4
- **Problem:** policy requires `#![deny(missing_docs)]`; implementation warns
  locally and only denies under CI's `RUSTDOCFLAGS=-D warnings`.
- **Impact:** trivial — a contributor running plain `cargo doc` locally gets
  warnings instead of errors; CI still gates. Docs are in fact complete today.
- **Recommended solution:** either bump workspace lint + crate attributes to
  `deny`, or amend the policy to "warn locally, deny in CI". Pick one; the two
  must agree.
- **Verification:** `cargo doc --workspace --no-deps` (no flags) fails on any
  undocumented public item after the bump.

### LOW-2 — README claims a "default backend" that does not exist
- **File:** `README.md:134` (repository-structure diagram)
- **Problem:** `jilva-terminal/` annotated "backend seam + default backend +
  test double"; only the seam + test double exist (ADR-0003: Crossterm default
  is *planned*, implementation is RFC detail).
- **Impact:** docs-only; a reader may look for backend code that is not there.
- **Recommended solution:** change to "backend seam + test double (default
  backend is RFC detail, see ADR-0003)".
- **Verification:** re-read of the section; `ls crates/jilva-terminal/src`
  shows no backend implementation.

### LOW-3 — Stale "scaffolding has not merged yet" notes
- **Files:** `README.md:108-111`, `docs/GETTING_STARTED.md:41-43`
- **Problem:** both carry a note for the pre-scaffolding state ("if `cargo`
  reports missing members, the workspace scaffolding has not merged yet").
  The scaffolding is now in the tree, so the note misdirects.
- **Impact:** minor contributor confusion.
- **Recommended solution:** delete or rephrase to past tense ("landed on this
  branch via …").
- **Verification:** re-read; `cargo check --workspace` succeeds on a fresh
  checkout of the branch.

### LOW-4 — CONTRIBUTING design-stage gate is now partially stale
- **File:** `CONTRIBUTING.md:11-13` ("What is not accepted yet: `src/` code…")
- **Problem:** written when no code existed; Phase 1 lands `src/` code by
  design, so a flat "`src/` code … not accepted" overstates the rule.
- **Impact:** could deter legitimate RFC-backed implementation PRs.
- **Recommended solution:** qualify: "`src/` code … without an accepted RFC"
  (already implied by the trailing clause — make it explicit).
- **Verification:** re-read after edit.

### LOW-5 — `benches/` in layout contracts has no directory
- **Files:** `README.md:138`, `docs/ARCHITECTURE.md:305-306`
- **Problem:** both list `benches/`; the directory does not exist (no
  benchmarks in Phase 1, correctly — nothing to measure yet).
- **Impact:** none functional; layout-vs-tree mismatch a reviewer could flag.
- **Recommended solution:** annotate as "(from Phase 3/perf work; not yet
  present)" or leave — acceptable either way once consciously decided.
- **Verification:** `ls benches` fails; docs say future explicitly.

### LOW-6 — `Cargo.lock` untracked by accident rather than by rule
- **File:** `.gitignore` (comment-only intent, no pattern)
- **Problem:** the comment states "Cargo.lock is NOT committed", but there is
  no `Cargo.lock` ignore pattern — the file exists locally and shows as
  untracked (`??`), so nothing stops an accidental `git add -A` from committing
  it. (Confirmed: `git check-ignore Cargo.lock` returns non-zero.)
- **Impact:** low — a committed lockfile in a lib-only workspace is merge noise,
  not breakage.
- **Recommended solution:** add an explicit `Cargo.lock` line to `.gitignore`
  (or delete the comment if the team prefers committing it — decide once).
- **Verification:** after the change, `git check-ignore -v Cargo.lock` prints
  the `.gitignore` rule and `git status --short` no longer lists it.

## 3. Files inspected (36)

Root manifests/config (7): `Cargo.toml`, `Cargo.lock` (package list only),
`rust-toolchain.toml`, `.github/workflows/ci.yml`, `.editorconfig`,
`.gitignore`, `LICENSE`. Source (13): `crates/jilva/src/lib.rs`,
`crates/jilva-core/src/{lib,component,error,events,layout,lifecycle,renderer,style}.rs`,
`crates/jilva-terminal/src/{lib,backend,capabilities,test_double}.rs`.
Crate manifests (3). Example + test (2): `examples/version.rs`,
`tests/facade_smoke.rs`. Docs (11): `README.md`, `SECURITY.md`,
`CONTRIBUTING.md`, `CHANGELOG.md`, `docs/{ARCHITECTURE,API_DESIGN,
DEVELOPMENT,GETTING_STARTED,QUALITY_STANDARDS,SECURITY_CONSIDERATIONS}.md`,
`docs/adr/0003-terminal-backend-strategy.md` (+ `ls` of `docs/adr`,
`docs/RFC`). Plus `git status`/`ls-files`/`diff --stat` for tree state and
`grep` sweeps (licensing, unwrap/expect/panic, unsafe/escape/backend refs,
facade-boundary imports, `cfg(windows)`).

Not re-executed here (orchestrator-verified, exit 0): `cargo check/test`,
`cargo fmt --check`, `cargo clippy -D warnings`, `cargo doc` with
`RUSTDOCFLAGS=-D warnings`.

## 4. Verdict

**READY FOR MERGE.** Zero BLOCKERs, zero HIGHs, 2 MEDIUMs, 6 LOWs — all
non-blocking for a Phase-1 foundation (unreleased `0.0.0`, no secrets, no
publishing, no executable terminal code). Recommended sequencing: fix
MEDIUM-2 (toolchain pin) and LOW-1 (missing_docs alignment) before or with the
merge since both are one-line changes that protect the guarantees Phase 2 will
rely on; schedule MEDIUM-1 (SHA-pinned actions) before any release/publish job
exists; fold LOW-2–LOW-5 into a single docs-touch-up PR; decide LOW-6
(lockfile rule) explicitly either way.
