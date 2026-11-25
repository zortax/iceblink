//! Marks with discs, boxes, a polyline and placed shapes.

use std::sync::Arc;

use iceblink::kurbo::BezPath;
use iceblink::{
    ClipRef, MarkBox, MarkFlags, MarkPayload, MarkPlacement, Marks, OrderEntry, PaintKind, SpaceRef,
};

use crate::common::props;
use crate::common::stories::{background, key};
use crate::common::story::Story;

/// A payload of discs, boxes, a polyline and placed shapes, drawn plain and as a union, then
/// moved.
pub(crate) const MARKS: Story = Story {
    name: "marks",
    steps: &[
        background,
        |tx, world| {
            let purple = world.solid(tx, [0.4, 0.1, 0.5, 1.0]);
            let mut star = BezPath::new();
            star.move_to((0.0, -5.0));
            star.line_to((4.0, 4.0));
            star.line_to((-5.0, -2.0));
            star.line_to((5.0, -2.0));
            star.line_to((-4.0, 4.0));
            star.close_path();

            let payload = Arc::new(MarkPayload {
                discs: vec![[20.0, 20.0, 6.0, 0.0], [40.0, 20.0, 6.0, 3.0]],
                boxes: vec![MarkBox {
                    rect: [60.0, 12.0, 80.0, 28.0],
                    radii: [3.0; 8],
                    shape: [2.0, 0.0, 0.0, 0.0],
                }],
                vertices: vec![
                    [f32::NAN, 0.0],
                    [10.0, 60.0],
                    [40.0, 40.0],
                    [70.0, 60.0],
                    [f32::NAN, 0.0],
                ],
                shapes: vec![Arc::new(star)],
                placements: vec![
                    MarkPlacement {
                        x: 20.5,
                        y: 90.25,
                        shape: 0,
                        _pad: 0,
                    },
                    MarkPlacement {
                        x: 50.0,
                        y: 90.0,
                        shape: 0,
                        _pad: 0,
                    },
                ],
            });

            let mut chunk = tx.record(key(2), props(world.space));
            let paint = chunk.paint_ref(PaintKind::SOLID, purple);
            let index = chunk.payload(payload);
            let marks = |flags: MarkFlags| Marks {
                bounds: [0.0, 0.0, 128.0, 128.0],
                paint,
                paint_origin: [0.0; 2],
                origin: [0.0; 2],
                axes: [1.0, 0.0, 0.0, 1.0],
                half_width: 1.5,
                discs: 2,
                boxes: 1,
                vertices: 5,
                placements: 2,
                first: 0,
                payload: index,
                clip: ClipRef::INHERIT,
                space: SpaceRef::INHERIT,
                flags,
            };
            chunk.marks(marks(MarkFlags::default()));
            let mut united = marks(MarkFlags::UNION);
            united.origin = [0.0, 20.0];
            united.placements = 0;
            chunk.marks(united);
            world.append(tx, OrderEntry::Chunk(key(2)));
        },
        |tx, _| tx.set_offset(key(2), [3.0, 2.0]),
    ],
};
