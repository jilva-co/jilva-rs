//! The mounted component tree: state slots, identity, focus and dispatch
//! (`docs/README.md#runtime`).
//!
//! Slots live in a hand-rolled generational table. Each frame, expansion
//! walks the model's [`View`], matches every component node to a slot by its
//! identity path, and records the mounted components in paint order. A
//! failed frame changes nothing: slots created during it are cleaned up and
//! the previous identity map stays (pipeline rule 2).

use std::any::Any;
use std::collections::HashMap;

use crate::component::{ComponentNode, ErasedComponent, Path, Segment};
use crate::error::{Error, Result};
use crate::events::{Change, InputEvent};
use crate::lifecycle::{Context, FollowUps};
use crate::style::Theme;
use crate::view::View;

/// A generational handle into the slot table. A stale handle (its slot was
/// freed and reused) resolves to nothing, never to another component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlotId {
    index: usize,
    generation: u32,
}

/// One mounted component: its latest configuration and its state.
struct Slot {
    generation: u32,
    component: Box<dyn ErasedComponent>,
    state: Box<dyn Any>,
}

/// A component as mounted in the last committed frame.
struct Mounted {
    path: Path,
    slot: SlotId,
    focusable: bool,
}

/// Work recorded while one frame expands, applied only if it succeeds.
#[derive(Default)]
struct Pending {
    by_path: HashMap<Path, SlotId>,
    created: Vec<SlotId>,
    mounted: Vec<(Mounted, Box<dyn ErasedComponent>)>,
}

/// What dispatching one input through the focus path produced.
pub(crate) struct Dispatched {
    pub(crate) consumed: bool,
    pub(crate) change: Change,
    pub(crate) quit: bool,
    pub(crate) theme: Option<Theme>,
}

/// The framework-owned component tree.
#[derive(Default)]
pub(crate) struct Tree {
    slots: Vec<Option<Slot>>,
    generations: Vec<u32>,
    free: Vec<usize>,
    by_path: HashMap<Path, SlotId>,
    mounted: Vec<Mounted>,
    focus: Option<Path>,
}

impl Tree {
    /// Number of components mounted in the last committed frame.
    #[cfg(test)]
    pub(crate) fn mounted_count(&self) -> usize {
        self.mounted.len()
    }

    /// Expands every component in `view` into its own view, mounting and
    /// unmounting slots. All or nothing: on error no slot is created,
    /// replaced or dropped.
    pub(crate) fn expand(&mut self, view: View, cx: &Context) -> Result<View> {
        let mut pending = Pending::default();
        let mut path = Path::new();
        let expanded = view.expand(&mut path, &mut |path, node| {
            self.mount(&mut pending, path, node, cx)
        });
        match expanded {
            Ok(view) => {
                self.commit(pending);
                Ok(view)
            }
            Err(error) => {
                for id in pending.created {
                    if let Some(slot) = self.remove(id) {
                        slot.component.cleanup(slot.state);
                    }
                }
                Err(error)
            }
        }
    }

    /// Keeps focus on a mounted, focusable component: an invalid focus is
    /// cleared, and no focus moves to the first focusable component.
    /// Returns `true` when focus changed.
    pub(crate) fn settle_focus(&mut self) -> bool {
        let valid = self
            .focus
            .as_ref()
            .is_some_and(|focus| self.mounted.iter().any(|m| m.focusable && &m.path == focus));
        if valid {
            return false;
        }
        let first = self
            .mounted
            .iter()
            .find(|m| m.focusable)
            .map(|m| m.path.clone());
        let changed = self.focus != first;
        self.focus = first;
        changed
    }

    /// Moves focus to the next (or previous) focusable component in paint
    /// order, wrapping around. Returns `true` when focus changed.
    pub(crate) fn move_focus(&mut self, forward: bool) -> bool {
        let focusable: Vec<&Path> = self
            .mounted
            .iter()
            .filter(|m| m.focusable)
            .map(|m| &m.path)
            .collect();
        if focusable.is_empty() {
            return false;
        }
        let current = self
            .focus
            .as_ref()
            .and_then(|focus| focusable.iter().position(|path| *path == focus));
        let count = focusable.len();
        let next = match (current, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
        };
        let next = focusable[next].clone();
        let changed = self.focus.as_ref() != Some(&next);
        self.focus = Some(next);
        changed
    }

    /// Delivers `input` to the focused component, then to each ancestor
    /// component, until one consumes it.
    pub(crate) fn dispatch_focused(
        &mut self,
        input: &InputEvent,
        cx: &Context,
        budget: &mut FollowUps,
    ) -> Result<Dispatched> {
        let mut result = Dispatched {
            consumed: false,
            change: Change::None,
            quit: false,
            theme: None,
        };
        let Some(focus) = self.focus.clone() else {
            return Ok(result);
        };
        let mut chain: Vec<(SlotId, bool)> = self
            .mounted
            .iter()
            .filter(|m| focus.starts_with(&m.path))
            .map(|m| (m.slot, m.path == focus))
            .collect();
        // Leaf first: longer paths are deeper.
        chain.reverse();
        for (slot, is_focus) in chain {
            let delivered = self.deliver(slot, input, &cx.with_focused(is_focus), budget)?;
            result.change = result.change.max(delivered.change);
            result.quit |= delivered.quit;
            if let Some(theme) = delivered.theme {
                result.theme = Some(theme);
            }
            if delivered.outcome.is_consumed() {
                result.consumed = true;
                break;
            }
        }
        Ok(result)
    }

