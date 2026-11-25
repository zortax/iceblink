//! Marks: placements drawn from atlas cells at the quarter-pixel phase of their anchor, unions
//! painted once through a bin, and payloads mapped by the item's axes.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, group, opaque, rect, straight};
use common::device::{SIDE, plain_renderer, present};
use iceblink::geom::{Point, Rect, Size};
use iceblink::kurbo::Shape as _;
use iceblink::{
    ClipRef, FRAME_NO_CLIP, FrameMarkPayload, FrameMarks, MAX_MARK_PAGES, MarkBin, MarkBox,
    MarkCell, MarkFlags, MarkPayload, MarkPlacement, MarkPlan, Marks, PageFormat, PageOp, PaintRef,
    SpaceRef,
};
use iceblink_wgpu::GroupPrecision;

#[test]
fn each_placement_reads_the_cell_of_its_phase() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    builder.page(PageOp::Create {
        page: 0,
        format: PageFormat::A8,
        size: [16, 16],
        levels: 1,
    });
    builder.page(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(16, 16)),
        bytes: Arc::from(vec![255u8; 256]),
        offset: 0,
        stride: 16,
    });
    // Phase zero draws four by four texels, and the half-pixel phase on x draws two by two.
    let mut cells = vec![
        MarkCell {
            rect: [0, 0, 4, 4],
            offset: [0, 0],
        };
        16
    ];
    cells[2] = MarkCell {
        rect: [8, 0, 2, 2],
        offset: [0, 0],
    };
    let payload = FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            placements: vec![
                MarkPlacement {
                    x: 10.0,
                    y: 10.0,
                    shape: 0,
                    _pad: 0,
                },
                MarkPlacement {
                    x: 40.5,
                    y: 10.0,
                    shape: 0,
                    _pad: 0,
                },
            ],
            shapes: vec![Arc::new(
                iceblink::kurbo::Rect::new(0.0, 0.0, 4.0, 4.0).to_path(0.1),
            )],
            ..MarkPayload::default()
        }),
        cells: Arc::from(cells),
    };
    let green = builder.solid(opaque(0, 255, 0));
    builder.marks(
        marks_item(
            rect(0.0, 0.0, 64.0, 32.0),
            green,
            [0, 0, 0, 2],
            MarkFlags::default(),
        ),
        payload,
    );

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(13, 13),
        [0, 255, 0, 255],
        "the whole-pixel phase's cell"
    );
    assert_eq!(
        pixels.rgba(41, 11),
        [0, 255, 0, 255],
        "the half-pixel phase's cell"
    );
    assert_eq!(pixels.rgba(43, 13), [0, 0, 0, 0], "which is smaller");
}

#[test]
fn a_union_of_overlapping_discs_is_painted_once_through_its_bin() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let half = builder.solid(iceblink::Color([0.0, 0.5, 0.0, 0.5]));
    let payload = FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            discs: vec![[40.0, 40.0, 20.0, 0.0], [60.0, 40.0, 20.0, 0.0]],
            ..MarkPayload::default()
        }),
        cells: Arc::from(Vec::new()),
    };
    builder.marks(
        marks_item(
            rect(0.0, 0.0, 128.0, 128.0),
            half,
            [2, 0, 0, 0],
            MarkFlags::UNION,
        ),
        payload,
    );
    let mut frame = builder.finish();
    frame.mark_plan = MarkPlan {
        bins: vec![MarkBin {
            item: 0,
            region: Rect::new(Point::new(0, 0), Size::new(128, 128)),
            at: [0, 0],
            page: 0,
        }],
        overflow: Vec::new(),
        pages: 2,
        extent: [128, 128],
        full_damage: true,
    };

    let pixels = present(&mut renderer, &frame);

    let alone = pixels.rgba(28, 40);
    let overlap = pixels.rgba(50, 40);
    assert!((126..=129).contains(&alone[3]), "{alone:?}");
    assert_eq!(alone, overlap, "the overlap is painted once");
}

