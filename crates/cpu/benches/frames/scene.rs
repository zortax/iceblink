//! A frame like an application window's: cards with borders and shadows, lines of text, a
//! blurred overlay and a frosted panel.

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, Backdrop, Batch, BatchKind, BlendMode, BorderStyle, ClipRef, Color, FRAME_NO_CLIP,
    Frame, FrameBackdrop, FrameFilter, FrameGroup, FramePaint, FrameShadow, ItemSource, Matrix4,
    PageFormat, PageOp, PaintKind, PaintRef, Quad, ROUND, Shadow, SpaceRef, Sprite,
};

/// The output's size.
pub(crate) const SIZE: [u32; 2] = [1920, 1080];

/// Returns the frame, redrawing `damage` as `[x, y, width, height]`, or the whole output, with
/// the overlay and the panel when `effects` is set.
pub(crate) fn frame(damage: Option<[i32; 4]>, effects: bool) -> Frame {
    let mut frame = Frame {
        size: SIZE,
        scale: 1.0,
        opaque: true,
        ..Frame::default()
    };
    match damage {
        Some([x, y, width, height]) => {
            let mut set = DamageSet::new();
            set.absorb(Rect::new(Point::new(x, y), Size::new(width, height)));
            frame.damage.draw = set;
            frame.damage.present = set;
        }
        None => {
            frame.damage.draw = DamageSet::full();
            frame.damage.present = DamageSet::full();
        }
    }
    frame.tables.spaces.replace = true;
    frame.tables.spaces.all = vec![Matrix4::IDENTITY];
    frame.tables.paints.replace = true;
    frame.tables.paints.all = vec![
        FramePaint::Solid(Color([0.95, 0.95, 0.96, 1.0])),
        FramePaint::Solid(Color([1.0, 1.0, 1.0, 1.0])),
        FramePaint::Solid(Color([0.8, 0.8, 0.85, 1.0])),
        FramePaint::Solid(Color([0.1, 0.1, 0.12, 1.0])),
        FramePaint::Solid(Color([0.2, 0.4, 0.9, 0.3])),
    ];
    frame.resources.pages.push(PageOp::Create {
        page: 0,
        format: PageFormat::A8,
        size: [64, 64],
        levels: 1,
    });
    let glyph: Vec<u8> = (0..64 * 64)
        .map(|index| {
            if (index / 64 + index % 64) % 7 < 4 {
                220
            } else {
                30
            }
        })
        .collect();
    frame.resources.pages.push(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(64, 64)),
        bytes: glyph.into(),
        offset: 0,
        stride: 64,
    });

    quad(&mut frame, [0.0, 0.0, 1920.0, 1080.0], 0, 0.0, 0.0);
    // Shadows and cards in a grid.
    for row in 0..8 {
        for column in 0..12 {
            let x = 40.0 + column as f32 * 155.0;
            let y = 40.0 + row as f32 * 128.0;
            shadow(&mut frame, [x, y, 140.0, 110.0]);
            quad(&mut frame, [x, y, 140.0, 110.0], 1, 8.0, 1.0);
            // Text: three lines of glyphs.
            for line in 0..3 {
                for letter in 0..14 {
                    sprite(
                        &mut frame,
                        [
                            x + 8.0 + letter as f32 * 9.0,
                            y + 12.0 + line as f32 * 18.0,
                            8.0,
                            12.0,
                        ],
                    );
                }
            }
        }
    }
    if effects {
        group(
            &mut frame,
            [1500.0, 100.0, 380.0, 600.0],
            FrameFilter::Blur(8.0),
        );
        backdrop(&mut frame, [300.0, 700.0, 900.0, 300.0]);
    }
    frame
}

fn batch(frame: &mut Frame, kind: BatchKind, position: u32) {
    let ink = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
    let range = position..position + 1;
    if let Some(last) = frame.batches.last_mut()
        && last.kind == kind
        && last.range.end == position
    {
        last.range.end += 1;
        return;
    }
    frame.batches.push(Batch { kind, range, ink });
}

fn quad(frame: &mut Frame, bounds: [f32; 4], fill: u32, radius: f32, border: f32) {
    let lane = &mut frame.lanes.quads;
    let position = lane.items.len() as u32;
    lane.items.push(Quad {
        bounds,
        radii: [radius; 8],
        shapes: ROUND,
        border: [border; 4],
        fill: PaintRef {
            kind: PaintKind::SOLID,
            _pad: [0; 3],
            index: fill,
        },
        stroke: PaintRef {
            kind: PaintKind::SOLID,
            _pad: [0; 3],
            index: 2,
        },
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    });
    lane.sources.push(ItemSource::transient(position));
    batch(frame, BatchKind::Quads, position);
}

fn shadow(frame: &mut Frame, bounds: [f32; 4]) {
    let lane = &mut frame.lanes.shadows;
    let position = lane.items.len() as u32;
    let blur = 4.0;
    lane.items.push(FrameShadow {
        bounds: [
            bounds[0] - 3.0 * blur,
            bounds[1] - 3.0 * blur + 2.0,
            bounds[2] + 6.0 * blur,
            bounds[3] + 6.0 * blur,
        ],
        shadow: Shadow {
            shape_bounds: [bounds[0], bounds[1] + 2.0, bounds[2], bounds[3]],
            radii: [8.0; 8],
            caster_bounds: bounds,
            caster_radii: [8.0; 8],
            shapes: ROUND,
            color: Color([0.0, 0.0, 0.0, 0.25]),
            blur,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            inset: 0,
        },
    });
    lane.sources.push(ItemSource::transient(position));
    batch(frame, BatchKind::Shadows, position);
}

fn sprite(frame: &mut Frame, bounds: [f32; 4]) {
    let lane = &mut frame.lanes.mono;
    let position = lane.items.len() as u32;
    lane.items.push(Sprite {
        bounds,
        paint: PaintRef {
            kind: PaintKind::SOLID,
            _pad: [0; 3],
            index: 3,
        },
        paint_origin: [0.0; 2],
        tile: AtlasTile {
            page: 0,
            rect: [0, 0, 8, 12],
        },
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    });
    lane.sources.push(ItemSource::transient(position));
    batch(frame, BatchKind::Mono { page: 0 }, position);
}

fn group(frame: &mut Frame, bounds: [f32; 4], filter: FrameFilter) {
    let index = frame.groups.len() as u32;
    let reach = 3.0 * 8.0 + 2.0;
    let grown = [
        bounds[0] - reach,
        bounds[1] - reach,
        bounds[2] + 2.0 * reach,
        bounds[3] + 2.0 * reach,
    ];
    frame.groups.push(FrameGroup {
        bounds: grown,
        source: grown,
        clip: FRAME_NO_CLIP,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![filter]),
    });
    let ink = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
    frame.batches.push(Batch {
        kind: BatchKind::GroupBegin(index),
        range: 0..0,
        ink,
    });
    quad(frame, bounds, 4, 12.0, 0.0);
    frame.batches.push(Batch {
        kind: BatchKind::GroupEnd(index),
        range: 0..0,
        ink,
    });
}

fn backdrop(frame: &mut Frame, bounds: [f32; 4]) {
    let index = frame.backdrops.len() as u32;
    let reach = 3.0 * 6.0 + 2.0;
    frame.backdrops.push(FrameBackdrop {
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
        filters: Arc::from(vec![FrameFilter::Blur(6.0)]),
    });
    let ink = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
    frame.batches.push(Batch {
        kind: BatchKind::Backdrop(index),
        range: 0..0,
        ink,
    });
}
