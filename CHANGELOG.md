# Changelog — jilva-rs

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project is pre-`1.0`: minor versions may break with migration notes,
patch versions never break. Any breaking change while in `0.x` still requires
an RFC amendment (see `docs/ARCHITECTURE.md` §3.1).

## [Unreleased]

### Added

- Phase 2 synchronous terminal engine: optional Crossterm backend, tracked
  session cleanup, typed input/output, capability snapshots, and deterministic
  headless failure injection. The public facade exposes the backend contract.
- Real terminal and headless examples, multi-platform headless CI, terminal
  implementation/testing guides, and independent audit records.
- Maintainer-authorized RFC acceptance revalidation dated 2026-10-17, preserving
  earlier Discussion history and deferred implementation decisions.
- Foundation documentation: system boundaries, crate structure, and dependency
  rules (`docs/ARCHITECTURE.md`); API principles and facade contract
  (`docs/API_DESIGN.md`).
- Seven-phase roadmap from foundation to alpha release (`docs/ROADMAP.md`).
- Contributor setup guide (`docs/GETTING_STARTED.md`).
- Rewritten project README with status, philosophy, contributor commands, repo
  structure, and architecture overview.
- Workspace contribution rules and everyday `cargo` commands (`CONTRIBUTING.md`).
