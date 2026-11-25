//! The cost of drawing a 1920 by 1080 frame on the CPU: whole with and without a blurred overlay
//! and a frosted panel, and with the damage of a typical edit.

mod scene;

use criterion::{Criterion, criterion_group, criterion_main};
use iceblink::RenderTarget;
use iceblink::Renderer;
use iceblink_cpu::CpuRenderer;

use crate::scene::{SIZE, frame};

fn renderer(threads: usize) -> CpuRenderer {
    let mut renderer = CpuRenderer::offscreen(SIZE).with_threads(threads);
    renderer.configure(RenderTarget {
        size: SIZE,
        scale: 1.0,
        opaque: true,
    });
    renderer
}

fn frames(criterion: &mut Criterion) {
    let whole = frame(None, true);
    let cards = frame(None, false);
    // A text field's line being typed into.
    let edit = frame(Some([400, 300, 420, 40]), true);
    let mut group = criterion.benchmark_group("frames");
    group.sample_size(20);
    for threads in [1, 16] {
        let mut drawing = renderer(threads);
        drawing.draw(&whole);
        group.bench_function(format!("whole_{threads}"), |bench| {
            bench.iter(|| drawing.draw(&whole));
        });
        group.bench_function(format!("cards_{threads}"), |bench| {
            bench.iter(|| drawing.draw(&cards));
        });
        group.bench_function(format!("edit_{threads}"), |bench| {
            bench.iter(|| drawing.draw(&edit));
        });
    }
    group.finish();
}

criterion_group!(benches, frames);
criterion_main!(benches);
