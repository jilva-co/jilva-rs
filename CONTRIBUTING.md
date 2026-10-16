# Contributing to jilva-rs

Design stage — different rules than a normal repo.

## What helps most right now

1. **RFC reviews** — read [`docs/RFC/`](docs/RFC/), find flaws, cite prior art, propose smaller alternatives.
2. **Pain reports** — what hurt you in Ratatui / Cursive / Bubble Tea / Textual? Concrete stories beat feature votes.
3. **Docs clarity** — if a sentence here confuses you, that is a bug. Open a `docs` issue.

## What is not accepted yet

- `src/` code, new dependencies, or API pull requests without an accepted RFC
  (Phase 1 foundation code exists by design; anything beyond it needs an RFC).
- Large unsolicited PRs — they will be closed with a pointer to Discussions.
- New top-level crates beyond `crates/jilva`, `crates/jilva-core`, and
  `crates/jilva-terminal` (reserved future names are integration points only —
  see `docs/ARCHITECTURE.md` §4.2).

## Process

1. **Discuss first** — [Ideas](https://github.com/orgs/jilva-co/discussions) for anything beyond typos.
2. **RFC for design** — follow [`docs/RFC/README.md`](docs/RFC/README.md).
3. **Small PRs** — one change, Conventional Commits (`docs:`, `chore:`, `feat:`, `fix:`), docs updated.

General org rules: [`jilva-co/.github CONTRIBUTING.md`](https://github.com/jilva-co/.github/blob/main/CONTRIBUTING.md).
Code of Conduct applies everywhere: [`CODE_OF_CONDUCT.md`](https://github.com/jilva-co/.github/blob/main/CODE_OF_CONDUCT.md).

## Workspace commands

Full workflow (toolchain, checks, tests, benches, examples): see
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md). The everyday commands:

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
cargo check --workspace
cargo test --workspace
```

Per-crate while iterating:

```bash
cargo check -p jilva
cargo check -p jilva-core
cargo check -p jilva-terminal
cargo test -p jilva-core
```

Rules that hold from Phase 1 onward (enforced in review; see
`docs/ARCHITECTURE.md` §6–§7):

- `examples/` and `tests/` import only the `jilva` facade — never
  `jilva-core` or `jilva-terminal` directly.
- `jilva-core` must build and pass tests on a machine with no TTY.
- `jilva` (the facade) contains re-exports and docs only — no logic.
- Every new third-party dependency needs a stated reason in its RFC/PR.
- Doc examples use the facade only and must be runnable against the test double.
