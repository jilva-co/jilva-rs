# exp03 — generational slot table + erased slots

## Hypothesis (H3)

A `Vec`-backed generational table holding `Box<dyn ErasedSlot>` gives
mount-once boxing with zero slot allocation in steady state; keyed identity
preserves state across reorder/add/remove; collisions and keyed/unkeyed mixing
degrade observably (outcome flags + documented fallback), never silently.

## Toolchain

- `rustc 1.98.1`, edition 2024, stable, std-only (`HashMap`, `Any` only).
- Detached crate: empty `[workspace]`; `forbid(unsafe_code)` — the table is
  safe indexed access throughout. (A counting `GlobalAlloc` was tried for
  allocation measurement and REMOVED: it needs `unsafe impl`, contradicting
  the crate's own posture. Capacities + ledger replace it — see below.)

## Commands

```text
cargo test                                        # 10 passed
cargo test allocation_map -- --nocapture         # prints ALLOC-MAP line
```

## Results

Positive (`cargo test`, 10 passed):

| # | Scenario | Verdict |
|---|----------|---------|
| 1 | persistent local state (10 frames, seed changes ignored after mount) | stable `SlotId`, state intact, `mounts==1` |
| 2 | keyed reorder | slots follow keys, no remount |
| 3 | add/remove | `(mounted, dropped)` = `(1,0)` / `(0,2)` exactly |
| 4 | key collision | first wins stably, extra stateless, `collisions` counted, first's state untouched |
| 5 | unkeyed reorder | state stays at POSITION (documented positional semantic, contrast with #2) |
| 6 | type change at same key | `(mounted, dropped)` = `(1,1)`; old id unusable |
| 7 | stale handle + index recycle | `get` → `None`; recycled index gets bumped generation (`old != new`) |
| 8 | mixed keyed/unkeyed list | `mixed_keys_fallback=true`, `mixed_fallbacks==1`, reconciliation still total; clean list clears flag |
| 9 | allocation map (64 slots, 200 steady frames) | `slot_boxes=64 at mount, +0 steady`; capacities `(64,112,0)` UNCHANGED; `drops=0` |
| 10 | collision ephemeral factory cadence | 1 table mount + 1 ephemeral/frame (stateless by construction) |

Verbatim ALLOC-MAP line:

```text
ALLOC-MAP: mounts=64 slot_boxes=64 steady_reconciles=200 added_boxes=0 capacities=(64,112,0) drops=0
```

## Allocation map (where allocations happen)

| Phase | What allocates | Count (64-child case) |
|---|---|---|
| mount | one `Box<dyn ErasedSlot>` per child + table/map backing growth | 64 boxes; capacities settle at (64,112,0) |
| update (steady reconcile) | NOTHING framework-side: lookups only; `ChildDesc` vec is caller-owned description cost, O(children) | 0 boxes, 0 growth over 200 frames |
| render | out of scope here (exp01/02: borrow-only) | — |
| removal | deallocation only (cleanup point runs where the slot drops) | `drops` counted |
| collision extra | one ephemeral box PER FRAME, dropped end of frame (intentional: statelessness enforced by allocation cadence) | 1/frame/collision |

Claim discipline: counts 1–4 are hard asserts in code. Caller-side
description cost (`Vec<ChildDesc>` per frame) is NOT eliminated — it is owned
data, bounded by child count, and distinct from slot storage.

## Limits

- Single sibling list per `reconcile(parent, ..)` call; nesting = parent-scoped
  keys (`(parent, key)` map) — depth composition is by construction, not
  stress-tested with deep trees.
- Sweep pass is O(table) per reconcile; production keeps per-parent live sets.
- `Key(String)` hashes per lookup; small-key optimisation (inline/arc) deferred
  to implementation with a benchmark.
- No concurrency: table is `!Sync` by interior (`HashMap` + `Vec` are `Send`
  but the reconciler takes `&mut`); `Send` policy is exp04's subject.
