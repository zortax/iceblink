//! Paths: where they are drawn, and how a projective matrix maps them.

use std::sync::Arc;

use iceblink::kurbo::{BezPath, Rect, Shape};
use iceblink::{
    ChunkRecorder, ClipRef, FillRule, Matrix4, OrderEntry, PaintRef, PathClip, PathFill, PathItem,
    SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props};
use crate::conformance::scene::stage::{SIDE, Stage};

/// A path reaching past the output keeps its whole ink, a path wholly off the output is not
/// drawn, and under a projective matrix a path with a point whose W is zero or below is not
/// drawn.
pub fn path_placement(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let side = SIDE as f64;
    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    square(&mut chunk, side - 10.0, 20.0);
    square(&mut chunk, side + 100.0, 20.0);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.redraw();

    let drawn: Vec<u32> = stage
        .frame
        .vector_plan
        .items
        .iter()
        .map(|planned| planned.path)
        .collect();
    let reaching = stage
        .frame
        .paths
        .iter()
        .find(|path| path.ink.origin.x == (side - 10.0) as i32);
    assert!(
        reaching
            .is_some_and(|path| path.ink.origin.x + path.ink.size.width == (side + 10.0) as i32),
        "a path's ink is not cut to the output: {:?}",
        reaching.map(|path| path.ink)
    );
    let outside = stage
        .frame
        .paths
        .iter()
        .enumerate()
        .filter(|(_, path)| path.ink.origin.x >= side as i32)
        .any(|(index, _)| drawn.contains(&(index as u32)));
    assert!(!outside, "a path wholly off the output is in no pass");

    // The negated identity maps every point where the identity does, with W of -1.
    let mut node = identity(Some(stage.space));
    node.local = Matrix4::from_rows([
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.0, 0.0, -1.0],
    ]);
    let mut tx = stage.tx();
    let tilted = tx.create_transform(node);
    let mut chunk = tx.record(key(1), props(tilted));
    square(&mut chunk, 40.0, 20.0);
    stage.commit(tx);
    stage.redraw();
    let drawn = stage
        .frame
        .vector_plan
        .items
        .iter()
        .filter_map(|planned| stage.frame.paths.get(planned.path as usize))
        .any(|path| !path.path.elements().is_empty());
    assert!(
        !drawn,
        "a path whose points have a W of zero or below is not drawn"
    );
}

/// Records a square path of side `size` with its top left at `(x, 10)`.
fn square(chunk: &mut ChunkRecorder<'_>, x: f64, size: f64) {
    let outline: BezPath = Rect::new(x, 10.0, x + size, 10.0 + size)
        .path_elements(0.1)
        .collect();
    chunk.path(PathItem {
        path: Arc::new(outline),
        placement: Default::default(),
        brush: Default::default(),
        fill: Some(PathFill {
            paint: PaintRef::color([0, 0, 0, 255]),
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        // A path clip keeps the path a frame path.
        clips: Arc::from([PathClip {
            path: Arc::new(
                Rect::new(-1.0e3, -1.0e3, 1.0e3, 1.0e3)
                    .path_elements(0.1)
                    .collect(),
            ),
            rule: FillRule::NON_ZERO,
        }]),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
}