    /// Delivers `input` to every mounted component, parents before children
    /// (used for resizes). Consumption does not stop it.
    pub(crate) fn broadcast(
        &mut self,
        input: &InputEvent,
        cx: &Context,
        budget: &mut FollowUps,
    ) -> Result<Dispatched> {
        let mut result = Dispatched {
            consumed: false,
            change: Change::None,
            quit: false,
            theme: None,
        };
        let targets: Vec<(SlotId, bool)> = self
            .mounted
            .iter()
            .map(|m| (m.slot, self.focus.as_ref() == Some(&m.path)))
            .collect();
        for (slot, is_focus) in targets {
            let delivered = self.deliver(slot, input, &cx.with_focused(is_focus), budget)?;
            result.change = result.change.max(delivered.change);
            result.quit |= delivered.quit;
            if let Some(theme) = delivered.theme {
                result.theme = Some(theme);
            }
        }
        Ok(result)
    }

    fn deliver(
        &mut self,
        id: SlotId,
        input: &InputEvent,
        cx: &Context,
        budget: &mut FollowUps,
    ) -> Result<crate::component::Delivered> {
        let slot = self
            .get_mut(id)
            .ok_or_else(|| Error::Lifecycle("mounted component has no slot".to_string()))?;
        slot.component
            .deliver(slot.state.as_mut(), input.clone(), cx, budget)
    }

    /// Expansion callback for one component node.
    fn mount(
        &mut self,
        pending: &mut Pending,
        path: &mut Path,
        node: ComponentNode,
        cx: &Context,
    ) -> Result<View> {
        let ComponentNode(component) = node;
        let stateless = path
            .iter()
            .any(|segment| matches!(segment, Segment::Stateless(_)));
        let focused = !stateless && self.focus.as_ref() == Some(&*path);
        let own_cx = cx.with_focused(focused);
        let view = if stateless {
            // A duplicate key: fresh state, described once, never mounted.
            let state = component.init()?;
            let view = component.view(state.as_ref(), &own_cx);
            component.cleanup(state);
            view?
        } else {
            let reusable = self
                .by_path
                .get(&*path)
                .copied()
                .filter(|id| {
                    self.get(*id).is_some_and(|slot| {
                        slot.component.component_type() == component.component_type()
                    })
                })
                .filter(|_| !pending.by_path.contains_key(&*path));
            let id = match reusable {
                Some(id) => id,
                None => {
                    let state = component.init()?;
                    let id = self.insert(Slot {
                        generation: 0,
                        component: component.clone_box(),
                        state,
                    });
                    pending.created.push(id);
                    id
                }
            };
            let slot = self
                .get(id)
                .ok_or_else(|| Error::Lifecycle("new slot vanished".to_string()))?;
            let view = component.view(slot.state.as_ref(), &own_cx)?;
            pending.by_path.insert(path.clone(), id);
            let mounted = Mounted {
                path: path.clone(),
                slot: id,
                focusable: component.focusable(),
            };
            pending.mounted.push((mounted, component));
            view
        };
        path.push(Segment::Inside);
        let expanded = view.expand(path, &mut |path, node| self.mount(pending, path, node, cx));
        path.pop();
        expanded
    }

    /// Applies a successful frame: drop slots no longer mounted, store the
    /// new configurations, and replace the identity map.
    fn commit(&mut self, pending: Pending) {
        let dropped: Vec<SlotId> = self
            .by_path
            .iter()
            .filter(|(path, id)| pending.by_path.get(*path) != Some(*id))
            .map(|(_, id)| *id)
            .filter(|id| !pending.by_path.values().any(|kept| kept == id))
            .collect();
        for id in dropped {
            if let Some(slot) = self.remove(id) {
                slot.component.cleanup(slot.state);
            }
        }
        let mut mounted = Vec::with_capacity(pending.mounted.len());
        for (entry, component) in pending.mounted {
            if let Some(slot) = self.get_mut(entry.slot) {
                slot.component = component;
            }
            mounted.push(entry);
        }
        self.by_path = pending.by_path;
        self.mounted = mounted;
    }

    fn insert(&mut self, mut slot: Slot) -> SlotId {
        let index = self.free.pop().unwrap_or_else(|| {
            self.slots.push(None);
            self.generations.push(0);
            self.slots.len() - 1
        });
        let generation = self.generations[index];
        slot.generation = generation;
        self.slots[index] = Some(slot);
        SlotId { index, generation }
    }

    fn remove(&mut self, id: SlotId) -> Option<Slot> {
        let slot = self.slots.get_mut(id.index)?;
        if slot.as_ref()?.generation != id.generation {
            return None;
        }
        let taken = slot.take();
        self.generations[id.index] = self.generations[id.index].wrapping_add(1);
        self.free.push(id.index);
        taken
    }

    fn get(&self, id: SlotId) -> Option<&Slot> {
        self.slots
            .get(id.index)?
            .as_ref()
            .filter(|slot| slot.generation == id.generation)
    }

    fn get_mut(&mut self, id: SlotId) -> Option<&mut Slot> {
        self.slots
            .get_mut(id.index)?
            .as_mut()
            .filter(|slot| slot.generation == id.generation)
    }
}

impl Drop for Tree {
    /// Unmount on teardown: every remaining slot gets its `cleanup`.
    fn drop(&mut self) {
        for slot in self.slots.drain(..).flatten() {
            slot.component.cleanup(slot.state);
        }
    }
}
