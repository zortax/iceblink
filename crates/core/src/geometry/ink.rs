//! The local ink of each item kind.

use iceblink::kurbo::Rect as KurboRect;
use iceblink::{
    ClipRef, Extend, Image, ItemKind, ItemOp, LocalRect, Matrix4, PathItem, RecordedChunk, Shadow,
    SpaceRef, kurbo,
};

use crate::frame::lower::content::glyph_run_ink;
use crate::geometry::bounds::{DeviceBox, map, of_pixels};
use crate::scene::chunk::RunInk;

/// The rectangle an item may change in its local space, with the space and clip it is drawn
/// through.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalInk<'a> {
    pub(crate) rect: LocalRect,
    pub(crate) space: SpaceRef,
    pub(crate) clip: ClipRef,
    /// The measured ink of a glyph run.
    pub(crate) measured: Option<&'a RunInk>,
}

impl LocalInk<'_> {
    /// Returns the box the item may change on the device under `world` at chunk offset `offset`.
    ///
    /// A glyph run whose glyphs keep their pixels between where they were measured and here
    /// covers its measured pixels, moved with the glyphs. Every other item covers its rectangle
    /// mapped through `world`.
    pub(crate) fn on_device(&self, world: &Matrix4, offset: [f32; 2]) -> DeviceBox {
        if let Some(measured) = self.measured
            && let Some(pixels) = measured.device
            && let Some([dx, dy]) = measured.moved_by(world, offset)
        {
            return of_pixels(pixels).outset([-dx, -dy, dx, dy]);
        }

        let mut rect = self.rect;
        rect[0] += offset[0];
        rect[1] += offset[1];
        map(world, rect)
    }
}

/// Returns the local ink of the item `op` of `chunk`, or `None` when the item changes no pixel.
///
/// A glyph run takes its ink from `runs` when it has one there.
pub(crate) fn local_ink<'a>(
    chunk: &RecordedChunk,
    op: ItemOp,
    runs: &'a [Option<RunInk>],
) -> Option<LocalInk<'a>> {
    let index = op.index as usize;
    let mut measured = None;
    let (rect, space, clip) = match op.kind {
        ItemKind::QUAD => chunk
            .quads
            .get(index)
            .map(|i| (i.bounds, i.space, i.clip))?,
        ItemKind::SHADER_QUAD => chunk
            .shader_quads
            .get(index)
            .map(|i| (i.bounds, i.space, i.clip))?,
        ItemKind::SHADOW => chunk
            .shadows
            .get(index)
            .map(|i| (shadow_ink(i), i.space, i.clip))?,
        ItemKind::LINE => chunk
            .lines
            .get(index)
            .map(|i| (i.bounds, i.space, i.clip))?,
        ItemKind::GLYPH_RUN => {
            let run = chunk.glyph_runs.get(index)?;
            let rect = match runs.get(index).and_then(Option::as_ref) {
                Some(ink) => {
                    measured = Some(ink);
                    ink.local?
                }
                None => glyph_run_ink(run, chunk.glyphs.get(run.glyphs.indices())?)?,
            };
            (rect, run.space, run.clip)
        }
        ItemKind::IMAGE => chunk
            .images
            .get(index)
            .map(|i| (image_ink(i), i.space, i.clip))?,
        ItemKind::MARKS => chunk
            .marks
            .get(index)
            .map(|i| (i.bounds, i.space, i.clip))?,
        ItemKind::PATH => chunk
            .paths
            .get(index)
            .map(|i| (path_ink(i), i.space, i.clip))?,
        ItemKind::EXTERNAL => chunk
            .externals
            .get(index)
            .map(|i| (i.bounds, i.space, i.clip))?,
        ItemKind::BACKDROP => chunk
            .backdrops
            .get(index)
            .map(|i| (i.bounds, i.space, i.clip))?,
        _ => return None,
    };

    let empty = !(rect[2] > 0.0 && rect[3] > 0.0);
    (!empty).then_some(LocalInk {
        rect,
        space,
        clip,
        measured,
    })
}

/// Returns the ink of a shadow: the blurred shape for a drop shadow, and the caster for an inset
/// one.
pub(crate) fn shadow_ink(shadow: &Shadow) -> LocalRect {
    if shadow.inset != 0 {
        return shadow.caster_bounds;
    }

    let reach = 3.0 * shadow.blur.max(0.0);
    DeviceBox::of(shadow.shape_bounds).outset([reach; 4]).rect()
}

/// Returns the ink of an image: its bounds cut to its frame, widened to the frame along a
/// repeating axis.
fn image_ink(image: &Image) -> LocalRect {
    let bounds = DeviceBox::of(image.bounds);
    let frame = DeviceBox::of(image.frame);
    let mut ink = bounds.intersect(frame);

    if image.extend[0] == Extend::REPEAT {
        ink.x0 = frame.x0;
        ink.x1 = frame.x1;
    }
    if image.extend[1] == Extend::REPEAT {
        ink.y0 = frame.y0;
        ink.y1 = frame.y1;
    }

    ink.rect()
}

/// Returns the ink of a path: its control box grown by the stroke's reach, placed, and cut to the
/// box of its path clips.
fn path_ink(path: &PathItem) -> LocalRect {
    let reach = path
        .stroke
        .as_ref()
        .map_or(0.0, |stroke| stroke_reach(&stroke.style));
    let control = path.path.control_box().inflate(reach, reach);
    let mut placed = path.placement.transform_rect_bbox(control);

    for clip in path.clips.iter() {
        let clipped = path.placement.transform_rect_bbox(clip.path.control_box());
        placed = placed.intersect(clipped);
    }

    to_local(placed)
}

/// Returns how far a stroke reaches past its path: half the width, times the miter limit for miter
/// joins or the square root of two for square caps, whichever is larger.
fn stroke_reach(style: &kurbo::Stroke) -> f64 {
    let join = if style.join == kurbo::Join::Miter {
        style.miter_limit.max(1.0)
    } else {
        1.0
    };
    let square = [style.start_cap, style.end_cap].contains(&kurbo::Cap::Square);
    let cap = if square {
        core::f64::consts::SQRT_2
    } else {
        1.0
    };
    style.width.abs() / 2.0 * join.max(cap)
}

fn to_local(rect: KurboRect) -> LocalRect {
    let rect = rect.abs();
    [
        rect.x0 as f32,
        rect.y0 as f32,
        rect.width() as f32,
        rect.height() as f32,
    ]
}
