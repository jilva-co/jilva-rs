//! Micro-benchmarks for the rendering engine's diff (std only, facade only).
//!
//! Run with `cargo bench -p jilva --bench render_diff`. Numbers are
//! machine-relative and meant for ordering scenarios and catching
//! regressions, not for publishing as framework performance claims
//! (`docs/RFC/0004-rendering-engine.md` §9).

use std::hint::black_box;
use std::time::{Duration, Instant};

use jilva::{
    Buffer, Damage, DiffOptions, Rect, ResolvedAttributes, ResolvedColor, TestDoubleBackend, diff,
};

const COLS: u16 = 200;
const ROWS: u16 = 60;

fn styled(shade: u8) -> ResolvedAttributes {
    ResolvedAttributes::new(
        ResolvedColor::Rgb(shade, 128, 255 - shade),
        ResolvedColor::Default,
    )
}

/// A screen of mixed narrow, wide, and styled text.
fn screen(seed: u8) -> Buffer {
    let mut buffer = Buffer::new(COLS, ROWS);
    for y in 0..ROWS {
        let attrs = styled(seed.wrapping_add(y as u8));
        buffer.set_string(
            0,
            y,
            &"The quick brown fox 日本語 jumps over — ".repeat(6),
            attrs,
        );
    }
    buffer
}

fn measure(label: &str, iterations: u32, mut work: impl FnMut() -> usize) {
    let mut commands = 0;
    for _ in 0..(iterations / 10).max(1) {
        commands = black_box(work());
    }
    let start = Instant::now();
    for _ in 0..iterations {
        commands = black_box(work());
    }
    let per_iteration: Duration = start.elapsed() / iterations;
    println!(
        "{label:<46} {:>10.1} µs/frame   {commands:>5} commands",
        per_iteration.as_secs_f64() * 1e6
    );
}

fn main() {
    let caps = TestDoubleBackend::full_profile();
    let options = DiffOptions::default();
    let before = screen(0);
    let churned = screen(97); // every row restyled: worst case
    let mut one_cell = before.clone();
    one_cell.set_string(100, 30, "Z", styled(5));
    let damage = Damage::from_rects([Rect::new(100, 30, 1, 1)]);

    println!(
        "diff on a {COLS}x{ROWS} screen ({} cells)",
        u32::from(COLS) * u32::from(ROWS)
    );
    measure("unchanged, Damage::Full", 400, || {
        diff(&before, &before, &Damage::Full, &caps, &options).len()
    });
    measure("one cell changed, Damage::Full", 400, || {
        diff(&before, &one_cell, &Damage::Full, &caps, &options).len()
    });
    measure("one cell changed, one-cell damage region", 400, || {
        diff(&before, &one_cell, &damage, &caps, &options).len()
    });
    measure("full-screen churn (every cell restyled)", 100, || {
        diff(&before, &churned, &Damage::Full, &caps, &options).len()
    });
}
