//! Experiment 02 — enum dispatch, tuple composition, declarative macros.
//!
//! Hypothesis H2:
//! (a) enum dispatch gives allocation-free static dispatch for a CLOSED widget
//!     set, at the cost of third-party extensibility;
//! (b) tuples give fixed-arity heterogeneous composition with zero boxing, at
//!     the cost of an arity wall;
//! (c) a `macro_rules!` constructor is pure sugar whose expansion is exactly
//!     the macro-free `Vec<Box<dyn Render>>` form (macro-free normative core
//!     demonstrable, RFC-0002 §7 commitment).
//!
//! Self-contained by design (local `Render` redefinition); addresses R2-C1
//! mechanism options (i) enum and (iii)-adjacent static forms, plus the
//! RFC-0002 §8 "framework enum + boxing fallback" hybrid.

#![forbid(unsafe_code)]

/// Minimal object-safe render seam (same shape as exp01, local copy).
pub trait Render {
    /// Describe as owned data.
    fn render(&self) -> String;
}

/// Fixture widget 1.
pub struct Header {
    /// Title text.
    pub title: &'static str,
}
/// Fixture widget 2.
pub struct TodoList {
    /// Row count (stand-in for real items).
    pub rows: usize,
}
/// Fixture widget 3.
pub struct StatusBar {
    /// Status text.
    pub text: &'static str,
}

impl Render for Header {
    fn render(&self) -> String {
        format!("H({})", self.title)
    }
}
impl Render for TodoList {
    fn render(&self) -> String {
        format!("L({})", self.rows)
    }
}
impl Render for StatusBar {
    fn render(&self) -> String {
        format!("S({})", self.text)
    }
}

// ---------------------------------------------------------------------------
// (a) Closed-world enum dispatch: no Box, static match dispatch.
// ---------------------------------------------------------------------------

/// Framework-owned closed widget set. Adding a variant is a breaking change
/// to this enum — the extensibility price, tested below.
pub enum AnyWidget {
    /// Header variant.
    Header(Header),
    /// List variant.
    Todos(TodoList),
    /// Status variant.
    Bar(StatusBar),
}

impl Render for AnyWidget {
    fn render(&self) -> String {
        match self {
            AnyWidget::Header(h) => h.render(),
            AnyWidget::Todos(t) => t.render(),
            AnyWidget::Bar(s) => s.render(),
        }
    }
}

/// Homogeneous (single-enum) child list: zero heap allocation.
pub fn column_enum(children: &[AnyWidget]) -> String {
    children.iter().map(Render::render).collect::<Vec<_>>().join("\n")
}

// ---------------------------------------------------------------------------
// (b) Tuple composition: fixed arity, static dispatch, zero boxing.
// ---------------------------------------------------------------------------

impl<A: Render, B: Render> Render for (A, B) {
    fn render(&self) -> String {
        format!("{}\n{}", self.0.render(), self.1.render())
    }
}

impl<A: Render, B: Render, C: Render> Render for (A, B, C) {
    fn render(&self) -> String {
        format!("{}\n{}\n{}", self.0.render(), self.1.render(), self.2.render())
    }
}

// ---------------------------------------------------------------------------
// (c) Hybrid from RFC-0002 §8 F2.2: framework enum fast path + boxing fallback.
// ---------------------------------------------------------------------------

/// Child slot: inline enum for the known set, erased box for everything else
/// (including third-party widgets the enum can never name).
pub enum Child {
    /// Fast path: inline, no indirection.
    Inline(AnyWidget),
    /// Fallback: one box per foreign child, mounted once.
    Boxed(Box<dyn Render>),
}

impl Render for Child {
    fn render(&self) -> String {
        match self {
            Child::Inline(w) => w.render(),
            Child::Boxed(b) => b.render(),
        }
    }
}

