//! The marks and the path of the scene.

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Circle, Shape, Stroke};
use iceblink::{
    ChunkRecorder, ClipRef, FillRule, MarkBox, MarkFlags, MarkPayload, MarkPlacement, Marks,
    PaintRef, PathClip, PathFill, PathItem, PathStroke, SpaceRef,
};

use super::Refs;

/// Records a marks item with every mark kind, then a filled, dashed and clipped path.
pub(super) fn record(recorder: &mut ChunkRecorder<'_>, refs: &Refs) {
    marks(recorder);
    path(recorder, refs);
}

/// Records union marks with a disc, a box, a polyline and two placed shapes.
fn marks(recorder: &mut ChunkRecorder<'_>) {
    let payload = recorder.payload(Arc::new(MarkPayload {
        discs: vec![[10.0, 10.0, 4.0, 1.0]],
        boxes: vec![MarkBox {
            rect: [20.0, 20.0, 30.0, 26.0],
            radii: [1.0; 8],
            shape: [2.0, 1.0, 0.0, 0.0],
        }],
        vertices: vec![
            [f32::NAN, 0.0],
            [0.0, 0.0],
            [20.0, 10.0],
            [40.0, 0.0],
            [f32::NAN, 0.0],
        ],
        shapes: vec![
            Arc::new(Circle::new((0.0, 0.0), 2.0).to_path(0.1)),
            Arc::new(Circle::new((0.0, 0.0), 400.0).to_path(0.1)),
        ],
        placements: vec![
            MarkPlacement {
                x: 30.3,
                y: 12.7,
                shape: 0,
                _pad: 0,
            },
            MarkPlacement {
                x: 40.0,
                y: 30.0,
                shape: 1,
                _pad: 0,
            },
        ],
    }));
    recorder.marks(Marks {
        bounds: [0.0, 0.0, 96.0, 64.0],
        paint: PaintRef::color([0, 0, 200, 255]),
        paint_origin: [1.0, 1.0],
        origin: [2.0, 3.0],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 1.5,
        discs: 1,
        boxes: 1,
        vertices: 5,
        placements: 2,
        first: 0,
        payload,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::UNION | MarkFlags::caps(MarkFlags::CAP_ROUND, MarkFlags::CAP_SQUARE),
    });
}

/// Records a curved path in the turned space, filled with the gradient and stroked with dashes.
fn path(recorder: &mut ChunkRecorder<'_>, refs: &Refs) {
    let mut outline = BezPath::new();
    outline.move_to((0.0, 0.0));
    outline.curve_to((30.0, 0.0), (30.0, 30.0), (0.0, 30.0));
    outline.close_path();
    recorder.path(PathItem {
        path: Arc::new(outline),
        placement: Affine::scale(0.8),
        brush: Affine::rotate(0.3),
        fill: Some(PathFill {
            paint: refs.gradient,
            rule: FillRule::EVEN_ODD,
        }),
        stroke: Some(PathStroke {
            paint: PaintRef::color([0, 0, 0, 255]),
            style: Stroke::new(2.0).with_dashes(0.0, [3.0, 2.0]),
        }),
        clips: Arc::new([PathClip {
            path: Arc::new(Circle::new((10.0, 10.0), 12.0).to_path(0.1)),
            rule: FillRule::NON_ZERO,
        }]),
        clip: refs.clip,
        space: refs.turned,
    });
}
