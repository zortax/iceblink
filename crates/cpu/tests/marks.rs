//! Marks drawn prim by prim, by both renderers, which must agree within a tolerance.

mod common;

use std::sync::Arc;

use iceblink::{
    ClipRef, FRAME_NO_CLIP, FrameMarkPayload, FrameMarks, MarkBox, MarkFlags, MarkPayload, Marks,
    PaintRef, SpaceRef,
};
use iceblink_testing::golden::Tolerance;

use common::build::{FrameBuilder, filled};
use common::gpu::{assert_close, gpu};
use common::on_cpu;

const CLOSE: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

fn marks(
    paint: PaintRef,
    axes: [f32; 4],
    flags: MarkFlags,
    counts: [u32; 3],
    half_width: f32,
) -> FrameMarks {
    FrameMarks {
        marks: Marks {
            bounds: [0.0, 0.0, 96.0, 96.0],
            paint,
            paint_origin: [0.0; 2],
            origin: [4.0, 4.0],
            axes,
            half_width,
            discs: counts[0],
            boxes: counts[1],
            vertices: counts[2],
            placements: 0,
            first: 0,
            payload: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            flags,
        },
        page: 0,
    }
}

fn payload() -> FrameMarkPayload {
    let separator = [f32::NAN, f32::NAN];
    FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            discs: vec![
                [6.0, 6.0, 5.0, 0.0],
                [20.0, 6.0, 5.5, 2.5],
                [34.5, 6.25, 3.3, 0.0],
            ],
            boxes: vec![
                MarkBox {
                    rect: [2.0, 16.0, 20.0, 30.0],
                    radii: [3.0; 8],
                    shape: [2.0, 0.0, 0.0, 0.0],
                },
                MarkBox {
                    rect: [26.0, 16.0, 44.0, 30.0],
                    radii: [4.0; 8],
                    shape: [4.0, 1.5, 0.0, 0.0],
                },
            ],
            vertices: vec![
                separator,
                [2.0, 40.0],
                [14.0, 34.0],
                [26.0, 44.0],
                [38.0, 36.0],
                separator,
                [4.0, 50.0],
                [40.0, 52.0],
                separator,
            ],
            ..MarkPayload::default()
        }),
        cells: Arc::from(Vec::new()),
    }
}

#[test]
fn discs_boxes_and_polylines_drawn_one_by_one() {
    let mut builder = FrameBuilder::new();
    let white = builder.solid([1.0; 4]);
    builder.quad(filled([0.0, 0.0, 96.0, 96.0], white));
    let ink = builder.solid([0.1, 0.3, 0.5, 0.8]);
    let caps = MarkFlags::caps(MarkFlags::CAP_ROUND, MarkFlags::CAP_SQUARE);
    // In payload units, with the axes scaling every length.
    builder.marks(
        marks(ink, [1.5, 0.0, 0.0, 1.5], caps, [3, 2, 9], 1.5),
        payload(),
    );
    // In local units, under turned axes, with square discs.
    builder.marks(
        marks(
            ink,
            [0.8, 0.3, -0.3, 0.8],
            caps | MarkFlags::SCREEN | MarkFlags::SQUARE_DISCS,
            [3, 2, 9],
            1.0,
        ),
        payload(),
    );
    let Some(mut gpu) = gpu() else {
        return;
    };
    let frame = builder.finish();
    assert_close("marks", &gpu.drawn(&frame), &on_cpu(&frame), CLOSE);
}
