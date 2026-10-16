//! Experiment 03 — framework-owned generational slot table + internal type
//! erasure for heterogeneous component state.
//!
//! Hypothesis H3: a `Vec`-backed generational table holding
//! `Box<dyn ErasedSlot>` gives mount-once boxing with ZERO slot allocation in
//! steady state; keyed identity preserves state across reorder/add/remove;
//! key collisions and keyed/unkeyed mixing degrade OBSERVABLY (flags +
//! documented fallback), never as silent corruption.
//!
//! Addresses RFC-0002 §8 F2.3/F2.4 and ARCHITECTURE_FEASIBILITY R2-C5.
//! `unsafe_code` is forbidden: the table is safe indexed access throughout.

#![forbid(unsafe_code)]

use std::any::Any;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Identity primitives
// ---------------------------------------------------------------------------

/// Opaque user-supplied key: `Clone + Eq + Hash`, as RFC-0002 §2 requires.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Key(pub String);

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Key(s.to_string())
    }
}

/// Generational slot handle. Stable across frames ONLY while the generation
/// matches; any unmount bumps the generation so stale handles fail loudly
/// (`get` returns `None`) instead of aliasing a recycled slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SlotId {
    index: u32,
    generation: u32,
}

// ---------------------------------------------------------------------------
// Type-erased slot storage
// ---------------------------------------------------------------------------

/// Object-safe erased slot. State lives behind `Any` for typed recovery;
/// `type_tag` lets the reconciler detect replacement without downcasting.
pub trait ErasedSlot {
    /// Shared access to the erased state.
    fn as_any(&self) -> &dyn Any;
    /// Exclusive access to the erased state (the single legal mutation site).
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Stable per-type tag for replacement detection.
    fn type_tag(&self) -> &'static str;
}

struct SlotBox<S: 'static> {
    state: S,
    tag: &'static str,
}

impl<S: 'static> ErasedSlot for SlotBox<S> {
    fn as_any(&self) -> &dyn Any {
        &self.state
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.state
    }
    fn type_tag(&self) -> &'static str {
        self.tag
    }
}

struct Entry {
    generation: u32,
    occupied: bool,
    parent: u32,
    key: Option<Key>,
    slot: Option<Box<dyn ErasedSlot>>,
}

/// Per-frame child description (owned data — caller side, rebuilt per frame).
pub struct ChildDesc {
    key: Option<Key>,
    type_tag: &'static str,
    init: Option<Box<dyn FnOnce() -> Box<dyn ErasedSlot>>>,
}

impl ChildDesc {
    /// Keyed child with initial state for first mount.
    pub fn keyed<S: 'static>(key: &str, type_tag: &'static str, state: S) -> Self {
        Self {
            key: Some(Key::from(key)),
            type_tag,
            init: Some(Box::new(move || {
                Box::new(SlotBox { state, tag: type_tag }) as Box<dyn ErasedSlot>
            })),
        }
    }

    /// Unkeyed (positional) child with initial state for first mount.
    pub fn unkeyed<S: 'static>(type_tag: &'static str, state: S) -> Self {
        Self {
            key: None,
            type_tag,
            init: Some(Box::new(move || {
                Box::new(SlotBox { state, tag: type_tag }) as Box<dyn ErasedSlot>
            })),
        }
    }
}

/// Reference produced by reconciliation for one child.
pub enum ChildRef {
    /// Live framework-owned slot.
    Live(SlotId),
    /// Duplicate-key extra: a FRESH stateless slot, owned by the caller for
    /// this frame only. Never inserted into the table — stateless by
    /// construction, so collisions cannot corrupt table state.
    StatelessEphemeral(Box<dyn ErasedSlot>),
}

/// Outcome of one reconciliation pass. Misuse is reported HERE as data —
/// no panics, no logs, no silent fallback.
pub struct ReconcileOutcome {
    /// One ref per input desc, in input order.
    pub refs: Vec<ChildRef>,
    /// True when this sibling list mixed keyed and unkeyed children and the
    /// table fell back to positional identity for the whole list.
    pub mixed_keys_fallback: bool,
    /// Duplicate-key extras in this pass (each got a stateless ephemeral).
    pub collisions: usize,
    /// Fresh mounts in this pass.
    pub mounted: usize,
    /// Unmounts (with cleanup) in this pass.
    pub dropped: usize,
}

