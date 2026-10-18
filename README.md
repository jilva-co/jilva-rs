<p align="center">
  <img src="https://raw.githubusercontent.com/jilva-co/.github/main/profile/jilva-logo.png" width="100" alt="Jilva logo" />
</p>

# jilva-rs

A Rust TUI framework for streaming CLIs, with an explicit update/view loop,
reusable components, and terminal-independent rendering.

![Jilva Studio: an actual 120 × 36 renderer frame](assets/studio-dark.svg)

Black-and-white defaults, visible keyboard focus, and working examples of
component state, responsive layout, and Unicode text.

**Unreleased (`0.0.0`).** The terminal backend, renderer, flex layout, app
runtime, components, and semantic dark/light themes work. Ready-made widgets,
inline scrollback, external streaming sources, and session recording are planned.

**[Project guide and development plan](docs/README.md)** — current capabilities,
architecture, framework comparison, milestones, and design decisions in one file.
Detailed API contracts and runnable examples live beside the Rust code.

## Try it

Requires the pinned Rust **1.98.1** toolchain (edition 2024) and Git.

```sh
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
cargo test --workspace --locked
cargo run -p jilva --example themes --locked
```

In Studio, `1` / `2` / `3` choose Counter, Layout, or Unicode. Press Tab to
focus the example, then `+` / `-` or Enter to interact; `r` resets it, `t`
switches dark/light, and `q` quits. Each example retains its component state
when switching examples, palettes, or terminal sizes. Everything runs offline.
Styles use semantic roles rather than app-defined RGB.
For smaller examples of state and focus:

```sh
cargo run -p jilva --example counters --locked
cargo run -p jilva --example counter --locked
```

For CI or a shell without an interactive terminal:

```sh
cargo run -p jilva --no-default-features --example headless_demo --locked
cargo run -p jilva --no-default-features --example view_demo --locked
cargo run -p jilva --no-default-features --example themes --locked -- --headless
```

Add `--snapshots target/studio` instead of `--headless` to export SVG/text frames
at 120×36, 80×24, and 40×18 across all four colour tiers, without a terminal.

Measure diffing, retained-frame updates, and paint + present locally:

```sh
cargo bench -p jilva --bench render_diff --locked
```

The [performance evidence](docs/README.md#performance-and-release-evidence)
records the workload and limits of these CPU measurements.

The default `crossterm` feature supplies the real terminal backend. Disabling
defaults keeps the headless backend and runtime without a terminal library.
Browse [examples](examples/) for rendering, motion, and backend demonstrations;
`cargo doc -p jilva --no-deps --open` opens the public API.

## Direction

Jilva is being developed around agent-style and other streaming CLIs:
a transcript, a multiline prompt, tool output, and an approval interaction.
The planned experience combines calm defaults, shell scrollback, and replayable
bug reports; these higher-level features are still to be built.

The core stays synchronous and independent of an async runtime. Applications
own networking and model providers; the framework owns terminal interaction.
The [comparison and milestones](docs/README.md#framework-comparison) explain
what existing frameworks already provide and what Jilva still needs to prove.

## Repository

| Path | Purpose |
| --- | --- |
| [`crates/jilva`](crates/jilva/) | Public facade used by apps, examples, and integration tests |
| [`crates/jilva-core`](crates/jilva-core/) | State, components, layout, rendering, and pure data |
| [`crates/jilva-terminal`](crates/jilva-terminal/) | Terminal I/O, capability detection, driver, and test backend |
| [`examples/`](examples/) / [`tests/`](tests/) / [`benches/`](benches/) | Runnable usage, headless verification, and measurements |

Read [CONTRIBUTING.md](CONTRIBUTING.md) for checks and review rules.
[MIT license](LICENSE) · [Security reports](SECURITY.md) · [Changelog](CHANGELOG.md).