/// Returns the anchor fractions the phase test places copies at: every 32nd of a pixel, and both
/// sides of each eighth where the phase changes.
fn fractions() -> Vec<f32> {
    let mut fractions: Vec<f32> = (0..32).map(|step| step as f32 / 32.0).collect();
    for boundary in [0.125f32, 0.375, 0.625, 0.875] {
        fractions.push(boundary - 1.0 / 256.0);
        fractions.push(boundary + 1.0 / 256.0);
    }
    fractions
}

// Cell `phase` is one texel of coverage `15 × (phase + 1)`, so the pixel a copy lights tells the
// phase it was drawn at.
#[test]
fn the_drawn_phase_lies_within_an_eighth_of_the_anchor() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    builder.page(PageOp::Create {
        page: 0,
        format: PageFormat::A8,
        size: [16, 16],
        levels: 1,
    });
    let mut texels = vec![0u8; 256];
    for (phase, texel) in texels.iter_mut().take(16).enumerate() {
        *texel = (15 * (phase + 1)) as u8;
    }
    builder.page(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(16, 16)),
        bytes: Arc::from(texels),
        offset: 0,
        stride: 16,
    });
    let cells: Vec<MarkCell> = (0..16)
        .map(|phase| MarkCell {
            rect: [phase, 0, 1, 1],
            offset: [0, 0],
        })
        .collect();
    let fractions = fractions();
    let base = |index: usize| (10 + 12 * (index % 9) as i32, 10 + 12 * (index / 9) as i32);
    let anchors: Vec<[f32; 2]> = (0..fractions.len())
        .map(|index| {
            let (x, y) = base(index);
            let fy = fractions[(index * 7 + 3) % fractions.len()];
            [x as f32 + fractions[index], y as f32 + fy]
        })
        .collect();
    let placements = anchors
        .iter()
        .map(|&[x, y]| MarkPlacement {
            x,
            y,
            shape: 0,
            _pad: 0,
        })
        .collect();
    let payload = FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            placements,
            shapes: vec![Arc::new(
                iceblink::kurbo::Rect::new(0.0, 0.0, 1.0, 1.0).to_path(0.1),
            )],
            ..MarkPayload::default()
        }),
        cells: Arc::from(cells),
    };
    let white = builder.solid(opaque(255, 255, 255));
    builder.marks(
        marks_item(
            rect(0.0, 0.0, 128.0, 128.0),
            white,
            [0, 0, 0, anchors.len() as u32],
            MarkFlags::default(),
        ),
        payload,
    );

    let pixels = present(&mut renderer, &builder.finish());

    for (index, anchor) in anchors.iter().enumerate() {
        let (x, y) = base(index);
        let lit: Vec<(i32, i32, u8)> = (-1..3)
            .flat_map(|dy| (-1..3).map(move |dx| (x + dx, y + dy)))
            .filter_map(|(px, py)| {
                let alpha = pixels.rgba(px, py)[3];
                (alpha > 0).then_some((px, py, alpha))
            })
            .collect();
        let [(px, py, alpha)] = lit.as_slice() else {
            panic!("the copy at {anchor:?} lit {lit:?}");
        };
        let phase = (f32::from(*alpha) / 15.0).round() as i32 - 1;
        assert!((0..16).contains(&phase), "level {alpha} at {anchor:?}");
        let drawn = [
            *px as f32 + (phase % 4) as f32 / 4.0,
            *py as f32 + (phase / 4) as f32 / 4.0,
        ];
        for axis in 0..2 {
            assert!(
                (drawn[axis] - anchor[axis]).abs() <= 0.125,
                "the copy at {anchor:?} is drawn at {drawn:?}"
            );
        }
    }
}

/// Returns a marks item over `bounds` painted with `paint`, drawing `counts` prims as
/// `[discs, boxes, vertices, placements]`.
fn marks_item(bounds: [f32; 4], paint: PaintRef, counts: [u32; 4], flags: MarkFlags) -> FrameMarks {
    FrameMarks {
        marks: Marks {
            bounds,
            paint,
            paint_origin: [0.0; 2],
            origin: [0.0; 2],
            axes: [1.0, 0.0, 0.0, 1.0],
            half_width: 0.0,
            discs: counts[0],
            boxes: counts[1],
            vertices: counts[2],
            placements: counts[3],
            first: 0,
            payload: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            flags,
        },
        page: 0,
    }
}