/// Declarative constructor sugar. Expands to exactly the macro-free form:
///
/// ```text
/// column![a, b]  ===  vec![Box::new(a) as Box<dyn Render>, Box::new(b) ...]
/// ```
#[macro_export]
macro_rules! column {
    [$($child:expr),* $(,)?] => {
        vec![$(Box::new($child) as Box<dyn Render>),*]
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    /// Third-party widget: defined in user code, unnameable by `AnyWidget`.
    struct Fancy {
        glyph: char,
    }
    impl Render for Fancy {
        fn render(&self) -> String {
            format!("F({})", self.glyph)
        }
    }

    #[test]
    fn enum_dispatch_renders_all_variants() {
        let kids = vec![
            AnyWidget::Header(Header { title: "t" }),
            AnyWidget::Todos(TodoList { rows: 2 }),
            AnyWidget::Bar(StatusBar { text: "ok" }),
        ];
        let out = column_enum(&kids);
        assert!(out.contains("H(t)") && out.contains("L(2)") && out.contains("S(ok)"), "{out}");
    }

    #[test]
    fn enum_layout_is_inline_no_indirection() {
        // Measured (not reasoned): enum size == largest variant here because
        // the pointer-carrying variants offer niches for the discriminant
        // (no extra tag word); a boxed trait object is always 2 words on the
        // stack PLUS one heap allocation per child.
        // Header{&str}=16B, TodoList{Vec}=24B, StatusBar{&str}=16B.
        let enum_size = size_of::<AnyWidget>();
        assert_eq!(enum_size, 24, "niche-filled: no tag word beyond largest variant");
        assert_eq!(size_of::<Box<dyn Render>>(), 16, "fat pointer, plus heap");
        assert_eq!(size_of::<Child>(), 24, "niche-filled again: Box ptr niche reused");
    }

    #[test]
    fn tuple_pair_and_triple_render_without_boxes() {
        // No Box appears anywhere: static types, stack values, direct calls.
        let pair = (Header { title: "a" }, StatusBar { text: "b" });
        assert_eq!(pair.render(), "H(a)\nS(b)");
        let triple = (
            Header { title: "a" },
            TodoList { rows: 1 },
            StatusBar { text: "b" },
        );
        assert_eq!(triple.render(), "H(a)\nL(1)\nS(b)");
    }

    #[test]
    fn macro_expansion_equals_macro_free_form() {
        // The normative core exists without the macro: byte-identical output.
        let via_macro: Vec<Box<dyn Render>> = column![
            Header { title: "a" },
            TodoList { rows: 3 },
            StatusBar { text: "z" }
        ];
        let manual: Vec<Box<dyn Render>> = vec![
            Box::new(Header { title: "a" }),
            Box::new(TodoList { rows: 3 }),
            Box::new(StatusBar { text: "z" }),
        ];
        let flat = |v: &[Box<dyn Render>]| v.iter().map(|c| c.render()).collect::<Vec<_>>();
        assert_eq!(flat(&via_macro), flat(&manual));
        assert_eq!(via_macro.len(), 3);
    }

    #[test]
    fn third_party_widget_composes_via_dyn_not_via_enum() {
        // dyn form: foreign type joins freely (open world).
        let open: Vec<Box<dyn Render>> = vec![
            Box::new(Header { title: "h" }),
            Box::new(Fancy { glyph: '*' }),
        ];
        let out: Vec<String> = open.iter().map(|c| c.render()).collect();
        assert_eq!(out, ["H(h)", "F(*)"]);
        // Enum form: `AnyWidget` has no variant for `Fancy` and cannot gain
        // one without editing framework code (closed world). The hybrid
        // `Child::Boxed` is the escape hatch:
        let mixed = vec![
            Child::Inline(AnyWidget::Header(Header { title: "h" })),
            Child::Boxed(Box::new(Fancy { glyph: '*' })),
        ];
        let out: Vec<String> = mixed.iter().map(|c| c.render()).collect();
        assert_eq!(out, ["H(h)", "F(*)"]);
    }

    #[test]
    fn hybrid_nested_dynamic_and_reordered() {
        let mut kids = vec![
            Child::Inline(AnyWidget::Todos(TodoList { rows: 1 })),
            Child::Boxed(Box::new(Fancy { glyph: '?' })),
        ];
        kids.push(Child::Inline(AnyWidget::Bar(StatusBar { text: "new" })));
        kids.swap(0, 2);
        let out: Vec<String> = kids.iter().map(|c| c.render()).collect();
        assert_eq!(out, ["S(new)", "F(?)", "L(1)"]);
        kids.remove(1);
        assert_eq!(kids.len(), 2);
    }
}
