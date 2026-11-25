//! Frames drawn in bands on several threads, and frames drawn in part, against the same frames
//! drawn whole on one thread.

mod common;

use std::sync::Arc;

use iceblink::Renderer;
use iceblink::{
    Backdrop, BlendMode, ClipRef, Color, FRAME_NO_CLIP, Frame, FrameBackdrop, FrameFilter,
    FrameGroup, FrameShadow, Quad, RenderTarget, Shadow, SpaceRef,
};
use iceblink_cpu::CpuRenderer;

use common::build::{FrameBuilder, area, filled};

const SIZE: [u32; 2] = [520, 400];

fn renderer(threads: usize) -> CpuRenderer {
    let mut renderer = CpuRenderer::offscreen(SIZE).with_threads(threads);
    renderer.configure(RenderTarget {
        size: SIZE,
        scale: 1.0,
        opaque: true,
    });
    renderer
}

// A frame of stripes under blurred, shadowed and blended groups and frosted backdrops, one of
// them inside a group.
fn busy(damage: Option<(i32, i32, i32, i32)>) -> Frame {
    let mut builder = FrameBuilder::sized(SIZE);
    let white = builder.solid([1.0; 4]);
    let dark = builder.solid([0.1, 0.1, 0.2, 1.0]);
    let red = builder.solid([0.8, 0.1, 0.1, 0.8]);
    builder.quad(filled([0.0, 0.0, 520.0, 400.0], white));
    for stripe in 0..40 {
        let x = stripe as f32 * 13.0;
        builder.quad(Quad {
            radii: [3.0; 8],
            ..filled([x, 0.0, 6.0, 400.0], dark)
        });
    }
    let shadow = FrameFilter::DropShadow {
        offset: [4.0, 6.0],
        std_dev: 5.0,
        color: Color([0.0, 0.0, 0.0, 0.5]),
    };
    for (index, filters) in [vec![FrameFilter::Blur(6.0)], vec![shadow], vec![]]
        .into_iter()
        .enumerate()
    {
        let x = 20.0 + index as f32 * 160.0;
        builder.begin(FrameGroup {
            bounds: [x - 40.0, 10.0, 200.0, 240.0],
            source: [x - 40.0, 10.0, 200.0, 240.0],
            clip: FRAME_NO_CLIP,
            opacity: 0.9,
            blend: if index == 2 {
                BlendMode::MULTIPLY
            } else {
                BlendMode::NORMAL
            },
            filters: Arc::from(filters),
        });
        builder.quad(filled([x, 40.0, 110.0, 150.0], red));
        if index == 2 {
            builder.backdrop(frosted([x + 10.0, 60.0, 60.0, 60.0], 3.0));
        }
        builder.end();
    }
    builder.shadow(FrameShadow {
        bounds: [40.0, 250.0, 200.0, 120.0],
        shadow: Shadow {
            shape_bounds: [70.0, 280.0, 140.0, 60.0],
            radii: [12.0; 8],
            caster_bounds: [70.0, 280.0, 140.0, 60.0],
            caster_radii: [12.0; 8],
            shapes: [2.0; 4],
            color: Color([0.0, 0.0, 0.0, 0.7]),
            blur: 9.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            inset: 0,
        },
    });
    builder.backdrop(frosted([300.0, 260.0, 180.0, 110.0], 8.0));
    if let Some((x, y, width, height)) = damage {
        builder.damage(area(x, y, width, height));
    }
    builder.finish()
}

fn frosted(bounds: [f32; 4], deviation: f32) -> FrameBackdrop {
    let reach = 3.0 * deviation + 2.0;
    FrameBackdrop {
        backdrop: Backdrop {
            bounds,
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds,
        source: [
            bounds[0] - reach,
            bounds[1] - reach,
            bounds[2] + 2.0 * reach,
            bounds[3] + 2.0 * reach,
        ],
        filters: Arc::from(vec![FrameFilter::Blur(deviation)]),
    }
}

#[test]
fn bands_on_many_threads_draw_what_one_thread_draws() {
    let frame = busy(None);
    let mut one = renderer(1);
    let mut many = renderer(8);
    one.draw(&frame);
    many.draw(&frame);
    assert_eq!(one.composed(), many.composed());
}

#[test]
fn a_partial_redraw_draws_what_a_whole_one_draws() {
    let mut whole = renderer(8);
    whole.draw(&busy(None));
    let mut partial = renderer(8);
    partial.draw(&busy(None));
    // The same picture again, over a rectangle that cuts through the groups and a backdrop.
    partial.draw(&busy(Some((150, 50, 220, 300))));
    let difference = whole
        .composed()
        .data()
        .iter()
        .zip(partial.composed().data())
        .map(|(a, b)| a.abs_diff(*b))
        .max();
    assert!(
        difference <= Some(1),
        "a partial redraw differs by {difference:?}"
    );
}
