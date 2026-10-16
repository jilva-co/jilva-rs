# jilva-rs

**Beautiful terminal user interfaces in Rust.** *In design — no code yet.*

`jilva-rs` is the first Jilva framework: a TUI framework for Rust that treats
beauty as a feature. Calm defaults, thoughtful components, and documentation
you actually enjoy reading.

> **Status:** we are writing RFCs before code. There is no crate, no API,
> and no release date. If you want to shape it, start with an RFC.

Part of [`jilva-co`](https://github.com/jilva-co) — beautiful frameworks for developers.

## Vision

Building a terminal UI today means choosing between raw power with rough edges
or simplicity that fights you later. `jilva-rs` aims for a third option:

- **Components that look right out of the box** — tables, forms, lists, command palettes, charts — with curated themes.
- **Layout that reads like intent** — flex and grid, not manual pixel math.
- **Motion with restraint** — tasteful transitions and live updates, never noise.
- **Correctness underneath** — Unicode, truecolor, mouse + keyboard, resize handling, testable rendering.

## Scope

In scope for `0.1.0` (to be confirmed by RFC):

- Core rendering loop and component model
- Layout (flex, grid)
- Theming
- Essential widgets (text, list, table, input, tabs, status bar)

Non-goals (at least for now):

- A GUI toolkit — terminal only
- A full application framework (config, networking, etc.)
- Tying you to a specific async runtime

Open questions live in [`docs/RFC/`](docs/RFC/).

## Roadmap

See [`docs/ROADMAP.md`](docs/ROADMAP.md). Short version:

1. RFCs (now) — scope, component model, theming
2. Prototype (later) — private branch, throwaway code to test ideas
3. Public alpha `0.1.0` (later) — only when the API survives review

## Repository layout

```text
jilva-rs/
  README.md          this file
  SECURITY.md        how to report vulnerabilities
  CONTRIBUTING.md    how to help (design stage rules)
  LICENSE-APACHE / LICENSE-MIT
  docs/
    ROADMAP.md       stages and milestones
    RFC/             design proposals (start here to contribute)
```

No `src/` yet — deliberately. Code arrives after RFC acceptance.

## Contributing

Design stage: ideas and scrutiny matter more than code.

1. Read the [RFC process](docs/RFC/README.md).
2. Open a [Discussion](https://github.com/orgs/jilva-co/discussions) before a PR.
3. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the org [Code of Conduct](https://github.com/jilva-co/.github/blob/main/CODE_OF_CONDUCT.md).

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
# no build step yet — read docs/RFC/ instead
```

## Prior art

Built with respect for `ratatui`, `cursive`, `Bubble Tea`, and `Textual` —
each proved part of the idea. Jilva's bet is that the next step is taste:
defaults, docs, and craft.

## License

Licensed under [MIT](LICENSE).
