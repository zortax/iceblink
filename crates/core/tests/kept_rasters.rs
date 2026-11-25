//! A vector pass damaged in part frame after frame by a moving dot, drawn by a renderer that keeps
//! its raster, against the same pictures redrawn whole without kept rasters.

mod common;

use std::sync::Arc;

use iceblink::Renderer;
use iceblink::kurbo::{Affine, BezPath};
use iceblink::{
    ChunkKey, ClipRef, FillRule, Gradient, GradientShape, GradientStop, Interpolation, OrderEntry,
    Paint, PaintKind, PathFill, PathItem, SpaceRef,
};
use iceblink_testing::gpu::device_lock;

use common::twin::Twin;

/// Returns the value the kept slice's renderer reports for `name`.
fn stat(twin: &Twin, name: &str) -> u64 {
    let mut found = 0;
    twin.kept.renderer.extra_stats(&mut |key, value| {
        if key == name {
            found = value;
        }
    });
    found
}

#[test]
fn a_kept_raster_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let dot = ChunkKey::new(3, 0);
    twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        world.boxes(
            tx,
            ChunkKey::new(1, 0),
            white,
            &[[0.0, 0.0, 128.0, 128.0]],
            true,
        );

        let ramp = tx.create_paint(Paint::Gradient(Gradient {
            shape: GradientShape::Linear {
                start: [0.0, 0.0],
                end: [60.0, 60.0],
            },
            stops: Arc::from(vec![
                GradientStop::new(0.0, iceblink::color::Color::BLACK),
                GradientStop::new(1.0, iceblink::color::Color::WHITE),
            ]),
            interpolation: Interpolation::default(),
            repeat: false,
        }));
        let mut triangle = BezPath::new();
        triangle.move_to((0.0, 0.0));
        triangle.line_to((60.0, 0.0));
        triangle.line_to((0.0, 60.0));
        triangle.close_path();
        let key = ChunkKey::new(2, 0);
        let mut chunk = tx.record(key, common::props(world.space));
        let fill = chunk.paint_ref(PaintKind::GRADIENT, ramp);
        chunk.path(PathItem {
            path: Arc::new(triangle),
            placement: Affine::translate((20.0, 20.0)),
            brush: Affine::IDENTITY,
            fill: Some(PathFill {
                paint: fill,
                rule: FillRule::NON_ZERO,
            }),
            stroke: None,
            clips: Arc::from(Vec::new()),
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
        });
        world.append(tx, OrderEntry::Chunk(key));

        let red = world.solid(tx, [1.0, 0.0, 0.0, 1.0]);
        world.boxes(tx, dot, red, &[[30.0, 30.0, 6.0, 6.0]], true);
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");
    assert_eq!(
        stat(&twin, "keep.rasters"),
        0,
        "a pass drawn once is not kept"
    );

    // Each move uncovers path pixels where the dot was, which only the pass's raster paints, and
    // one uncovers the triangle's long edge.
    for (step, at) in [[40.0, 24.0], [47.0, 47.0], [24.0, 40.0], [30.0, 30.0]]
        .into_iter()
        .enumerate()
    {
        twin.commit(&mut (), |tx, world, _| {
            let red = world.solid(tx, [1.0, 0.0, 0.0, 1.0]);
            world.boxes(tx, dot, red, &[[at[0], at[1], 6.0, 6.0]], false);
        });
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "step {step} differs: {found:?}");
        assert_eq!(stat(&twin, "keep.rasters"), 1, "a pass drawn twice is kept");
    }
    assert!(
        stat(&twin, "keep.hits") >= 3,
        "the pass is composited from its kept raster: {} hits",
        stat(&twin, "keep.hits")
    );
}
