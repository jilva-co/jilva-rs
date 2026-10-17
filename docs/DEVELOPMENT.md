# Development guide — jilva-rs

The workspace pins Rust 1.98.1, edition 2024, in `rust-toolchain.toml`.
Install that toolchain with rustfmt and clippy through rustup. No async
runtime, service credentials, or real terminal is required for headless tests.

```bash
git clone https://github.com/jilva-co/jilva-rs.git
cd jilva-rs
rustup toolchain install 1.98.1 --component rustfmt --component clippy
cargo check --workspace --all-targets --locked
```

Work on a topic branch; the Phase 2 branch is `phase2-terminal-engine`.
Do not push directly to main. Use scoped Conventional Commits and a pull
request. The workspace lockfile is committed for reproducible backend,
example, and CI checks; consumers of the library use their own resolution.

## Quality gates

```bash
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --doc --locked
cargo check --workspace --all-targets --no-default-features --locked
cargo test --workspace --all-targets --no-default-features --locked
cargo run -p jilva --no-default-features --example headless_demo --locked
```

The real backend is behind the default `crossterm` feature. The headless
build uses only workspace crates and the standard library. CI also checks
`--all-features` and strict documentation on Ubuntu, Windows, and macOS.
Ordinary tests never open or modify a real terminal.

Strict docs on Unix:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
```

Strict docs on PowerShell:

```powershell
$env:RUSTDOCFLAGS = "-D warnings"
cargo doc --workspace --no-deps --locked
cargo test --workspace --doc --locked
Remove-Item Env:\RUSTDOCFLAGS
```

When testing only one crate, use `cargo test -p jilva-core` or
`cargo test -p jilva-terminal`. Integration tests and examples depend on
the public `jilva` facade. See [PHASE2_TESTING.md](PHASE2_TESTING.md) for
scripted failures, command goldens, and the platform verification record.

## Real terminal demonstration

```bash
cargo run -p jilva --example backend_demo --locked
```

Run from an interactive terminal with terminal input and output attached.
The demo acquires raw mode and supported session modes, prints through
typed commands, reads input, and reports resize events. Exit with `q`,
Escape, or Ctrl+C. The backend performs restoration on explicit leave and
best-effort restoration on Drop, including panic unwinding.

Manual checklist for each target terminal:

1. Record OS, terminal/version, command, and outcome.
2. Run the demo, type printable keys and arrows, and verify the status.
3. Resize and verify columns/rows update. Test a very small window.
4. Exit via each documented shortcut; confirm shell echo and cursor work,
   and the alternate screen has been left.
5. If testing mouse/focus/paste options, enable them explicitly in the
   backend options, verify reports, and verify their teardown.

A successful headless build does not prove visual behavior. Forced kill,
abort, power loss, and unrecoverable output failure cannot guarantee
restoration. Prior cursor visibility and externally owned terminal modes
cannot be discovered and reconstructed universally; follow the ownership
contract in [PHASE2_TERMINAL_ENGINE.md](PHASE2_TERMINAL_ENGINE.md).

## Restoration troubleshooting

Use explicit `leave()` when an error needs to be inspected. A failed cleanup
keeps unsuccessful resource flags for a later leave/Drop attempt, while
still attempting the other cleanup steps. When both setup and rollback
fail, diagnostics retain the primary failure and restoration context.

If a process was forcibly terminated, recover from the shell using the
terminal's reset action; on Unix, `stty sane` and `reset` can restore a
usable shell. On Windows, opening a new terminal tab gives a fresh console.
These are recovery instructions, not actions run automatically by tests.

In headless tests inspect `call_log()`, mode flags, captured typed commands,
and restoration-attempt counts. They identify the exact failing operation
without changing process terminal state.

## Windows and CI

Use PowerShell-compatible commands. Keep LF source endings per
`.editorconfig`. If incremental compilation cannot hard-link in a
restricted workspace, set `CARGO_INCREMENTAL=0` for that local check; CI
uses its normal caches. Do not change the pinned Rust version to repair a
local tool installation.

CI defines the same headless quality matrix on Ubuntu, Windows, and macOS.
A configured job is not a passed job: record the actual run URL and result.
Live terminal tests must be opt-in and isolated from ordinary CI.
