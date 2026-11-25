//! Vector pass keys for a renderer that keeps rasters.

mod common;

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath};
use iceblink::{
    Built, ChunkKey, ClipRef, FillRule, Gradient, GradientShape, GradientStop, Interpolation,
    OrderEntry, Paint, PaintId, PaintKind, PaintRef, PassKey, PathFill, PathItem,
    RenderCapabilities, SpaceRef,
};

use common::{FULL, Scene, props, quad, rect};

const KEEPING: RenderCapabilities = RenderCapabilities {
    kept_rasters: true,
    ..FULL
};

/// Returns a ramp from `from` to opaque white, which keeps a path on the vector route.
fn ramp(from: iceblink::color::Color) -> Paint {
    Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [20.0, 0.0],
        },
        stops: Arc::from(vec![
            GradientStop::new(0.0, from),
            GradientStop::new(1.0, iceblink::color::Color::WHITE),
        ]),
        interpolation: Interpolation::default(),
        repeat: false,
    })
}

/// Returns a scene with a triangle at (40, 40) filled with a gradient paint slot, with its first
/// frame built, and the slot's paint.
fn triangle(caps: RenderCapabilities) -> (Scene, PaintId) {
    let mut scene = Scene::with_caps(caps);
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((20.0, 0.0));
    path.line_to((0.0, 20.0));
    path.close_path();

    let mut tx = scene.tx();
    let paint = tx.create_paint(ramp(iceblink::color::Color::BLACK));
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let fill = chunk.paint_ref(PaintKind::GRADIENT, paint);
    chunk.path(PathItem {
        path: Arc::new(path),
        placement: Affine::translate((40.0, 40.0)),
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
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    (scene, paint)
}

/// Returns the key of the first vector pass of the last frame.
fn key(scene: &Scene) -> PassKey {
    scene.frame.vector_plan.passes[0].key
}

#[test]
fn a_renderer_that_keeps_no_rasters_gets_no_key() {
    let (scene, _) = triangle(FULL);
    assert_eq!(key(&scene), PassKey::NONE);
}

#[test]
fn a_pass_damaged_in_part_keeps_its_whole_region_and_its_key() {
    let (mut scene, _) = triangle(KEEPING);
    let first = key(&scene);
    assert_ne!(first, PassKey::NONE);

    let mut tx = scene.tx();
    tx.record(ChunkKey::new(2, 0), props(scene.space))
        .quad(quad([44.0, 44.0, 2.0, 2.0], PaintRef::NONE));
    tx.splice(scene.root, 1, 0, &[OrderEntry::Chunk(ChunkKey::new(2, 0))]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);

    let pass = &scene.frame.vector_plan.passes[0];
    assert_eq!(pass.region, rect(32, 32, 32, 32));
    assert!(!pass.clamped);
    assert_eq!(pass.key, first);
}

#[test]
fn a_paint_written_again_changes_the_key() {
    let (mut scene, paint) = triangle(KEEPING);
    let first = key(&scene);

    let mut tx = scene.tx();
    tx.set_paint(paint, ramp(iceblink::color::Color::TRANSPARENT));
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);

    assert_ne!(key(&scene), first);
    assert_ne!(key(&scene), PassKey::NONE);
}
