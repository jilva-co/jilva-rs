# exp01 — `dyn`-compatible render seam

## Hypothesis (H1)

An object-safe render-only trait supports heterogeneous `Vec<Box<dyn Render>>`
children across all composition scenarios. The full RFC-0002-style `Component`
trait (associated `State`/`Message` + receiver-less `init`/`update`) can NEVER
be a `dyn` object, so `column([Header, TodoList, StatusBar])` cannot compile
as sketched: the array literal has no common element type (E0308) and no
erased element type exists for that trait shape (E0191/E0038).

## Toolchain

- `rustc 1.98.1 (48a229cea 2026-09-01)`, edition 2024, stable, std-only.
- Detached crate: empty `[workspace]` in `Cargo.toml`; `forbid(unsafe_code)`.

## Commands

```text
cargo test            # 8/8 pass
rustc --edition 2024 --crate-type=lib src/negative_not_dyn_compatible.rs
                      # EXPECTED FAILURE (evidence, exit != 0)
```

## Results

Positive (`cargo test`, 8 passed):

| # | Scenario | Verdict |
|---|----------|---------|
| 1 | static homogeneous list | renders |
| 2 | heterogeneous `Header + TodoList + StatusBar` (the RFC sketch, with explicit `Vec<Box<dyn Render>>` element type) | renders |
| 3 | nested columns (`Panel` holding erased children) | renders |
| 4 | conditional children (`if flag push`) | renders iff flag |
| 5 | dynamic add/remove across frames | renders |
| 6 | reordered children | render order follows vec order |
| 7 | owned descriptions retained across frames (`frame_n` alive during `frame_n+1`) | no borrow escape |
| 8 | `dyn SharedState<State=i64, Message=i64>` holding two DISTINCT types | compiles — proving `dyn`+assoc-types forces homogeneity |

Negative (compiler output, verbatim excerpts):

- `let _kids = [Header, TodoList];` → `error[E0308]: mismatched types ...
  expected Header, found TodoList`.
- `fn erased(_: &dyn Component)` → `error[E0191]: the value of the associated
  types State and Message in Component must be specified`.
- `fn erased(_: &dyn Component<State = String, Message = char>)` → 
  `error[E0038]: the trait Component is not dyn compatible ...
  because associated function init has no self parameter`
  (same for `update`; rustc suggests `where Self: Sized` quarantine — the
  technique test 8 validates).
- Incidental findings while building: `Box<dyn Render + Send>` requires
  `T: 'static` (E0310 otherwise); even a `render(slot: &State)` associated
  function without `&self` trips E0038 — object safety demands a receiver.

## Allocation note

One `Box` (two words: data ptr + vtable) per heterogeneous child, allocated
at vec construction. Re-render borrows (`&[Box<dyn Render>]`) allocate
nothing. Measured allocation behaviour across mount/update/removal is
exp03's subject, not this crate's.

## Limits

- `Render` carries no state/message channel by construction; identity and
  update routing need the separate slot channel (exp03).
- `dyn` requires `'static` widgets (no borrowed props); borrowed props would
  need generics, which would break object safety again.
- Dynamic dispatch cost (vtable indirection, no inlining) not measured here;
  the homogeneous fast path is exp02's subject.
