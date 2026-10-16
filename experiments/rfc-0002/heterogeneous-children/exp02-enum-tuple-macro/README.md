# exp02 — enum dispatch, tuple composition, declarative macros

## Hypothesis (H2)

(a) Enum dispatch = allocation-free static dispatch for a CLOSED widget set,
priced in extensibility. (b) Tuples = zero-box fixed-arity heterogeneous
composition, priced in an arity wall. (c) `macro_rules!` constructors are pure
sugar over the macro-free `Vec<Box<dyn Render>>` form.

## Toolchain

- `rustc 1.98.1`, edition 2024, stable, std-only, `forbid(unsafe_code)`.
- Detached crate: empty `[workspace]`.

## Commands

```text
cargo test            # 6 passed
```

## Results

Positive (`cargo test`, 6 passed):

| # | Scenario | Verdict |
|---|----------|---------|
| 1 | enum renders all variants | ok |
| 2 | `size_of` layout (MEASURED, `std::mem::size_of`) | `AnyWidget`=24B (niche-filled, NO tag word — better than the predicted 32), `Box<dyn Render>`=16B stack + heap, `Child`=24B |
| 3 | tuple pair/triple heterogeneous render, zero `Box` | ok |
| 4 | `column![...]` expansion byte-identical to manual `vec![Box::new...]` | ok — macro-free normative core exists |
| 5 | third-party `Fancy` joins `Vec<Box<dyn Render>>` freely; joins `Child` via `Boxed` fallback; can NEVER inhabit `AnyWidget` without editing framework code | open vs closed, demonstrated |
| 6 | hybrid `Child` vec: nested/dynamic/reordered/removed | ok |

Two predictions were wrong in the good direction: both `AnyWidget` and
`Child` niche-fill to 24B (pointer niches absorb the discriminant), so the
inline fast path is cheaper than the reasoned estimate. Values asserted in
code — any layout change fails the test instead of silently rotting.

## Allocation note

- Enum/tuple paths: ZERO heap allocation for composition itself (payload
  Strings aside — app data, not framework overhead).
- Hybrid: exactly one `Box` per `Child::Boxed` child, at construction.
- `Vec<Child>` backing store (re)allocates on push beyond capacity —
  caller-owned description cost, amortised, bounded by child count.

## Limits

- Tuples: impls provided for arities 2–3; production needs N (macro-generated
  impls or a builder) — the wall is real but movable with `macro_rules!`.
- Enum match dispatch is static but not inlined across crate boundaries
  without LTO; no benchmarks run (no criterion harness in scope).
- `column!` returns `Vec` (heap); a stack-fixed alternative (`[..; N]`)
  cannot mix types — documented, not solved.
