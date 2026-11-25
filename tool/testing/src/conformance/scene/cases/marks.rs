//! Marks: the placements of shapes too large for an atlas page.

use std::sync::Arc;

use iceblink::kurbo::{BezPath, Rect, Shape};
use iceblink::{
    ClipRef, MarkFlags, MarkPayload, MarkPlacement, Marks, Matrix4, OrderEntry, PaintRef, SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props};
use crate::conformance::scene::stage::Stage;

/// A placement of a shape too large for an atlas page is drawn as a path, also under a projective
/// matrix.
pub fn projected_placements(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let mut node = identity(Some(stage.space));
    node.local = Matrix4::from_rows([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.001, 0.0, 0.0, 1.0],
    ]);
    let tilted = tx.create_transform(node);
    let huge: BezPath = Rect::new(0.0, 0.0, 4096.0, 4096.0)
        .path_elements(0.1)
        .collect();
    let mut chunk = tx.record(key(1), props(tilted));
    let payload = chunk.payload(Arc::new(MarkPayload {
        shapes: vec![Arc::new(huge)],
        placements: vec![MarkPlacement {
            x: 10.0,
            y: 10.0,
            shape: 0,
            _pad: 0,
        }],
        ..MarkPayload::default()
    }));
    chunk.marks(Marks {
        bounds: [0.0, 0.0, 100.0, 100.0],
        paint: PaintRef::color([0, 0, 0, 255]),
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements: 1,
        first: 0,
        payload,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::default(),
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.redraw();

    assert!(
        !stage.frame.vector_plan.items.is_empty(),
        "the placement of a shape larger than every page is drawn as a path"
    );
}
