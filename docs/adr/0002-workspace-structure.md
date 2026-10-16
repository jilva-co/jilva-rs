# ADR-0002: Cargo workspace structure and dependency direction

- Status: Accepted
- Date: 2026-10-16
- Context: Phase 1 foundation, `phase1-foundation` branch.
- Related: `docs/ARCHITECTURE.md` (§4–§7), `docs/API_DESIGN.md` (§2).

## Context

Phase 1 must fix the crate topology before any code exists, because crate
boundaries are the hardest thing to change later (public paths, semver units,
dependency edges). The choice is between a single crate with modules, a small
multi-crate workspace now, and a fully split workspace up front (render,
layout, style, widgets as crates from day one). The decision must also fix the
dependency direction rules and the exact file layout implementers follow.

## Decision

**A three-crate Cargo workspace: `jilva` (facade), `jilva-core` (foundations),
`jilva-terminal` (backend abstraction)** — with all other conceivable crates
reserved as integration points only (ARCHITECTURE.md §4.2), never created in
Phase 1.

Dependency direction (strict, acyclic):

```text
user app → jilva → jilva-core
                 ↘ jilva-terminal (names the terminal library; core never does)
```

Rules: no workspace cycles ever; `jilva-core` is dependency-free within the
workspace and platform-agnostic (no TTY, no `cfg(windows)` branching);
`jilva-terminal` never depends on workspace code and never sees app state or
components; `jilva` contains no logic, only re-exports; `examples/` and `tests/`
import the facade only. Full rules and the exact file layout contract live in
ARCHITECTURE.md §6–§7 and are normative.

## Consequences

- Positive: users depend on one crate (`jilva`) with one version while
  internals stay refactorable; the platform/dependency blast radius is confined
  to `jilva-terminal`; `jilva-core` remains fast, hermetic, and TTY-free to test.
- Positive: future extractions (`jilva-render`, `jilva-layout`, `jilva-style`,
  …) have pre-defined seams, so growth does not require re-architecture — only
  an RFC showing a seam has stabilised and a module has outgrown it.
- Negative: three crates mean three manifests and version bumps to coordinate
  pre-`1.0`; accepted as small, mechanical cost, mitigated by workspace-level
  versioning discipline owned by the implementation agent.
- Negative: pre-`0.1.0` co-evolution of lifecycle/component/renderer inside one
  crate risks organic coupling across module lines; mitigated by the §3
  boundary ownership rules and the conformance checklist (ARCHITECTURE.md §11).

## Alternatives considered

1. **Single crate with modules.** Rejected: it cannot give users a stable
   one-dependency surface independent of internal topology, and it invites
   platform-specific code and third-party terminal dependencies into the same
   compilation unit as the pure foundations, weakening testability guarantees.
2. **Full split now (render/layout/style/widgets/motion/devtools/cli crates).**
   Rejected: premature seams freeze fastest while knowledge is thinnest. Seven
   extra crates would slow RFC iteration, multiply versioning churn, and invite
   circular dependencies before the dependency direction has been validated by
   real code. The reserved-name table (§4.2) captures the upside (named growth
   paths) without the cost.
3. **Core depends on the terminal abstraction (inverted direction).** Rejected:
   it would make the pure foundations untestable without terminal machinery and
   couple every core change to backend evolution. The chosen direction keeps
   detection behind the seam and profiles-as-data flowing into core.
