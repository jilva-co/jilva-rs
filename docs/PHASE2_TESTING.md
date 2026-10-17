# Phase 2 testing

The backend contract is specified in [PHASE2_BACKEND_CONTRACT.md](PHASE2_BACKEND_CONTRACT.md).
Both backend implementations use the same core-owned input, output, and
`TerminalCapabilities` types. Tests under `tests/` import only the `jilva` facade.

## Automated coverage

| Area | Evidence | Scope |
| --- | --- | --- |
| Typed input FIFO | `scripted_input_preserves_unicode_key_kinds_and_all_modifiers` | Complete cluster strings, press/repeat/release kinds, all key and mouse modifiers, focus and paste payloads |
| Disabled input modes | `source_readiness_can_filter_all_unsupported_units_without_an_error` | Mouse, focus, and paste require their advertised input mode; invalid resize and undecodable units are skipped |
| Fair input filtering | `unsupported_input_filtering_is_bounded_and_eventually_reaches_valid_input` | At most 64 source units per read; later input remains reachable |
| Input boundary | `input_added_during_a_session_is_visible_to_a_successful_poll_or_read` | Script pushes become observable at the next successful poll or read; injected failures consume nothing and FIFO order remains intact |
| Empty input | `empty_queue_reports_no_input_for_zero_and_nonzero_timeouts` | `Ok(false)` / `Ok(None)`, with no sleeping in the double |
| Resize causality | `consumed_resize_updates_size_and_only_the_screen_capability_domain` | Size and screen geometry update when consumed; only changed geometry advances the screen version and snapshot ID |
| Capability injection | `injected_capability_tiers_are_cached_without_environment_probes` | All four colour tiers use explicit snapshots; repeated observations do not detect again |
| Typed golden output | `submitted_commands_preserve_complete_payloads_and_fifo_across_batches` | Exact command order, RGB/palette values, every attribute, rectangles, and Unicode text survive buffering and flush |
| Validation atomicity | `invalid_print_data_rejects_the_entire_batch_without_losing_prior_output` | Directly constructed commands containing C0, DEL, or C1 controls reject the entire batch before append |
| Geometry validation | `out_of_bounds_commands_reject_the_entire_submit_before_buffering`, `resize_before_flush_revalidates_and_discards_the_entire_stale_batch` | Cursor/rectangle bounds reject the whole batch; a shrink before flush discards every stale command before capture |
| Flush boundaries | `successful_flushes_record_exact_batches_including_empty_output` | Exact typed batches and empty successful flush markers |
| Output failures | `injected_submit_failure_keeps_prior_buffer_and_allows_a_fresh_retry`, `failed_flush_discards_attempted_output_and_retry_cannot_replay_it` | Submit preserves prior pending output; attempted flush discards pending output, including on failure |
| Session reset | `repeated_sessions_advance_snapshot_identity_without_stale_work` | Successful leave discards ready/staged input and unflushed output; previous captured records remain inspectable |
| Invalid geometry | `degenerate_sizes_fail_and_explicit_reconfiguration_recovers` | Enter rejects zero dimensions before enabling modes; invalid live size observations fail and explicit test reconfiguration recovers |
| Partial setup and cleanup | `failed_rollback_blocks_reenter_until_cleanup_retries_the_dirty_mode`, `failed_cleanup_attempts_later_modes_and_retains_the_failed_mode_for_retry` | All enabled modes are attempted in reverse order; failed flags remain retryable and block new setup |
| Panic and Drop | `panic_unwind_restores_all_enabled_modes`, double unit test `drop_cleanup_uses_failure_injection_and_can_retry_dirty_setup` | Mode state proves cleanup; Drop uses the same injected-failure path and never unwinds on an I/O error |
| Deterministic replay | `replay_records_identical_typed_streams_and_batch_boundaries` | Two independent runs yield equal events, commands, batches, call logs, and snapshots |

Input/output tests live in `tests/backend_io.rs`; session tests live in
`tests/backend_session.rs`. Double unit tests additionally inspect dirty-mode
cleanup and absolute indexed failures. These tests require no terminal, stdin,
PTY, process environment mutation, or async runtime. Production backend unit
tests cover its encoder, decoder, setup, cleanup, and writer failure paths
using injected terminal operations; they do not establish real terminal parity.

Poll reports source readiness, not a promise that a supported decoded event
exists. A source containing only unsupported or disabled events may produce
`poll == true` followed by `read == None`. Callers should continue their loop.

The output policy deliberately distinguishes rejection from delivery attempts:
an invalid or failed submit does not append any prefix; an attempted flush
removes the pending batch even when it returns an error. Real OS writes may
have delivered an unknown prefix. A caller can resubmit a fresh complete frame,
but an empty retry must not replay the failed batch automatically. The double
records only successful commits, not a claim about bytes delivered on failure.

`consumed_events`, `buffered_commands`, `captured_commands`, and
`flushed_command_batches` expose the full typed values. `consumed`, `captured`,
and `flushed_batches` are readable diagnostic descriptions; colour values and
modifier details must be asserted through the typed inspections.

## Failure injection

Rules are independent per operation. Call indices are absolute, one-based,
per-operation attempt counts, including calls rejected outside a session.

