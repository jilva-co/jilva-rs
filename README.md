<p align="center">
  <img src="https://raw.githubusercontent.com/jilva-co/.github/main/profile/jilva-logo.png" width="120" alt="Jilva logo" />
</p>

<h1 align="center">jilva-rs</h1>

<p align="center"><strong>Jilva — Beautiful terminal applications, effortlessly.</strong></p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <img src="https://img.shields.io/badge/toolchain-1.98.1-orange.svg" alt="Rust toolchain: stable 1.98.1" />
  <img src="https://img.shields.io/badge/edition-2024-lightgrey.svg" alt="Rust edition: 2024" />
</p>

`jilva-rs` is the first Jilva framework: a terminal UI (TUI) framework for Rust
that treats beauty as a feature. Calm defaults, thoughtful components, and
documentation you actually enjoy reading.

> **Status:** Phase 2 — Terminal Engine, implemented and verified. Synchronous
> Crossterm integration, typed input/output, capability profiles, and a
> deterministic headless backend are implemented. Phase 3 — Rendering Engine
> (grapheme-aware cell buffer, damage tracking, differential output) is
> implemented on the `phase3-rendering-engine` branch and awaits review of
> [RFC-0004](docs/RFC/0004-rendering-engine.md). The framework is unreleased
> (`0.0.0`); the application runtime, layout engine, and widgets are future
> work. See [`docs/PHASE2_TERMINAL_ENGINE.md`](docs/PHASE2_TERMINAL_ENGINE.md)
> for the supported terminal contract and limitations.