/// Uploads a solid cell of three texels square to page 0, and returns cells that refer to it at
/// every phase.
fn solid_cell(builder: &mut FrameBuilder) -> Arc<[MarkCell]> {
    builder.page(PageOp::Create {
        page: 0,
        format: PageFormat::A8,
        size: [16, 16],
        levels: 1,
    });
    builder.page(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(3, 3)),
        bytes: Arc::from(vec![255u8; 9]),
        offset: 0,
        stride: 3,
    });
    Arc::from(vec![
        MarkCell {
            rect: [0, 0, 3, 3],
            offset: [0, 0],
        };
        16
    ])
}

/// Returns a payload placing the one three-texel shape at each of `anchors`.
fn placed(anchors: &[[f32; 2]], cells: &Arc<[MarkCell]>) -> FrameMarkPayload {
    FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            placements: anchors
                .iter()
                .map(|&[x, y]| MarkPlacement {
                    x,
                    y,
                    shape: 0,
                    _pad: 0,
                })
                .collect(),
            shapes: vec![Arc::new(
                iceblink::kurbo::Rect::new(0.0, 0.0, 3.0, 3.0).to_path(0.1),
            )],
            ..MarkPayload::default()
        }),
        cells: Arc::clone(cells),
    }
}

/// Draws a white cell of three pixels square from pixel (10, 10) inside a group over the top left
/// 32 pixels, with the group held at half resolution when `half`, and returns the sum of alpha
/// around the cell.
fn grouped_ink(half: bool) -> u32 {
    let frame = || {
        let mut builder = FrameBuilder::new();
        let cells = solid_cell(&mut builder);
        let white = builder.solid(opaque(255, 255, 255));
        builder.begin_group(group(rect(0.0, 0.0, 32.0, 32.0), 1.0, &[]));
        builder.marks(
            marks_item(
                rect(8.0, 8.0, 8.0, 8.0),
                white,
                [0, 0, 0, 1],
                MarkFlags::default(),
            ),
            placed(&[[10.01, 10.01]], &cells),
        );
        builder.end_group();
        builder.finish()
    };
    // A quarter of the full-resolution target is one half-resolution target and no more.
    let budget = half.then(|| {
        let mut measure = plain_renderer();
        measure.set_group_precision(GroupPrecision::High);
        present(&mut measure, &frame());
        measure.groups().bytes() / 4
    });
    let mut renderer = plain_renderer();
    renderer.set_group_precision(GroupPrecision::High);
    if let Some(budget) = budget {
        renderer.set_group_budget(budget);
    }
    let pixels = present(&mut renderer, &frame());
    assert_eq!(renderer.groups().degraded(), u32::from(half));
    let mut ink = 0;
    for y in 4..20 {
        for x in 4..20 {
            ink += u32::from(pixels.rgba(x, y)[3]);
        }
    }
    ink
}

#[test]
fn a_half_resolution_placement_ending_on_an_odd_pixel_keeps_its_last_row_and_column() {
    // The cell covers pixels 10 to 12, and its last row and column are the first half of a
    // half-resolution texel.
    let full = grouped_ink(false);
    let half = grouped_ink(true);

    assert_eq!(full, 9 * 255);
    assert!(
        half.abs_diff(full) <= full / 10,
        "at half resolution the cell paints {half} of {full}"
    );
}