| API | Behavior |
| --- | --- |
| `fail_enter` / `fail_enter_once` | Fail before enabling modes or minting a snapshot |
| `fail_enter_partial` | Fail after modeled setup; attempt rollback of every enabled mode in reverse order |
| `fail_rollback_once` | Fail the first active partial-setup restoration stage; later stages still run and `leave` can retry the dirty flag |
| `fail_poll` / `fail_poll_once` | Keep ready and staged input unchanged |
| `fail_read` / `fail_read_once` / `fail_read_at` | Consume no source unit on the failed read |
| `fail_submit` / `fail_submit_once` | Keep prior buffered commands unchanged |
| `fail_flush` / `fail_flush_once` / `fail_flush_at` | Discard attempted pending output; record no successful capture or batch |
| `fail_leave` / `fail_leave_once` | Make the session unusable, fail the first active restoration stage, attempt later stages, retain failed mode state for retry |
| `inject` / `inject_once` / `inject_at` | Generic schedules for all six operations |
| `clear_failure` / `clear_failures` | Remove one operation's rules or every rule; clearing enter also clears partial-setup and rollback rules |

`failures_configured` describes armed rules, `op_calls` reports attempt counts,
and `call_log` explains operation and restoration order. Raw mode, alternate
screen, mouse capture, focus reporting, bracketed paste, and keyboard enhancement
have separate state getters. `is_entered == false` after failed setup does not
prove complete restoration: inspect dirty mode flags and retry `leave`. Starting
cleanup makes the session unusable even if one restoration fails; subsequent
input/output calls reject it until cleanup succeeds and a new session enters.
Setup errors retain their original kind and include rollback failure context.

## Commands and CI

The repository pins Rust **1.98.1** in `rust-toolchain.toml`; CI installs that
exact version. The quality matrix is configured for Windows, Linux, and macOS.
Each platform checks default, all-feature, and no-default-feature builds, runs
strict Clippy and headless tests with and without default features, runs the
headless demo, and builds strict rustdoc plus documentation tests. Formatting
runs in its own pinned-toolchain job. Real terminal sessions are not opened by CI.

Representative local commands:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --all-targets --no-default-features --locked
cargo test --workspace --doc --all-features --locked
cargo doc --workspace --all-features --no-deps --locked
cargo run -p jilva --example headless_demo --no-default-features --locked
```

Set `RUSTDOCFLAGS=-D warnings` for the documentation commands, as CI does.
The no-default-feature path excludes the optional crossterm dependency and skips
the real-terminal example through its required feature declaration.

Validation on **2026-10-17**: local Windows full gates passed, with 111
unit/integration tests, 27 doctests and a separate passing opt-in real-terminal
test. The no-default-feature suite passed 84 tests. These overlapping feature
runs are not added together as distinct tests. Strict formatting, Clippy and
rustdoc passed. Ubuntu, Windows and macOS jobs all completed successfully in
[CI run 39000026013](https://github.com/jilva-co/jilva-rs/actions/runs/39000026013)
for source commit `9fef589`. The earlier formatting failure and its correction
are recorded in [the delivery report](reviews/PHASE2_CONTINUATION_REPORT.md).
Linux and macOS were executed remotely in headless CI, not locally or through
an interactive emulator.

## Real terminal checks

Run the real demo from an interactive terminal:

```text
cargo run -p jilva --example backend_demo
cargo run -p jilva --example backend_demo -- --mouse --focus --paste
```

The input modes in the second command are opt-in. Confirm the printed
capability snapshot matches modes that were actually established. Type normal
text and key combinations, resize repeatedly, test mouse buttons/drag/scroll,
switch terminal focus, and paste both ordinary text and text containing control
characters. The diagnostic display must escape input before printing it.
Exit with `q`, Escape, and Ctrl+C in separate runs; each must restore the
terminal. Confirm cursor visibility, normal shell line editing/echo, and exit
from the alternate screen. Repeat under Windows Terminal, a Linux terminal
(e.g. xterm or kitty), and a macOS terminal. Record terminal version, OS, enabled
modes, and observed results; do not infer platform support from one host.

Unwinding and error handling can attempt cleanup. Forced termination, power
loss, and an OS crash cannot run Rust Drop guards. Real TTY behavior, keyboard
protocol negotiation, terminal focus/paste integration, colour rendering, and
shell restoration still require manual or PTY evidence. No such interactive
platform matrix is claimed by the focused headless tests above.

Orchestrator's actual Windows ConPTY checks (2026-10-17): the real demo received
the initial 80×24 resize and an injected `h` key, then exited successfully on
`q`. Separate sessions exited successfully on Escape with mouse/focus/paste
options enabled and on Ctrl+C, after which the enclosing shell printed a
completion marker. Output included the expected alternate-screen/cursor and
requested-mode teardown. The ignored
`real_terminal_enter_leave_and_panic_unwind_smoke` test was then explicitly run
with `TERM=xterm-256color`, `--ignored --test-threads=1 --nocapture`: one test
passed, asserting raw mode before/after explicit leave, panic-unwind Drop, and
subsequent session reacquisition. Its caught intentional panic is expected.
These are PTY behavior checks; physical window resizing, actual mouse gestures,
focus changes, paste integration and visual appearance were not manually
verified across terminal emulators. The test remains ignored in normal CI.