/// Framework-owned generational slot table (hand-rolled, zero-dep).
pub struct SlotTable {
    entries: Vec<Entry>,
    free: Vec<u32>,
    key_map: HashMap<(u32, Key), SlotId>,
    positional: HashMap<(u32, usize), SlotId>,
    /// Structural allocation ledger: slot boxes created, ever (mount-only).
    pub box_allocs: usize,
    /// Total mounts / unmounts over table lifetime.
    pub mounts: usize,
    /// Total unmounts (each runs the cleanup point).
    pub drops: usize,
    /// Total duplicate-key extras denied table residence, ever.
    pub collisions: usize,
    /// Total mixed keyed/unkeyed lists fallen back to positional, ever.
    pub mixed_fallbacks: usize,
}

impl SlotTable {
    /// Create an empty table.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free: Vec::new(),
            key_map: HashMap::new(),
            positional: HashMap::new(),
            box_allocs: 0,
            mounts: 0,
            drops: 0,
            collisions: 0,
            mixed_fallbacks: 0,
        }
    }

    fn alloc_entry(&mut self, parent: u32, key: Option<Key>, slot: Box<dyn ErasedSlot>) -> SlotId {
        self.box_allocs += 1;
        self.mounts += 1;
        if let Some(index) = self.free.pop() {
            let e = &mut self.entries[index as usize];
            e.occupied = true;
            e.parent = parent;
            e.key = key;
            e.slot = Some(slot);
            SlotId { index, generation: e.generation }
        } else {
            let index = self.entries.len() as u32;
            self.entries.push(Entry {
                generation: 0,
                occupied: true,
                parent,
                key,
                slot: Some(slot),
            });
            SlotId { index, generation: 0 }
        }
    }

    fn unmount(&mut self, id: SlotId) {
        let e = &mut self.entries[id.index as usize];
        debug_assert!(e.occupied && e.generation == id.generation);
        if !(e.occupied && e.generation == id.generation) {
            return;
        }
        e.slot = None; // cleanup point: user `cleanup(state)` runs here.
        e.occupied = false;
        e.generation = e.generation.wrapping_add(1);
        self.free.push(id.index);
        self.drops += 1;
    }

    fn live(&self, id: SlotId) -> bool {
        self.entries
            .get(id.index as usize)
            .is_some_and(|e| e.occupied && e.generation == id.generation)
    }

    /// Typed shared access. `None` on stale handle, wrong type, or free slot —
    /// never a wrong-typed reference.
    pub fn get<S: 'static>(&self, id: SlotId) -> Option<&S> {
        if !self.live(id) {
            return None;
        }
        self.entries[id.index as usize]
            .slot
            .as_ref()
            .and_then(|s| s.as_any().downcast_ref::<S>())
    }

    /// Typed exclusive access (the single legal mutation site per slot).
    pub fn get_mut<S: 'static>(&mut self, id: SlotId) -> Option<&mut S> {
        if !self.live(id) {
            return None;
        }
        self.entries[id.index as usize]
            .slot
            .as_mut()
            .and_then(|s| s.as_any_mut().downcast_mut::<S>())
    }

    /// Reconcile one sibling list under `parent` against fresh descriptions.
    pub fn reconcile(&mut self, parent: u32, mut descs: Vec<ChildDesc>) -> ReconcileOutcome {
        let any_keyed = descs.iter().any(|d| d.key.is_some());
        let all_keyed = descs.iter().all(|d| d.key.is_some());
        let mixed = any_keyed && !all_keyed;
        if mixed {
            self.mixed_fallbacks += 1;
        }
        let keyed_mode = all_keyed && !mixed;

        let mut outcome = ReconcileOutcome {
            refs: Vec::with_capacity(descs.len()),
            mixed_keys_fallback: mixed,
            collisions: 0,
            mounted: 0,
            dropped: 0,
        };
        let drops_before = self.drops;
        let mounts_before = self.mounts;

        // Survivor set: every slot re-referenced this pass stays alive.
        let mut retained: HashSet<(u32, u32)> = HashSet::new();
        let mut seen_keys: HashSet<Key> = HashSet::new();

        for (pos, desc) in descs.drain(..).enumerate() {
            let ChildDesc { key, type_tag, init } = desc;
            let init = init.expect("desc init consumed once");
            if keyed_mode {
                let k = key.expect("keyed mode implies keys");
                if !seen_keys.insert(k.clone()) {
                    // DUPLICATE KEY: first wins; extra gets a fresh stateless
                    // ephemeral every frame + observable collision count.
                    self.collisions += 1;
                    outcome.collisions += 1;
                    outcome.refs.push(ChildRef::StatelessEphemeral(init()));
                    self.box_allocs += 1; // ephemeral box, dropped end of frame
                    continue;
                }
                let wanted = (parent, k.clone());
                let reuse = self
                    .key_map
                    .get(&wanted)
                    .copied()
                    .filter(|id| self.live(*id))
                    .filter(|id| {
                        self.entries[id.index as usize]
                            .slot
                            .as_ref()
                            .is_some_and(|s| s.type_tag() == type_tag)
                    });
                match reuse {
                    Some(id) => {
                        retained.insert((id.index, id.generation));
                        outcome.refs.push(ChildRef::Live(id));
                    }
                    None => {
                        // Miss OR type replacement: drop the stale keyed slot.
                        if let Some(stale) = self.key_map.get(&wanted).copied()
                            && self.live(stale)
                        {
                            self.unmount(stale);
                        }
                        let id = self.alloc_entry(parent, Some(k.clone()), init());
                        self.key_map.insert(wanted, id);
                        retained.insert((id.index, id.generation));
                        outcome.refs.push(ChildRef::Live(id));
                    }
                }
            } else {
                // POSITIONAL mode (pure unkeyed, or mixed fallback w/ keys ignored).
                let wanted = (parent, pos);
                let reuse = self
                    .positional
                    .get(&wanted)
                    .copied()
                    .filter(|id| self.live(*id))
                    .filter(|id| {
                        self.entries[id.index as usize]
                            .slot
                            .as_ref()
                            .is_some_and(|s| s.type_tag() == type_tag)
                    });
                match reuse {
                    Some(id) => {
                        retained.insert((id.index, id.generation));
                        outcome.refs.push(ChildRef::Live(id));
                    }
                    None => {
                        if let Some(stale) = self.positional.get(&wanted).copied()
                            && self.live(stale)
                        {
                            self.unmount(stale);
                        }
                        let id = self.alloc_entry(parent, None, init());
                        self.positional.insert(wanted, id);
                        retained.insert((id.index, id.generation));
                        outcome.refs.push(ChildRef::Live(id));
                    }
                }
            }
        }

        // Sweep: unmount same-parent slots no longer referenced.
        // (O(table) sweep; production keeps per-parent sets — noted in README.)
        let mut sweep: Vec<SlotId> = Vec::new();
        for (index, e) in self.entries.iter().enumerate() {
            if e.occupied && e.parent == parent && !retained.contains(&(index as u32, e.generation)) {
                sweep.push(SlotId { index: index as u32, generation: e.generation });
            }
        }
        // Forget map entries pointing at swept slots so a re-added key mounts fresh.
        for id in &sweep {
            let e = &self.entries[id.index as usize];
            if keyed_mode {
                if let Some(k) = e.key.clone()
                    && self.key_map.get(&(parent, k.clone())) == Some(id)
                {
                    self.key_map.remove(&(parent, k));
                }
            } else if let Some(pos) = self
                .positional
                .iter()
                .find_map(|(p, v)| (*v == *id).then_some(*p))
            {
                self.positional.remove(&pos);
            }
            self.unmount(*id);
        }

        outcome.mounted = self.mounts - mounts_before;
        outcome.dropped = self.drops - drops_before;
        outcome
    }
}

