//! Small frames for the validation and fake renderer tests, and the instances they draw.

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, Batch, BatchKind, BorderStyle, ClipRef, Color, DeviceRect, FRAME_NO_CLIP, Frame,
    FrameMarks, FramePaint, ItemSource, MarkFlags, Marks, Matrix4, PageFormat, PageOp, PaintKind,
    PaintRef, Quad, ROUND, SpaceRef, Sprite,
};
use iceblink_testing::validate::{FrameViolation, Rule};

/// The side of every test output.
pub(crate) const SIDE: u32 = 64;

/// A device rectangle.
pub(crate) fn rect(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

/// A reference to paint slot `index`, which holds a solid paint.
pub(crate) fn solid(index: u32) -> PaintRef {
    PaintRef {
        kind: PaintKind::SOLID,
        _pad: [0; 3],
        index,
    }
}

/// An unclipped, borderless quad over `bounds` in space slot 0, filled with paint slot 0.
pub(crate) fn quad(bounds: [f32; 4]) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: solid(0),
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef(FRAME_NO_CLIP),
        space: SpaceRef(0),
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// A mono sprite of `tile` drawn at `origin`, one texel per pixel, in space slot 0.
pub(crate) fn sprite(origin: [f32; 2], tile: AtlasTile) -> Sprite {
    Sprite {
        bounds: [
            origin[0],
            origin[1],
            tile.rect[2] as f32,
            tile.rect[3] as f32,
        ],
        paint: solid(0),
        paint_origin: [0.0; 2],
        tile,
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    }
}

/// A marks item that draws `discs` discs of payload 0 with `flags`.
pub(crate) fn marks(discs: u32, flags: MarkFlags) -> FrameMarks {
    FrameMarks {
        marks: Marks {
            bounds: [0.0, 0.0, 16.0, 16.0],
            paint: solid(0),
            paint_origin: [0.0; 2],
            origin: [0.0; 2],
            axes: [1.0, 0.0, 0.0, 1.0],
            half_width: 0.0,
            discs,
            boxes: 0,
            vertices: 0,
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

/// A first frame of the output, with full damage, the identity in space slot 0 and opaque white
/// in paint slot 0, and nothing drawn.
pub(crate) fn first() -> Frame {
    let mut frame = Frame {
        size: [SIDE, SIDE],
        scale: 1.0,
        ..Frame::default()
    };
    frame.damage.draw = DamageSet::full();
    frame.damage.present = DamageSet::full();
    frame.tables.spaces.replace = true;
    frame.tables.spaces.all = vec![Matrix4::IDENTITY];
    frame.tables.paints.replace = true;
    frame.tables.paints.all = vec![FramePaint::Solid(Color([1.0; 4]))];
    frame.tables.clips.replace = true;
    frame.tables.params.replace = true;
    frame
}

/// [`first`] with one transient quad drawn by one batch.
pub(crate) fn with_quad() -> Frame {
    let mut frame = first();
    push_quad(&mut frame, quad([0.0, 0.0, 16.0, 16.0]));
    frame
}

/// Adds `quad` as a transient instance drawn by a batch of its own.
pub(crate) fn push_quad(frame: &mut Frame, quad: Quad) {
    let lane = &mut frame.lanes.quads;
    let at = lane.sources.len() as u32;
    lane.sources
        .push(ItemSource::transient(lane.items.len() as u32));
    lane.items.push(quad);
    frame.batches.push(batch(BatchKind::Quads, at..at + 1));
}

/// A batch over the whole output.
pub(crate) fn batch(kind: BatchKind, range: core::ops::Range<u32>) -> Batch {
    Batch {
        kind,
        range,
        ink: rect(0, 0, SIDE as i32, SIDE as i32),
    }
}

/// A batch of a kind drawn one at a time.
pub(crate) fn object(kind: BatchKind) -> Batch {
    batch(kind, 0..0)
}

/// Adds the creation of `page` in `format`, of the output's size.
pub(crate) fn create_page(frame: &mut Frame, page: u32, format: PageFormat) {
    frame.resources.pages.push(PageOp::Create {
        page,
        format,
        size: [SIDE, SIDE],
        levels: 1,
    });
}

/// Adds an upload of `rect` to `page` from tightly packed one-byte texels.
pub(crate) fn upload(frame: &mut Frame, page: u32, rect: DeviceRect) {
    let bytes = (rect.size.width * rect.size.height).max(0) as usize;
    frame.resources.pages.push(PageOp::Upload {
        page,
        level: 0,
        rect,
        bytes: Arc::from(vec![0u8; bytes]),
        offset: 0,
        stride: rect.size.width as u32,
    });
}

/// The rules `result` reports, in order and without repeats.
pub(crate) fn rules(result: Result<(), Vec<FrameViolation>>) -> Vec<Rule> {
    let mut rules: Vec<Rule> = result
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|violation| violation.rule)
        .collect();
    rules.sort();
    rules.dedup();
    rules
}
