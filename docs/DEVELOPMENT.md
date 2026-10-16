# Development guide — jilva-rs

How to get a working local checkout and run exactly what CI runs.
No invented scripts: every command below is stock `cargo` / `rustup`.

## Prerequisites

- Rust stable toolchain via [rustup](https://rustup.rs/) (rustc, cargo,
  rustfmt, clippy). The `rust-toolchain.toml` at the repo root pins the
  channel; rustup picks it up automatically.
- Git.
- No secrets, tokens, or extra services needed for Phase 1.

```bash
rustup update stable
rustc --version
cargo --version
```

## Clone

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
git checkout phase1-foundation
```

## Local checks (mirror CI 1:1)

Run these before pushing. They are the same commands CI runs, in the
same order:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo doc --workspace --no-deps
```

Docs build with warnings denied, exactly like CI:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

PowerShell (Windows) equivalent for the docs step:

```powershell
$env:RUSTDOCFLAGS = "-D warnings"
cargo doc --workspace --no-deps
Remove-Item Env:\RUSTDOCFLAGS
```

## Windows notes (PowerShell)

- Use PowerShell or `cmd`; do not rely on Unix-only shell syntax.
- Line endings: the repo uses LF (see `.editorconfig`). If Git warns about
  CRLF, keep the default checkout behavior and let editors enforce LF on
  save — do not bulk-convert files.
- Long-path builds: if `cargo` fails with path-length errors on Windows,
  enable long paths (`git config --system core.longpaths true` plus the
  Windows Group Policy / registry setting) rather than moving the checkout
  into a short path hack.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `rustfmt` / `clippy` component missing | `rustup component add rustfmt clippy` then retry |
| Stale or weird build errors after toolchain update | `cargo clean` then re-run the check sequence |
| Toolchain out of date | `rustup update` then verify with `rustc --version` |
| `cargo fmt --check` fails on files you did not touch | Line-ending drift — check your editor respects `.editorconfig` (LF) |

## Branch and commit conventions

- Active Phase 1 branch: `phase1-foundation`. Base all Phase 1 work on it.
- [Conventional Commits](https://www.conventionalcommits.org/) for messages:
  `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`, `test:`.
  Examples: `ci: add multi-OS check matrix`, `docs: document local dev loop`.
- Keep commits small and scoped; one concern per commit.
- Push to `phase1-foundation` or a short-lived `phase1-foundation/<topic>`
  branch and open a PR — never push directly to `main`.

## Branch-protection recommendation (not applied — for a maintainer to set)

In GitHub repo settings → Branches → Add rule for `main`:

- Require a pull request before merging; dismiss stale approvals on push.
- Disallow direct pushes to `main`; no bypass except repository admins.
- Require status checks to pass before merging, with these required checks
  (the five CI jobs defined in `.github/workflows/ci.yml`):
  - `fmt`
  - `check`
  - `clippy`
  - `test`
  - `doc`
- Require branches to be up to date before merging.
- Optionally: require linear history and Conventional Commit titles via a
  linter when the team grows.

Apply via the GitHub UI or API — intentionally not scripted here, since
branch protection is a remote-side setting, not a repo file.