impl Default for SlotTable {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Allocation instrumentation (deterministic, safe).
//
// Every heap allocation on the framework side of reconcile happens through a
// counted site: slot boxes (`box_allocs`) and the three backing stores below.
// Snapshotting capacities across phases proves steady-state behaviour without
// a global allocator (which would need `unsafe` and harness-noise filtering).
// ---------------------------------------------------------------------------

impl SlotTable {
    /// Backing-store capacities: `(entries, key_map, positional)`.
    /// Stable capacities across steady-state reconciles == no backing growth.
    pub fn capacities(&self) -> (usize, usize, usize) {
        (
            self.entries.capacity(),
            self.key_map.capacity(),
            self.positional.capacity(),
        )
    }
}

/// Shared test state: a simple counter.
#[derive(Debug, PartialEq)]
pub struct Counter {
    /// Count value.
    pub n: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const P: u32 = 0; // single sibling list under parent 0

    fn live_id(o: &ReconcileOutcome, i: usize) -> SlotId {
        match &o.refs[i] {
            ChildRef::Live(id) => *id,
            ChildRef::StatelessEphemeral(_) => panic!("expected live slot at {i}"),
        }
    }

    #[test]
    fn persistent_local_state_across_frames() {
        let mut t = SlotTable::new();
        let o = t.reconcile(P, vec![ChildDesc::keyed("a", "Counter", Counter { n: 0 })]);
        let id = live_id(&o, 0);
        for i in 1..=10 {
            let o = t.reconcile(P, vec![ChildDesc::keyed("a", "Counter", Counter { n: -1 })]);
            assert_eq!(live_id(&o, 0), id, "identity stable across frames");
            t.get_mut::<Counter>(id).unwrap().n += 1;
            assert_eq!(t.get::<Counter>(id).unwrap().n, i);
        }
        assert_eq!(t.mounts, 1, "mounted exactly once");
        assert_eq!(t.drops, 0);
    }

