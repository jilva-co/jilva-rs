//! Experiment 01 — `dyn`-compatible render seam for heterogeneous children.
//!
//! Hypothesis H1: an object-safe render-only trait supports heterogeneous
//! `Vec<Box<dyn Render>>` children (static, nested, conditional, dynamic,
//! reordered, added, removed). The full `Component` trait with associated
//! `State`/`Message` types and receiver-less `init`/`update` is NOT
//! `dyn`-compatible (E0038/E0191 — see `src/negative_not_dyn_compatible.rs`,
//! which is intentionally excluded from the build).
//!
//! Addresses RFC-0002 §8 F2.2 and ARCHITECTURE_FEASIBILITY R2-C1.

#![forbid(unsafe_code)]

/// Immutable per-frame context threaded into every render call.
/// Widths are supplied by the layout authority; nothing is measured here.
#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    /// Available width in cells.
    pub width: u16,
    /// Whether the focus path passes through the rendering subtree.
    pub focused: bool,
}

/// Object-safe render seam.
///
/// Object safety holds because: only `&self` receivers, no generic methods,
/// no associated types or constants, and the return type is owned (`String`),
/// so no `&self`-borrowed lifetime escapes into the `dyn` type.
pub trait Render {
    /// Describe this component as owned data for the given context.
    fn render(&self, ctx: &Ctx) -> String;
    /// Stable per-type tag; the identity layer uses it to detect
    /// type replacement at an existing identity (RFC-0002 §2).
    fn type_tag(&self) -> &'static str;
}

/// Positive proof of object safety: `&dyn Render` / `Box<dyn Render>` exist.
#[allow(dead_code)]
fn assert_dyn_compatible(_: &dyn Render) {}

/// Positive proof that `Send` widgets can opt into a `Send` seam without
/// forcing it on every widget (see exp04 for the policy argument).
#[allow(dead_code)]
fn assert_send_seam<T: Render + Send + 'static>(t: T) -> Box<dyn Render + Send> {
    Box::new(t)
}

// ---------------------------------------------------------------------------
// Fixture widgets mirroring the RFC-0002 sketch:
//   frame.column([Header, TodoList, StatusBar])
// ---------------------------------------------------------------------------

/// Static text header.
pub struct Header {
    /// Title text.
    pub title: String,
}

impl Render for Header {
    fn render(&self, ctx: &Ctx) -> String {
        let mark = if ctx.focused { ">" } else { " " };
        format!("{mark} H[{} w={}]", self.title, ctx.width)
    }
    fn type_tag(&self) -> &'static str {
        "Header"
    }
}

/// Dynamic list of rows.
pub struct TodoList {
    /// Row labels.
    pub items: Vec<String>,
}

impl Render for TodoList {
    fn render(&self, _ctx: &Ctx) -> String {
        self.items
            .iter()
            .map(|i| format!("  - {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn type_tag(&self) -> &'static str {
        "TodoList"
    }
}

/// One-line status bar.
pub struct StatusBar {
    /// Status text.
    pub text: String,
}

impl Render for StatusBar {
    fn render(&self, _ctx: &Ctx) -> String {
        format!("S({})", self.text)
    }
    fn type_tag(&self) -> &'static str {
        "StatusBar"
    }
}

/// A container holding pre-erased heterogeneous children (nesting case).
pub struct Panel {
    /// Child widgets, already type-erased at construction.
    pub kids: Vec<Box<dyn Render>>,
}

impl Render for Panel {
    fn render(&self, ctx: &Ctx) -> String {
        column(&self.kids, ctx)
    }
    fn type_tag(&self) -> &'static str {
        "Panel"
    }
}

/// Vertical composition over a heterogeneous child list.
///
/// This is the RFC-0002 `column([Header, TodoList, StatusBar])` sketch made
/// compilable: the array literal has no common element type, so callers name
/// the erased element type explicitly (`Vec<Box<dyn Render>>`).
pub fn column(children: &[Box<dyn Render>], ctx: &Ctx) -> String {
    children
        .iter()
        .map(|c| c.render(ctx))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Positive proof for §A.3: `dyn` + fully-specified associated types compiles
// but forces ALL children to share one State/Message pair (homogeneous-only).
// ---------------------------------------------------------------------------

/// Component-flavoured trait whose non-object-safe methods are quarantined
/// behind `where Self: Sized`, leaving an object-safe remainder once the
/// associated types are bound to concrete types.
pub trait SharedState {
    /// State type shared by every child behind one `dyn` handle.
    type State;
    /// Message type shared by every child behind one `dyn` handle.
    type Message;
    /// Constructor: receiver-less, hence `Sized`-quarantined.
    fn init() -> Self::State
    where
        Self: Sized;
    /// Update: receiver-less, hence `Sized`-quarantined.
    fn update(slot: &mut Self::State, msg: Self::Message) -> &'static str
    where
        Self: Sized;
    /// Object-safe remainder: render through a `&self` receiver plus a shared
    /// borrow of the slot. (`&self` is what keeps this method object-safe;
    /// an associated function without a receiver would fail E0038 exactly
    /// like `init`/`update`.)
    fn render(&self, slot: &Self::State) -> String;
}

/// Counter widget A.
pub struct CounterA;
/// Counter widget B (distinct type, same state/message vocabulary).
pub struct CounterB;

impl SharedState for CounterA {
    type State = i64;
    type Message = i64;
    fn init() -> i64 {
        0
    }
    fn update(slot: &mut i64, msg: i64) -> &'static str {
        *slot += msg;
        "repaint"
    }
    fn render(&self, slot: &i64) -> String {
        format!("A={slot}")
    }
}

