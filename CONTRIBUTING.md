# Contributing

Start with the [project guide](docs/README.md) for implemented behaviour,
boundaries, and the next milestone. Small documentation fixes can go straight
to a PR. For API, lifecycle, dependency, or scope changes, include the proposal
and evidence in the PR, then record the maintainer decision in the guide.
Discussions are welcome; no external waiting period is required.

Use the toolchain in `rust-toolchain.toml`. Run the CI checks that apply:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy --workspace --all-targets --no-default-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --all-targets --no-default-features --locked
cargo test --workspace --doc --all-features --locked
cargo test --workspace --doc --no-default-features --locked
cargo doc --workspace --all-features --no-deps --locked
cargo doc --workspace --no-default-features --no-deps --locked
```

For rustdoc checks, set `RUSTDOCFLAGS=-D warnings` using your shell's environment
syntax. [CI](.github/workflows/ci.yml) also checks feature combinations and
runs headless demos on Linux, macOS, and Windows.

Keep terminal I/O in `jilva-terminal`, pure behaviour in `jilva-core`, and the
public entry point in `jilva`. Examples/tests/benches use facade imports.
Unsafe code is forbidden; recoverable failures return errors rather than panic.
Test changed behaviour and error paths; document public APIs beside their code.
Public seam/error changes need two reviewers. New dependencies need a reason.

Use Conventional Commits and keep each PR focused. Update the existing guide
instead of adding separate plans, RFC files, or audit reports.
[Code of Conduct](https://github.com/jilva-co/.github/blob/main/CODE_OF_CONDUCT.md)
applies. Report vulnerabilities through [SECURITY.md](SECURITY.md).
