//! Micro-benchmarks for diffing and presenting frames (std only, facade only).
//!
//! Run with `cargo bench -p jilva --bench render_diff`. Numbers are
//! machine-relative and meant for ordering scenarios and catching
//! regressions, not for publishing as framework performance claims
//! (`docs/README.md#performance-and-release-evidence`).

use std::hint::black_box;
use std::time::{Duration, Instant};

use jilva::{
    Buffer, Damage, DiffOptions, Rect, Renderer, ResolvedAttributes, ResolvedColor,
    TestDoubleBackend, View, diff,
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
    const SAMPLES: usize = 5;
    // `cargo test --all-targets` also runs this harness. Keep that a quick
    // correctness smoke check; performance samples require `cargo bench`.
    let iterations = if cfg!(debug_assertions) {
        iterations.min(3)
    } else {
        iterations
    };
    let mut commands = 0;
    for _ in 0..(iterations / 10).max(1) {
        commands = black_box(work());
    }
    let mut samples = [Duration::ZERO; SAMPLES];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..iterations {
            commands = black_box(work());
        }
        *sample = start.elapsed() / iterations;
    }
    samples.sort_unstable();
    let per_iteration = samples[SAMPLES / 2];
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
    println!("median of 5 batches; warmup = 10% of iterations; no terminal I/O");
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

    let mut renderer = Renderer::new(COLS, ROWS);
    renderer.back_buffer_mut().clone_from(&before);
    black_box(renderer.present(&Damage::Full, &caps));
    measure("present unchanged, Damage::Full", 1_000, || {
        renderer.present(&Damage::Full, &caps).len()
    });
    let mut tick = false;
    measure("present one-cell edit, Damage::Full", 1_000, || {
        tick = !tick;
        renderer
            .back_buffer_mut()
            .set_string(100, 30, if tick { "A" } else { "Z" }, styled(5));
        renderer.present(&Damage::Full, &caps).len()
    });
    measure("present one-cell edit, damage region", 1_000, || {
        tick = !tick;
        renderer
            .back_buffer_mut()
            .set_string(100, 30, if tick { "A" } else { "Z" }, styled(5));
        renderer.present(&damage, &caps).len()
    });
    measure("present full-screen restyle", 100, || {
        tick = !tick;
        renderer
            .back_buffer_mut()
            .clone_from(if tick { &before } else { &churned });
        renderer.present(&Damage::Full, &caps).len()
    });

    let mut heap = Buffer::new(COLS, ROWS);
    for y in 0..ROWS {
        heap.set_string(0, y, &"👨‍👩‍👧‍👦".repeat(usize::from(COLS / 2)), styled(5));
    }
    renderer.back_buffer_mut().clone_from(&heap);
    black_box(renderer.present(&Damage::Full, &caps));
    measure("present unchanged long emoji clusters", 400, || {
        renderer.present(&Damage::Full, &caps).len()
    });

    let view = View::text(&"The quick brown fox 日本語 jumps over — ".repeat(6));
    black_box(renderer.render(&view, &caps));
    measure(
        "render unchanged text view (paint + present)",
        1_000,
        || renderer.render(&view, &caps).len(),
    );
}
