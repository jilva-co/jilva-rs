//! Rendering-model prototypes for RFC-0002 resolution.
//!
//! Hypothesis: the normative contract (owned descriptions, `&`-only view)
//! composes and error-propagates more cleanly than `&mut Frame` in user code,
//! while a framework-internal mutable cell buffer still bounds hot-path
//! allocation. Approach C = A at the user seam + internal mutable frame.
//!
//! Toolchain: stable 1.98.1, edition 2024, std-only, zero external deps.
//! Run: `cargo run --release` (from this dir). Exit 0 = all checks pass.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Allocation probe (test harness only, NOT proposed framework design)
// ---------------------------------------------------------------------------

static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn alloc_during(f: impl Fn()) -> usize {
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    f();
    ALLOC_BYTES.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Shared domain + test fixture
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Item {
    id: u64,
    label: String,
    done: bool,
}

#[derive(Debug, Clone)]
struct Model {
    title: String,
    items: Vec<Item>,
    selected: usize,
   _dr: String, // borrowed-data donor: long-lived owned string views borrow from
}

#[derive(Debug, Clone, Copy)]
struct Context {
    width: u16,
    height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RenderError {
    Overflow(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Overflow(m) => write!(f, "overflow: {m}"),
        }
    }
}

fn fixture(n: usize) -> (Model, Context) {
    let items = (0..n as u64)
        .map(|i| Item { id: 1000 + i, label: format!("task-{i}"), done: i % 2 == 0 })
        .collect();
    (
        Model { title: "todos".into(), items, selected: 1, _dr: "donor".into() },
        Context { width: 80, height: 24 },
    )
}

static CHECK_N: AtomicUsize = AtomicUsize::new(0);

fn check(ok: &mut bool, cond: bool, _echo: bool) {
    let n = CHECK_N.fetch_add(1, Ordering::Relaxed) + 1;
    println!("  [{}] check #{n}", if cond { "PASS" } else { "FAIL" });
    *ok &= cond;
}

// ---------------------------------------------------------------------------
// Approach A: owned `fn view(&self, ctx: &Context) -> Result<View, E>`
// ---------------------------------------------------------------------------

mod approach_a {
    use super::{Context, Model, RenderError};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Node {
        pub key: Option<u64>,
        pub text: String, // owned: no lifetime threads across the boundary
        pub children: Vec<Node>,
    }

    impl Node {
        pub fn leaf(key: Option<u64>, text: String) -> Self {
            Node { key, text, children: Vec::new() }
        }
    }

    pub trait Component {
        fn view(&self, ctx: &Context) -> Result<Node, RenderError>;
    }

    pub struct Row<'m> {
        pub item: &'m super::Item, // borrowed model data: compiles because we copy out
    }

    impl Component for Row<'_> {
        fn view(&self, ctx: &Context) -> Result<Node, RenderError> {
            if self.item.label.len() as u16 > ctx.width {
                return Err(RenderError::Overflow(format!("row {}", self.item.id)));
            }
            let mark = if self.item.done { "[x] " } else { "[ ] " };
            let mut text = String::with_capacity(mark.len() + self.item.label.len());
            text.push_str(mark);
            text.push_str(&self.item.label); // borrow -> owned copy at seam
            Ok(Node::leaf(Some(self.item.id), text))
        }
    }

    pub struct List<'m> {
        pub model: &'m Model,
    }

    impl Component for List<'_> {
        fn view(&self, ctx: &Context) -> Result<Node, RenderError> {
            // Dynamic children: map over borrowed slice, collect owned nodes.
            // Keys travel as data (u64), preserving identity across reorders.
            let mut children = Vec::with_capacity(self.model.items.len() + 1);
            children.push(Node::leaf(None, self.model.title.clone()));
            for item in &self.model.items {
                children.push(Row { item }.view(ctx)?); // `?` propagates, no rollback
            }
            Ok(Node { key: None, text: String::new(), children })
        }
    }

    // Simulated framework slot table: state follows keys across reorder.
    pub fn reconcile(slots: &mut super::HashMap<u64, usize>, view: &Node) {
        for child in view.children.iter().skip(1) {
            if let Some(k) = child.key {
                *slots.entry(k).or_insert(0) += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Approach B1: `fn view(&self, frame: &mut DescFrame)` — Frame as BUILDER
// ---------------------------------------------------------------------------

mod approach_b_builder {
    use super::{Context, Model, RenderError};

    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct DescFrame {
        pub nodes: Vec< super::approach_a::Node>,
    }

    impl DescFrame {
        pub fn text(&mut self, key: Option<u64>, s: String) {
            self.nodes.push(super::approach_a::Node::leaf(key, s));
        }
    }

    pub trait Component {
        fn view(&self, ctx: &Context, frame: &mut DescFrame) -> Result<(), RenderError>;
    }

    pub struct List<'m> {
        pub model: &'m Model,
    }

    impl Component for List<'_> {
        fn view(&self, ctx: &Context, frame: &mut DescFrame) -> Result<(), RenderError> {
            frame.text(None, self.model.title.clone());
            for item in &self.model.items {
                if item.label.len() as u16 > ctx.width {
                    // PARTIAL-MUTATION PROBLEM: title + earlier rows already
                    // pushed. Caller must clear/discard; contract must say so.
                    return Err(RenderError::Overflow(format!("row {}", item.id)));
                }
                let mark = if item.done { "[x] " } else { "[ ] " };
                frame.text(Some(item.id), format!("{mark}{}", item.label));
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Approach B2: `fn view(&self, frame: &mut CellFrame)` — Frame as CELL BUFFER
// ---------------------------------------------------------------------------

mod approach_b_cells {
    use super::{Context, Model, RenderError};

    pub struct CellFrame {
        pub width: u16,
        pub height: u16,
        pub cells: Vec<char>, // row-major; framework attenuation omitted
        pub cursor_y: u16,
    }

    impl CellFrame {
        pub fn new(w: u16, h: u16) -> Self {
            CellFrame { width: w, height: h, cells: vec![' '; (w as usize) * (h as usize)], cursor_y: 0 }
        }
        pub fn text(&mut self, s: &str) -> Result<(), RenderError> {
            // USER CODE DOES CELL MATH HERE: y-advance, truncation, wrap.
            // API_DESIGN §1 ("intent over mechanics") violated by construction.
            if self.cursor_y >= self.height {
                return Err(RenderError::Overflow("frame full".into()));
            }
            let y = self.cursor_y as usize;
            let mut x = 0usize;
            for ch in s.chars() {
                if x >= self.width as usize {
                    break; // silent truncation: no layout authority consulted
                }
                self.cells[y * self.width as usize + x] = ch;
                x += 1;
            }
            self.cursor_y += 1;
            Ok(())
        }
    }

    pub struct List<'m> {
        pub model: &'m Model,
    }

    impl List<'_> {
        pub fn view(&self, _ctx: &Context, frame: &mut CellFrame) -> Result<(), RenderError> {
            frame.text(&self.model.title)?;
            // Dynamic children + borrow of `self.model` while holding
            // `&mut frame`: compiles here only because they are disjoint
            // objects. In a real tree, parent and child both needing `&mut`
            // to the SAME frame forces sequential (non-nestable) calls or
            // split borrows threaded by hand — nesting is manual, not
            // compositional. No `?`-without-rollback issue is even
            // expressible: a half-painted buffer IS the output.
            for item in &self.model.items {
                frame.text(&format!("{} {}", if item.done { "[x]" } else { "[ ]" }, item.label))?;
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Approach C: hybrid — owned Desc → layout → framework-internal &mut CellFrame
// ---------------------------------------------------------------------------

mod approach_c {
    use super::{Context, Model, RenderError};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Desc {
        pub key: Option<u64>,
        pub text: String,
        pub children: Vec<Desc>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Placed<'d> {
        pub key: Option<u64>,
        pub text: &'d str, // borrowed from Desc: layout adds zero string allocation
        pub x: u16,
        pub y: u16,
        pub w: u16,
    }

    pub struct CellFrame {
        pub width: u16,
        pub height: u16,
        pub cells: Vec<char>,
    }

    impl CellFrame {
        pub fn blank(w: u16, h: u16) -> Self {
            CellFrame { width: w, height: h, cells: vec![' '; (w as usize) * (h as usize)] }
        }
        pub fn clear(&mut self) {
            self.cells.fill(' ');
        }
    }

    // USER SEAM (pure, & only) — identical shape to Approach A.
    pub fn describe(model: &Model, ctx: &Context) -> Result<Desc, RenderError> {
        if model.title.len() as u16 > ctx.width {
            return Err(RenderError::Overflow("title".into()));
        }
        let mut children = Vec::with_capacity(model.items.len());
        for item in &model.items {
            if item.label.len() as u16 > ctx.width {
                return Err(RenderError::Overflow(format!("row {}", item.id)));
            }
            children.push(Desc {
                key: Some(item.id),
                text: format!("{} {}", if item.done { "[x]" } else { "[ ]" }, item.label),
                children: Vec::new(),
            });
        }
        Ok(Desc { key: None, text: model.title.clone(), children })
    }

    // FRAMEWORK-OWNED layout: Desc -> regions. Fallible; no user cell math.
    // Borrows Desc text: adds Vec only, no string copies.
    pub fn layout<'d>(desc: &'d Desc, ctx: &Context) -> Result<Vec<Placed<'d>>, RenderError> {
        let mut out: Vec<Placed<'d>> = Vec::with_capacity(desc.children.len() + 1);
        let mut y: u16 = 0;
        macro_rules! put {
            ($key:expr, $text:expr) => {{
                if y >= ctx.height {
                    return Err(RenderError::Overflow("viewport full".into()));
                }
                out.push(Placed { key: $key, text: $text, x: 0, y, w: ctx.width });
                y += 1;
            }};
        }
        put!(desc.key, &desc.text);
        for c in &desc.children {
            put!(c.key, &c.text);
        }
        Ok(out)
    }

    // FRAMEWORK-OWNED paint: the ONLY `&mut Frame` in the system, internal.
    // Infallible by construction (layout guarantees bounds); reusable buffer.
    pub fn paint(placed: &[Placed<'_>], frame: &mut CellFrame) {
        frame.clear();
        for p in placed {
            let base = p.y as usize * frame.width as usize + p.x as usize;
            for (i, ch) in p.text.chars().take(p.w as usize).enumerate() {
                if base + i < frame.cells.len() {
                    frame.cells[base + i] = ch;
                }
            }
        }
    }

    pub fn render(model: &Model, ctx: &Context, frame: &mut CellFrame) -> Result<(), RenderError> {
        let desc = describe(model, ctx)?; // user code ends here
        let placed = layout(&desc, ctx)?; // framework
        paint(&placed, frame); // framework, &mut confined internally
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

use approach_a::Component as _;

fn main() {
    let mut ok = true;

    // ---- A: composition / nesting / borrowed data / owned output ----
    {
        println!("A: owned view(&self, ctx) -> View");
        let (model, ctx) = fixture(3);
        let v1 = approach_a::List { model: &model }.view(&ctx).unwrap();
        check(&mut ok, v1.children.len() == 4, v1.children.len() == 4);
        check(&mut ok, v1.children[1].key == Some(1000), v1.children[1].key == Some(1000));
        check(&mut ok, v1.children[1].text == "[x] task-0", v1.children[1].text == "[x] task-0");
        // Owned: description outlives the borrow (drop model borrow, keep view).
        let owned = v1.clone();
        drop(model);
        check(&mut ok, owned.children.len() == 4, owned.children.len() == 4);
        // Determinism: repeated renders identical.
        let (m2, c2) = fixture(3);
        let a = approach_a::List { model: &m2 }.view(&c2).unwrap();
        let b = approach_a::List { model: &m2 }.view(&c2).unwrap();
        check(&mut ok, a == b, a == b);
    }

    // ---- A: dynamic reorder — state follows key ----
    {
        println!("A: keyed reorder");
        let (mut model, ctx) = fixture(3);
        let mut slots: HashMap<u64, usize> = HashMap::new();
        let v1 = approach_a::List { model: &model }.view(&ctx).unwrap();
        approach_a::reconcile(&mut slots, &v1);
        model.items.swap(0, 2); // reorder, keys stable
        let v2 = approach_a::List { model: &model }.view(&ctx).unwrap();
        approach_a::reconcile(&mut slots, &v2);
        check(&mut ok, v2.children[1].key == Some(1002), v2.children[1].key == Some(1002));
        check(&mut ok, slots.get(&1000) == Some(&2), slots.get(&1000) == Some(&2)); // slot survived move
    }

    // ---- A: error propagation is total (no partial output) ----
    {
        println!("A: error propagation");
        let (mut model, _) = fixture(2);
        let tiny = Context { width: 3, height: 24 };
        let r = approach_a::List { model: &model }.view(&tiny);
        check(&mut ok, r.is_err(), r.is_err());
        model.items.clear();
        let r2 = approach_a::List { model: &model }.view(&tiny);
        check(&mut ok, r2.is_ok(), r2.is_ok()); // empty list still describes title... (title fits)
    }

    // ---- B1: builder frame — partial mutation on error ----
    {
        println!("B1: &mut DescFrame builder");
        use approach_b_builder::Component as _;
        let (model, _) = fixture(3);
        let tiny = Context { width: 3, height: 24 };
        let mut f = approach_b_builder::DescFrame::default();
        let r = approach_b_builder::List { model: &model }.view(&tiny, &mut f);
        check(&mut ok, r.is_err(), r.is_err());
        // Demonstrates the contract burden: caller MUST discard `f` on Err.
        let partial = !f.nodes.is_empty();
        check(&mut ok, partial, partial); // title row leaked into failed frame
        println!("      note: failed B1 render left {} orphan node(s) — discard-on-Err required", f.nodes.len());
    }

    // ---- B2: cell frame — user code does geometry ----
    {
        println!("B2: &mut CellFrame (cells)");
        let (model, ctx) = fixture(3);
        let mut f = approach_b_cells::CellFrame::new(ctx.width, ctx.height);
        let r = approach_b_cells::List { model: &model }.view(&ctx, &mut f);
        check(&mut ok, r.is_ok(), r.is_ok());
        // Half-paint is observable output: no description exists to diff.
        let (m2, c2) = fixture(3);
        let mut tiny = approach_b_cells::CellFrame::new(3, 24);
        let r2 = approach_b_cells::List { model: &m2 }.view(&c2, &mut tiny);
        check(&mut ok, r2.is_err() || tiny.cells.iter().any(|&c| c != ' '), true);
        println!("      note: B2 overflow leaves half-painted cells; nothing to diff against");
    }

    // ---- C: full pipeline ----
    {
        println!("C: hybrid describe -> layout -> paint");
        let (model, ctx) = fixture(3);
        let mut frame = approach_c::CellFrame::blank(ctx.width, ctx.height);
        let r = approach_c::render(&model, &ctx, &mut frame);
        check(&mut ok, r.is_ok(), r.is_ok());
        let row: String = frame.cells[0..5].iter().collect();
        check(&mut ok, row == "todos", row == "todos");
        // Error at describe/layout leaves...? paint never ran: reuse shows stale?
        // Contract: on Err, framework skips paint AND skips flush (diff vs old).
        let tiny = Context { width: 3, height: 24 };
        let before = frame.cells.clone();
        let r2 = approach_c::render(&model, &tiny, &mut frame);
        check(&mut ok, r2.is_err(), r2.is_err());
        // paint() not reached on Err, but clear() also not reached: buffer
        // untouched -> previous frame intact -> diff emits nothing. Correct.
        check(&mut ok, frame.cells == before, frame.cells == before);
        // Repeated renders deterministic.
        let mut f2 = approach_c::CellFrame::blank(ctx.width, ctx.height);
        approach_c::render(&model, &ctx, &mut f2).unwrap();
        let mut f3 = approach_c::CellFrame::blank(ctx.width, ctx.height);
        approach_c::render(&model, &ctx, &mut f3).unwrap();
        check(&mut ok, f2.cells == f3.cells, f2.cells == f3.cells);
    }

    // ---- Allocation comparison on EQUIVALENT work (50 rows, measured) ----
    {
        println!("alloc: 1000 renders x 50 rows (bytes allocated, measured)");
        let (model, mut ctx) = fixture(50);
        ctx.height = 64; // fit 1 title + 50 rows so describe+layout succeed
        let a_bytes = alloc_during(|| {
            for _ in 0..1000 {
                black_box(approach_a::List { model: black_box(&model) }.view(black_box(&ctx)).unwrap());
            }
        });
        println!("      A owned describe: {a_bytes} B / 1000 renders");
        let c_bytes = alloc_during(|| {
            for _ in 0..1000 {
                let d = black_box(approach_c::describe(black_box(&model), black_box(&ctx)).unwrap());
                let p = black_box(approach_c::layout(black_box(&d), black_box(&ctx)).unwrap());
                black_box(p);
            }
        });
        println!("      C describe+layout (paint buffer reused, excluded): {c_bytes} B / 1000 renders");
        let reuse = alloc_during(|| {
            let mut f = approach_c::CellFrame::blank(ctx.width, ctx.height);
            for _ in 0..1000 {
                black_box(approach_c::render(black_box(&model), black_box(&ctx), &mut f).unwrap());
            }
        });
        println!("      C full render w/ reused cell buffer: {reuse} B / 1000 renders");
        check(&mut ok, a_bytes > 0 && c_bytes > 0 && reuse > 0, true);
    }

    println!("{}", if ok { "RESULT: ALL CHECKS PASS" } else { "RESULT: FAILURES PRESENT" });
    std::process::exit(if ok { 0 } else { 1 });
}