#[test]
fn union_items_past_the_last_bin_page_paint_alpha_once() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let cells = solid_cell(&mut builder);
    let translucent = builder.solid(straight(255, 255, 255, 128));
    // Each item covers the output, so each takes a whole bin page, and the last ones find none.
    let items: u32 = 10;
    for index in 0..items {
        // Two copies a pixel apart, which overlap in two columns.
        let x = 10.01 + 12.0 * index as f32;
        builder.marks(
            marks_item(
                rect(0.0, 0.0, SIDE as f32, SIDE as f32),
                translucent,
                [0, 0, 0, 2],
                MarkFlags::UNION,
            ),
            placed(&[[x, 10.01], [x + 1.0, 10.01]], &cells),
        );
    }
    let mut frame = builder.finish();
    let output = Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32));
    let bin = |item: u32| MarkBin {
        item,
        region: output,
        at: [0, 0],
        page: item.min(MAX_MARK_PAGES),
    };
    frame.mark_plan = MarkPlan {
        bins: (0..MAX_MARK_PAGES).map(bin).collect(),
        overflow: (MAX_MARK_PAGES..items).map(bin).collect(),
        pages: MAX_MARK_PAGES + 1,
        extent: [SIDE, SIDE],
        full_damage: true,
    };

    let pixels = present(&mut renderer, &frame);

    for index in 0..items as i32 {
        let x = 10 + 12 * index;
        let single = pixels.rgba(x, 11)[3];
        let overlap = pixels.rgba(x + 1, 11)[3];
        assert!((127..=129).contains(&single), "item {index}: {single}");
        assert!(
            overlap.abs_diff(single) <= 1,
            "item {index} paints its overlap at {overlap}, one copy at {single}"
        );
    }
}

/// Draws a union of two discs, a box with a border and a round-capped union polyline, with every
/// payload position and length multiplied by `k`, mapped to local space by `axes` and moved by
/// `[3, 4]`.
fn scaled_marks(k: f32, axes: [f32; 4]) -> iceblink_wgpu::Pixels {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(opaque(200, 60, 20));
    let mut push = |payload: MarkPayload, counts, flags, half_width: f32| {
        let mut item = marks_item(
            rect(0.0, 0.0, SIDE as f32, SIDE as f32),
            paint,
            counts,
            flags,
        );
        item.marks.origin = [3.0, 4.0];
        item.marks.axes = axes;
        item.marks.half_width = half_width * k;
        builder.marks(
            item,
            FrameMarkPayload {
                payload: Arc::new(payload),
                cells: Arc::from(Vec::new()),
            },
        );
    };
    push(
        MarkPayload {
            discs: vec![
                [6.1 * k, 6.3 * k, 3.7 * k, 0.0],
                [9.0 * k, 7.0 * k, 3.1 * k, 1.2 * k],
            ],
            ..MarkPayload::default()
        },
        [2, 0, 0, 0],
        MarkFlags::UNION,
        0.0,
    );
    push(
        MarkPayload {
            boxes: vec![MarkBox {
                rect: [20.2 * k, 3.1 * k, 34.7 * k, 11.9 * k],
                radii: [2.0 * k; 8],
                shape: [2.0, 1.3 * k, 0.0, 0.0],
            }],
            ..MarkPayload::default()
        },
        [0, 1, 0, 0],
        MarkFlags::default(),
        0.0,
    );
    let nan = f32::NAN;
    push(
        MarkPayload {
            vertices: vec![
                [nan, nan],
                [4.0 * k, 20.0 * k],
                [14.3 * k, 31.1 * k],
                [30.2 * k, 22.4 * k],
                [nan, nan],
            ],
            ..MarkPayload::default()
        },
        [0, 0, 5, 0],
        MarkFlags::UNION | MarkFlags::caps(MarkFlags::CAP_ROUND, MarkFlags::CAP_ROUND),
        1.1,
    );
    let mut frame = builder.finish();
    let output = Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32));
    // The two union items, at positions 0 and 2 of the lane, each take a page.
    frame.mark_plan = MarkPlan {
        bins: [(0, 0), (2, 1)]
            .into_iter()
            .map(|(item, page)| MarkBin {
                item,
                region: output,
                at: [0, 0],
                page,
            })
            .collect(),
        overflow: Vec::new(),
        pages: 3,
        extent: [SIDE, SIDE],
        full_damage: true,
    };
    present(&mut renderer, &frame)
}

#[test]
fn mark_axes_draw_as_the_mapped_payload() {
    let mapped = scaled_marks(1.0, [2.5, 0.0, 0.0, 2.5]);
    let expected = scaled_marks(2.5, [1.0, 0.0, 0.0, 1.0]);

    assert!(expected.rgba(18, 19)[3] > 200, "the control draws");
    assert!(
        mapped.max_difference(&expected) <= 1,
        "axes map the payload as a pre-scaled payload draws: {}",
        mapped.max_difference(&expected)
    );
}
