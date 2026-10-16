# exp04 — `Send` policy for the single-threaded loop

## Hypothesis (H4)

A universal `Send` bound on models/slots excludes idiomatic single-threaded
interiors while buying nothing for a loop with no executor. `Send` belongs
only at the async-bridge boundary, on completion values crossing threads.

## Toolchain

- `rustc 1.98.1`, edition 2024, stable, std-only (`thread` in one test only;
  no Tokio, no executor, no proc macros).
- Detached crate: empty `[workspace]`; `forbid(unsafe_code)`.

## Commands

```text
cargo test                                        # 5 passed
rustc --edition 2024 --crate-type=lib src/negative_send_bound.rs
                                                  # EXPECTED FAILURE (E0277 x2)
```

## Results

Positive (`cargo test`, 5 passed):

| # | Scenario | Verdict |
|---|----------|---------|
| 1 | `Rc<RefCell<Vec>>` memo + `Cell<usize>` cursor in model AND slot, mutated via `&mut` and interior | works, zero friction |
| 2 | `PlainModel`, plain slot, `Cell<usize>` are `Send` | `assert_send` compiles |
| 3 | unbounded `run_update(&mut S, …)` over `SharedModel` (`!Send`) | compiles + runs |
| 4 | `Bridge::spawn_job` → `JoinHandle<Out>`; completion joined + applied on loop thread; `!Send` state never crosses | works |
| 5 | `Effect::Task` routed by core without executing/spawning/naming threads; executed later on a worker; `After`/`None` are pure data | works — no executor-by-stealth |

Negative (verbatim excerpts):

- `require_send(SharedModel{ rows: Rc<RefCell<..>> })` →
  `error[E0277]: Rc<RefCell<Vec<String>>> cannot be sent between threads safely`.
- `require_send(Rc<String>)` →
  `error[E0277]: Rc<String> cannot be sent between threads safely`.

Either error is what EVERY single-threaded widget author hits under a
universal `Send` bound. That is the whole policy argument in two lines.

## Policy evaluation (evidence-backed)

| Policy | `Rc`/`RefCell`/`Cell` usable? | Bridge shape | Verdict |
|---|---|---|---|
| (a) no universal `Send` (core unbounded) | yes (#1, #3) | possible, unchanged | compatible with everything below |
| (b) `Send` at async boundary only (RECOMMENDED = a + bounds on `Effect::Task` job + completion) | yes | `F: Send`, `Out: Send`; state never crosses (#4, #5) | cheapest sufficient: rule out only what actually travels |
| (c) `Send` for models | NO (#neg-1) | bridge trivially fine — at the cost of banning (a)'s ergonomics | pays the tax where the benefit isn't: models never cross threads in a single-threaded loop |
| (d) `Send` for all (models + slots) | NO (#neg-1, #neg-2) | same as (c), plus slot memos banned | strictly worse than (c); no measured benefit (no multi-threaded rendering: non-goal) |

`Sync` is not required by ANY policy: the loop holds `&mut` on one thread;
shared references never cross threads either. `Cell<usize>` being `Send`
(test #2) further shows even thread-transferable interiors survive (b).

## Limits

- `std::thread::spawn` stands in for "any executor"; Tokio-specific bounds
  (`Send` futures) are out of scope and undesired in core by ARCHITECTURE §2.4.
- No measurement of cross-thread throughput: irrelevant — the loop is
  single-threaded by RFC contract; the bridge moves completions, not frames.
- `Effect::After(u64, M)` timer delivery mechanism (who ticks) is lifecycle-RFC
  territory (clock ownership X5), not decided here.