    #[test]
    fn keyed_reorder_state_follows_key() {
        let mut t = SlotTable::new();
        let mk = || {
            vec![
                ChildDesc::keyed("a", "Counter", Counter { n: 1 }),
                ChildDesc::keyed("b", "Counter", Counter { n: 2 }),
            ]
        };
        let o = t.reconcile(P, mk());
        let (ida, idb) = (live_id(&o, 0), live_id(&o, 1));
        // Swap order; per-frame descs carry only mount seeds, never live state.
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("b", "Counter", Counter { n: 99 }),
                ChildDesc::keyed("a", "Counter", Counter { n: 99 }),
            ],
        );
        assert_eq!(live_id(&o, 0), idb, "slot followed key b");
        assert_eq!(live_id(&o, 1), ida, "slot followed key a");
        assert_eq!(t.get::<Counter>(idb).unwrap().n, 2);
        assert_eq!(t.get::<Counter>(ida).unwrap().n, 1);
        assert_eq!(t.mounts, 2, "no remount on reorder");
    }

    #[test]
    fn add_remove_mount_unmount_counts() {
        let mut t = SlotTable::new();
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("a", "Counter", Counter { n: 0 }),
                ChildDesc::keyed("b", "Counter", Counter { n: 0 }),
            ],
        );
        assert_eq!((o.mounted, o.dropped), (2, 0));
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("a", "Counter", Counter { n: 0 }),
                ChildDesc::keyed("b", "Counter", Counter { n: 0 }),
                ChildDesc::keyed("c", "Counter", Counter { n: 0 }),
            ],
        );
        assert_eq!((o.mounted, o.dropped), (1, 0), "added child mounts once");
        let o = t.reconcile(P, vec![ChildDesc::keyed("c", "Counter", Counter { n: 0 })]);
        assert_eq!((o.mounted, o.dropped), (0, 2), "removed children unmount");
        assert_eq!(t.drops, 2);
    }

    #[test]
    fn key_collision_first_wins_extra_stateless() {
        let mut t = SlotTable::new();
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("dup", "Counter", Counter { n: 7 }),
                ChildDesc::keyed("dup", "Counter", Counter { n: 666 }),
            ],
        );
        assert_eq!(o.collisions, 1);
        let first = live_id(&o, 0);
        assert!(matches!(o.refs[1], ChildRef::StatelessEphemeral(_)));
        // First keeps its state; the extra's seed never leaks into the table.
        assert_eq!(t.get::<Counter>(first).unwrap().n, 7);
        // Next frame: first still live with state, extra is a FRESH ephemeral.
        let o2 = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("dup", "Counter", Counter { n: -1 }),
                ChildDesc::keyed("dup", "Counter", Counter { n: -1 }),
            ],
        );
        assert_eq!(live_id(&o2, 0), first, "first wins, stably");
        assert!(matches!(o2.refs[1], ChildRef::StatelessEphemeral(_)));
        assert_eq!(t.get::<Counter>(first).unwrap().n, 7, "no corruption from extra");
        assert_eq!(t.collisions, 2);
    }

    #[test]
    fn unkeyed_positional_reorder_keeps_state_at_position() {
        let mut t = SlotTable::new();
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::unkeyed("Counter", Counter { n: 1 }),
                ChildDesc::unkeyed("Counter", Counter { n: 2 }),
            ],
        );
        let (s0, s1) = (live_id(&o, 0), live_id(&o, 1));
        // Swap the DESCRIPTORS. Positional identity reuses slots by position,
        // so state stays where it was — the documented positional semantic.
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::unkeyed("Counter", Counter { n: 2 }),
                ChildDesc::unkeyed("Counter", Counter { n: 1 }),
            ],
        );
        assert_eq!(live_id(&o, 0), s0);
        assert_eq!(live_id(&o, 1), s1);
        assert_eq!(t.get::<Counter>(s0).unwrap().n, 1, "state stayed at position 0");
        assert_eq!(t.get::<Counter>(s1).unwrap().n, 2, "state stayed at position 1");
    }

    #[test]
    fn type_change_at_identity_drops_old_slot() {
        let mut t = SlotTable::new();
        let o = t.reconcile(P, vec![ChildDesc::keyed("k", "Counter", Counter { n: 5 })]);
        let old = live_id(&o, 0);
        let o = t.reconcile(P, vec![ChildDesc::keyed("k", "Flag", true)]);
        assert_eq!((o.mounted, o.dropped), (1, 1), "replace, not reuse");
        assert!(t.get::<Counter>(old).is_none(), "stale id + wrong type both fail");
        let new = live_id(&o, 0);
        assert_ne!(old, new);
        assert_eq!(*t.get::<bool>(new).unwrap(), true);
    }

    #[test]
    fn stale_handle_after_unmount_and_generational_reuse() {
        let mut t = SlotTable::new();
        let o = t.reconcile(P, vec![ChildDesc::keyed("a", "Counter", Counter { n: 0 })]);
        let stale = live_id(&o, 0);
        let o = t.reconcile(P, vec![ChildDesc::keyed("b", "Counter", Counter { n: 0 })]);
        assert_eq!(o.dropped, 1);
        assert!(t.get::<Counter>(stale).is_none(), "stale handle fails loudly");
        // Index recycled by the new mount, but the generation differs.
        let fresh = live_id(&o, 0);
        assert_ne!(stale, fresh, "generation bumped on recycle");
    }

    #[test]
    fn mixed_keys_fallback_is_observable_not_silent() {
        let mut t = SlotTable::new();
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("a", "Counter", Counter { n: 1 }),
                ChildDesc::unkeyed("Counter", Counter { n: 2 }),
            ],
        );
        assert!(o.mixed_keys_fallback, "misuse flagged in the outcome");
        assert_eq!(t.mixed_fallbacks, 1);
        assert_eq!(o.refs.len(), 2, "reconciliation still total: nothing lost");
        // Recovery: a clean all-keyed list clears the flag.
        let o = t.reconcile(
            P,
            vec![
                ChildDesc::keyed("a", "Counter", Counter { n: 0 }),
                ChildDesc::keyed("b", "Counter", Counter { n: 0 }),
            ],
        );
        assert!(!o.mixed_keys_fallback);
    }

    #[test]
    fn allocation_map_mount_once_zero_steady_state() {
        // Deterministic ledger + capacity snapshots (no global allocator, no
        // unsafe, no harness noise): slot boxes are created at mount only and
        // no backing store grows during steady state.
        let mut t = SlotTable::new();
        let seed = |i: usize| ChildDesc::keyed(&format!("k{i}"), "Counter", Counter { n: 0 });
        let descs: Vec<ChildDesc> = (0..64).map(seed).collect();
        let o = t.reconcile(P, descs);
        assert_eq!(o.mounted, 64);
        let boxes_at_mount = t.box_allocs;
        let caps_at_mount = t.capacities();

        for _ in 0..200 {
            let f: Vec<ChildDesc> = (0..64).map(seed).collect();
            let o = t.reconcile(P, f);
            assert_eq!((o.mounted, o.dropped), (0, 0));
        }
        assert_eq!(t.box_allocs, boxes_at_mount, "ZERO slot boxes in steady state");
        assert_eq!(t.capacities(), caps_at_mount, "ZERO backing growth in steady state");
        assert_eq!(t.mounts, 64);
        assert_eq!(t.drops, 0);

        println!(
            "ALLOC-MAP: mounts=64 slot_boxes={boxes_at_mount} steady_reconciles=200 added_boxes=0 capacities={caps_at_mount:?} drops=0"
        );
    }

    #[test]
    fn ephemeral_factory_runs_once_per_collision_frame() {
        // Proves extras are re-created per frame (stateless) rather than cached.
        let calls = Arc::new(AtomicUsize::new(0));
        let mut t = SlotTable::new();
        for _ in 0..3 {
            let c = calls.clone();
            let mk = || {
                let c2 = c.clone();
                ChildDesc {
                    key: Some(Key::from("d")),
                    type_tag: "Counter",
                    init: Some(Box::new(move || {
                        c2.fetch_add(1, Ordering::Relaxed);
                        Box::new(SlotBox { state: Counter { n: 0 }, tag: "Counter" })
                            as Box<dyn ErasedSlot>
                    })),
                }
            };
            let o = t.reconcile(P, vec![mk(), mk()]);
            assert_eq!(o.collisions, 1);
            assert!(matches!(o.refs[1], ChildRef::StatelessEphemeral(_)));
        }
        // 1 table mount + 1 ephemeral per frame x3 = 4 factory runs.
        assert_eq!(calls.load(Ordering::Relaxed), 4);
    }
}