Part of [`jilva-co`](https://github.com/jilva-co) — beautiful frameworks for developers.

## Philosophy

- **Beauty as a feature, correctness underneath.** Defaults carry the taste;
  the foundations carry Unicode, input, resize, and restoration semantics.
- **Intent over mechanics.** Users describe *what* (`row`, `accent`,
  `on_select`), never *how* (coordinates, ANSI codes, escape sequences).
- **Small stable seams, free interiors.** Crate boundaries are narrow and
  documented; interiors may evolve within semver.
- **No ambient global state.** Terminal mode, styling context, and app state
  are passed explicitly, so rendering stays testable without a real terminal.
- **No async runtime in the core contract.** Synchronous operation is the
  baseline; async integration, if any, is an opt-in layer above the core.
- **Docs are part of the API.** Every public item is documented with a runnable
  example; if a sentence here confuses you, that is a bug.

## Why Jilva

Building a terminal UI today means choosing between raw power with rough edges
or simplicity that fights you later. `jilva-rs` aims for a third option:

- **Components that look right out of the box** — tables, forms, lists, command
  palettes, charts — with curated themes.
- **Layout that reads like intent** — flex and grid, not manual cell math.
- **Motion with restraint** — tasteful transitions and live updates, never noise.
- **Correctness underneath** — Unicode graphemes, double-width and emoji
  handling, truecolor with graceful degradation, mouse + keyboard, resize
  handling, testable rendering.

Built with respect for `ratatui`, `cursive`, `Bubble Tea`, and `Textual` —
each proved part of the idea. Jilva's bet is that the next step is taste:
defaults, docs, and craft. Jilva positions itself as one more experiment in
that lineage, not as a replacement.

## Rust first, future languages later

Rust is the reference implementation: the architecture, component model, and
design system are proven here first. Sibling frameworks (`jilva-go`,
`jilva-ts`) start only after the Rust alpha is genuinely usable — see
[`docs/ROADMAP.md`](docs/ROADMAP.md). Lessons, RFCs, and the theme vocabulary
are shared; each language keeps its own idiomatic API.

## Scope

In scope for `0.1.0` under accepted RFC-0001/0002/0003:

- Core run loop and component model
- Layout (flex, grid)
- Theming
- Essential widgets (text, list, table, input, tabs, status bar)

Non-goals (at least for now):

- A GUI toolkit — terminal only
- A full application framework (config, networking, etc.)
- Tying you to a specific async runtime

Open questions live in [`docs/RFC/`](docs/RFC/). The MSRV number itself is
RFC-track; until that RFC lands, code must build on stable **1.98.1** with no
nightly features.

## Getting started (contributors)

Prerequisites: Git and Rust stable **1.98.1** (edition 2024). New to the repo?
Start with [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md); workspace
workflow details live in [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
cargo check --workspace
cargo test --workspace
```

Run the real terminal demonstration from an interactive terminal. Type keys,
resize the window, and exit with `q`, Escape, or Ctrl+C:

```bash
cargo run -p jilva --example backend_demo
```

See the rendering engine work without a terminal (paint, diff, and the
instruction stream that crosses the backend seam):

```bash
cargo run -p jilva --no-default-features --example render_demo
cargo bench -p jilva --bench render_diff
```

For CI and redirected output, use the headless example. Disabling default
features excludes Crossterm and its OS dependencies:

```bash
cargo run -p jilva --no-default-features --example headless_demo
```

Targeted checks while working on one crate:

```bash
cargo check -p jilva
cargo check -p jilva-core
cargo check -p jilva-terminal
```

## Repository structure

The workspace layout (see `docs/ARCHITECTURE.md` §7):

```text
jilva-rs/
  Cargo.toml                  # workspace manifest ONLY (members + shared lints)
  LICENSE                     # MIT
  README.md / SECURITY.md / CONTRIBUTING.md / CHANGELOG.md
  docs/
    ARCHITECTURE.md           # system boundaries, crates, dependency rules
    API_DESIGN.md             # API principles and facade contract
    ROADMAP.md                # the 7 phases
    GETTING_STARTED.md        # contributor setup
    DEVELOPMENT.md            # workspace workflow
    RFC/                      # design proposals (start here to contribute)
    adr/                      # accepted architectural decisions
  crates/
    jilva/                    # public facade: re-exports + docs only, no logic
    jilva-core/               # foundations: lifecycle, component, events,
                              # layout, renderer model, style, error (modules)
    jilva-terminal/           # optional Crossterm backend, detection, test double;
                              # the only crate that performs terminal I/O
  examples/                   # one tiny example per concept; facade-only imports
  tests/                      # workspace integration tests (golden-output, no TTY)
  benches/                    # std-only micro-benchmarks (render_diff)
```

Conventions: `examples/` and `tests/` import only the `jilva` facade, never
`jilva-core` or `jilva-terminal` directly. No `src/` at the workspace root. No
additional top-level crates in Phase 1 — reserved names (`jilva-render`,
`jilva-layout`, `jilva-style`, `jilva-widgets`, `jilva-motion`,
`jilva-devtools`, `jilva-cli`) are integration points only, created solely via
the extraction rule in `docs/ARCHITECTURE.md` §4.2.

## Architecture overview

The full contract is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); the
public-surface rules are [`docs/API_DESIGN.md`](docs/API_DESIGN.md). Short
version:

- **`jilva`** is the only crate users depend on. It re-exports a curated
  surface and contains no logic.
- **`jilva-core`** holds the framework foundations (app lifecycle, component
  contract, state/event propagation, layout protocol, renderer buffer/diff
  model, style tokens, error taxonomy). It is platform-agnostic, names no
  backend, and must stay testable with no TTY.
- **`jilva-terminal`** owns everything that touches a real terminal behind a
  narrow backend seam, plus capability detection and an in-memory test double.
- The dependency graph is acyclic: user app → `jilva` → `jilva-terminal`
  → `jilva-core`, with a direct facade → core edge. Shared events, output
  commands, and capability data live in core, which has no dependencies.
- Rendering is a pure function of (previous buffer, new frame description) plus
  an explicit capability profile — so golden-output tests run headless.
- RFC-2 accepts the component model and RFC-3 the theming architecture.
  Their runtime implementations belong to subsequent roadmap milestones.

## Roadmap

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the full plan. No dates — phases
move when the work is honestly done.

| Phase | Name | State |
|---|---|---|
| 1 | Foundation | Complete |
| 2 | Terminal Engine | Implemented and verified |
| 3 | Rendering Engine | Implemented on branch; RFC-0004 awaiting decision |
| 4 | Layout & Components | In progress (flex solver slice; RFC-0005 Draft) |
| 5 | State & Interactivity | Planned |
| 6 | Design System | Planned |
| 7 | Alpha Release (`0.1.0`) | Planned |

Breaking changes are expected through `0.x`. `1.0` ships only when the API is
stable and proven in real apps.

## Contributing

Architectural changes follow the maintainer-driven RFC process; public
Discussions remain welcome without a mandatory waiting period.

1. Read the [RFC process](docs/RFC/README.md).
2. Open a [Discussion](https://github.com/orgs/jilva-co/discussions) before a PR.
3. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the org
   [Code of Conduct](https://github.com/jilva-co/.github/blob/main/CODE_OF_CONDUCT.md).

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
# design discussion first — see docs/RFC/; code follows accepted RFCs
```

## Prior art

Built with respect for `ratatui`, `cursive`, `Bubble Tea`, and `Textual` —
each proved part of the idea. Jilva's bet is that the next step is taste:
defaults, docs, and craft.

## License

Licensed under [MIT](LICENSE). Security reports: see [SECURITY.md](SECURITY.md).
Changes are tracked in [CHANGELOG.md](CHANGELOG.md).
