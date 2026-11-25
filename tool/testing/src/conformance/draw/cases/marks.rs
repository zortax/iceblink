//! Marks: discs and rings, boxes, and polylines with their caps.

use std::sync::Arc;

use iceblink::{
    ClipRef, FRAME_NO_CLIP, FrameMarkPayload, FrameMarks, MarkBox, MarkFlags, MarkPayload, Marks,
    PaintRef, SpaceRef,
};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden};
use crate::golden::Tolerance;

/// Antialiased edges: a pixel may differ by a few steps where its coverage is partial.
const EDGES: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 48,
};

fn marks(bounds: [f32; 4], paint: PaintRef, flags: MarkFlags) -> Marks {
    Marks {
        bounds,
        paint,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements: 0,
        first: 0,
        payload: 0,
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        flags,
    }
}

fn payload(payload: MarkPayload) -> FrameMarkPayload {
    FrameMarkPayload {
        payload: Arc::new(payload),
        cells: Arc::from(Vec::new()),
    }
}

/// Discs, rings and square discs, boxes with radii and borders, a polyline in each cap style,
/// and discs mapped by axes that stretch them.
pub fn mark_prims(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let blue = builder.solid(opaque(30, 60, 200));
    let red = builder.solid(opaque(200, 40, 40));

    let discs = MarkPayload {
        discs: vec![
            [12.0, 12.0, 8.0, 0.0],
            [34.0, 12.0, 8.0, 4.0],
            [56.0, 12.5, 6.5, 2.0],
        ],
        ..MarkPayload::default()
    };
    builder.marks(
        FrameMarks {
            marks: Marks {
                discs: 3,
                ..marks([0.0, 0.0, 128.0, 28.0], blue, MarkFlags::default())
            },
            page: 0,
        },
        payload(discs.clone()),
    );
    builder.marks(
        FrameMarks {
            marks: Marks {
                discs: 3,
                origin: [64.0, 0.0],
                ..marks([64.0, 0.0, 64.0, 28.0], red, MarkFlags::SQUARE_DISCS)
            },
            page: 0,
        },
        payload(discs.clone()),
    );

    let boxes = MarkPayload {
        boxes: vec![
            MarkBox {
                rect: [4.0, 32.0, 36.0, 56.0],
                radii: [6.0; 8],
                shape: [2.0, 0.0, 0.0, 0.0],
            },
            MarkBox {
                rect: [44.0, 32.0, 76.0, 56.0],
                radii: [8.0; 8],
                shape: [4.0, 3.0, 0.0, 0.0],
            },
            MarkBox {
                rect: [84.0, 32.0, 124.0, 56.0],
                radii: [0.0; 8],
                shape: [2.0, 0.0, 0.0, 0.0],
            },
        ],
        ..MarkPayload::default()
    };
    builder.marks(
        FrameMarks {
            marks: Marks {
                boxes: 3,
                ..marks([0.0, 28.0, 128.0, 32.0], blue, MarkFlags::default())
            },
            page: 0,
        },
        payload(boxes),
    );

    let caps = [
        MarkFlags::CAP_BUTT,
        MarkFlags::CAP_SQUARE,
        MarkFlags::CAP_ROUND,
    ];
    for (index, cap) in caps.into_iter().enumerate() {
        let top = 66.0 + index as f32 * 14.0;
        let line = MarkPayload {
            vertices: vec![
                [f32::NAN, 0.0],
                [10.0, top],
                [40.0, top + 8.0],
                [70.0, top],
                [110.0, top + 6.0],
                [f32::NAN, 0.0],
            ],
            ..MarkPayload::default()
        };
        builder.marks(
            FrameMarks {
                marks: Marks {
                    vertices: 6,
                    half_width: 2.5,
                    ..marks(
                        [0.0, top - 8.0, 128.0, 22.0],
                        red,
                        MarkFlags::caps(cap, cap),
                    )
                },
                page: 0,
            },
            payload(line),
        );
    }

    builder.marks(
        FrameMarks {
            marks: Marks {
                discs: 2,
                origin: [20.0, 112.0],
                axes: [2.0, 0.0, 0.0, 0.5],
                ..marks([0.0, 104.0, 128.0, 24.0], blue, MarkFlags::default())
            },
            page: 0,
        },
        payload(MarkPayload {
            discs: vec![[0.0, 0.0, 6.0, 0.0], [30.0, 0.0, 6.0, 3.0]],
            ..MarkPayload::default()
        }),
    );

    golden("mark_prims", &canvas.draw(builder.finish()), EDGES);
}
