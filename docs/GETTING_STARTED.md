# Getting Started — jilva-rs

> **Status:** Phase 2 — Terminal Engine. The real terminal and headless
> demonstrations are runnable. The renderer, application runtime, layout
> engine, and widgets remain future milestones; nothing is released yet.

## 1. Clone the repo

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
```

## 2. Install the toolchain

- Rust stable **1.98.1**, edition 2024 (see `docs/ARCHITECTURE.md`).
- The MSRV number itself is RFC-track; until that RFC lands, build on stable
  1.98.1 and do not rely on nightly features.
- If you use `rustup`:

```bash
rustup toolchain install 1.98.1 --component rustfmt --component clippy
rustc --version   # expect 1.98.1
```

## 3. Verify the workspace

```bash
cargo check --workspace
cargo test --workspace
```

In an interactive terminal, run `cargo run -p jilva --example backend_demo`
and exit with `q`, Escape, or Ctrl+C. For CI and pipes, run
`cargo run -p jilva --no-default-features --example headless_demo`.

Per-crate while iterating:

```bash
cargo check -p jilva
cargo check -p jilva-core
cargo check -p jilva-terminal
```

## 4. Where to read next

Read in this order:

1. [`README.md`](../README.md) — vision, scope, non-goals.
2. [`ARCHITECTURE.md`](ARCHITECTURE.md) — system boundaries (§3), crate
   structure (§4), dependency rules (§6), file layout (§7). Normative for
   contributors.
3. [`API_DESIGN.md`](API_DESIGN.md) — API principles and the facade
   contract. Sketches are illustrative, not final.
4. [`ROADMAP.md`](ROADMAP.md) — the 7 phases; Phase 2 is current.
5. [`RFC/README.md`](RFC/README.md) — how design decisions get made.
6. [`CONTRIBUTING.md`](../CONTRIBUTING.md) — design-stage rules and review
   conventions.
7. [`DEVELOPMENT.md`](DEVELOPMENT.md) — day-to-day workspace workflow
   and terminal-restoration troubleshooting.

Key rules to internalise early: `examples/` and `tests/` import only the
`jilva` facade; `jilva-core` never names a backend and stays TTY-independent;
the facade contains no logic.

## 5. How to join the RFC discussion

1. Lurk the [Ideas discussions](https://github.com/orgs/jilva-co/discussions)
   to see what is open.
2. Read [`RFC/README.md`](RFC/README.md): one proposal per RFC, prior art
   required (what do Ratatui / Cursive / Bubble Tea / Textual do here?),
   non-goals and open questions explicit.
3. Open a Discussion before a PR for anything beyond typos.
4. Review open RFCs: find flaws, cite prior art, propose smaller alternatives.
   Concrete pain reports from other frameworks beat feature votes.

Looking for the fastest useful contribution? Review an open RFC, or report a
concrete pain story from a framework you have used.