impl SharedState for CounterB {
    type State = i64;
    type Message = i64;
    fn init() -> i64 {
        100
    }
    fn update(slot: &mut i64, msg: i64) -> &'static str {
        *slot -= msg;
        "repaint"
    }
    fn render(&self, slot: &i64) -> String {
        format!("B={slot}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            width: 80,
            focused: false,
        }
    }

    fn sketch_children() -> Vec<Box<dyn Render>> {
        vec![
            Box::new(Header {
                title: "todos".into(),
            }),
            Box::new(TodoList {
                items: vec!["a".into(), "b".into()],
            }),
            Box::new(StatusBar { text: "ok".into() }),
        ]
    }

    #[test]
    fn heterogeneous_column_renders_all_types() {
        let kids = sketch_children();
        let out = column(&kids, &ctx());
        assert!(out.contains("H[todos w=80]"), "{out}");
        assert!(out.contains("- a") && out.contains("- b"), "{out}");
        assert!(out.contains("S(ok)"), "{out}");
    }

    #[test]
    fn static_homogeneous_list() {
        let kids: Vec<Box<dyn Render>> = vec![
            Box::new(Header { title: "x".into() }),
            Box::new(Header { title: "y".into() }),
        ];
        let out = column(&kids, &ctx());
        assert!(out.contains("H[x") && out.contains("H[y"), "{out}");
    }

    #[test]
    fn nested_columns() {
        let inner = Panel {
            kids: vec![
                Box::new(TodoList { items: vec!["n".into()] }),
                Box::new(StatusBar { text: "in".into() }),
            ],
        };
        let outer: Vec<Box<dyn Render>> = vec![
            Box::new(Header { title: "o".into() }),
            Box::new(inner),
        ];
        let out = column(&outer, &ctx());
        assert!(out.contains("- n") && out.contains("S(in)"), "{out}");
    }

    #[test]
    fn conditional_children() {
        for flag in [false, true] {
            let mut kids: Vec<Box<dyn Render>> = vec![Box::new(Header {
                title: "h".into(),
            })];
            if flag {
                kids.push(Box::new(StatusBar { text: "opt".into() }));
            }
            let out = column(&kids, &ctx());
            assert_eq!(out.contains("S(opt)"), flag, "{out}");
        }
    }

    #[test]
    fn dynamic_add_and_remove_across_frames() {
        let mut kids = sketch_children();
        assert_eq!(kids.len(), 3);
        kids.push(Box::new(Header { title: "late".into() }));
        let out = column(&kids, &ctx());
        assert!(out.contains("H[late"), "{out}");
        kids.remove(1); // drop the TodoList mid-life
        let out = column(&kids, &ctx());
        assert!(!out.contains("- a") && out.contains("S(ok)"), "{out}");
    }

    #[test]
    fn reordered_children_render_in_new_order() {
        let mut kids = sketch_children();
        let tail = kids.pop().expect("status bar");
        kids.insert(0, tail);
        let out = column(&kids, &ctx());
        let s = out.find("S(ok)").expect("status first");
        let h = out.find("H[todos").expect("header later");
        assert!(s < h, "{out}");
    }

    #[test]
    fn render_borrows_nothing_across_frames() {
        // Owned descriptions: keep frame N alive while rendering frame N+1.
        let kids = sketch_children();
        let frame_n = column(&kids, &ctx());
        let frame_n1 = column(&kids, &Ctx { width: 40, focused: true });
        assert_ne!(frame_n, frame_n1);
        assert!(frame_n.contains("w=80") && frame_n1.contains("w=40"), "{frame_n} / {frame_n1}");
    }

    #[test]
    fn sized_quarantine_dyn_forces_homogeneous_state() {
        // Two DISTINCT widget types share one `dyn` handle ONLY because they
        // agreed on State=i64, Message=i64. A widget with State=String could
        // never join this vec: dyn+associated-types is homogeneous-only.
        let kids: Vec<Box<dyn SharedState<State = i64, Message = i64>>> =
            vec![Box::new(CounterA), Box::new(CounterB)];
        let mut slots = vec![CounterA::init(), CounterB::init()];
        for (k, s) in kids.iter().zip(slots.iter_mut()) {
            // `update` is Sized-quarantined: unreachable through `dyn`.
            // Only the object-safe remainder is callable here; drive update
            // through the concrete types to show the split.
            let _ = k.render(s);
        }
        CounterA::update(&mut slots[0], 5);
        CounterB::update(&mut slots[1], 5);
        assert_eq!(CounterA.render(&slots[0]), "A=5");
        assert_eq!(CounterB.render(&slots[1]), "B=95");
    }
}
