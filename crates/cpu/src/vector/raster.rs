//! Rasterising one vector pass into a premultiplied pixmap of its region.

use std::sync::Arc;

use iceblink::geom::{Device, Point, Rect, Size};
use iceblink::{
    ClipEntry, FRAME_NO_CLIP, FillRule, FramePaint, FramePath, Matrix4, PaintKind, PaintRef,
    PlannedPath,
};
use kurbo::{Affine, BezPath};
use vello_cpu::peniko::Fill;
use vello_cpu::{RenderContext, RenderMode, Resources};

use crate::vector::brush::brush;
use crate::vector::outline::ClipOutline;

/// The raster of one pass: its region on the device and its premultiplied texels.
#[derive(Clone, Debug)]
pub(crate) struct Raster {
    pub(crate) region: Rect<i32, Device>,
    pixels: Arc<vello_cpu::Pixmap>,
    // Keeps the outlines the raster was drawn from alive while the raster is kept.
    _outlines: Arc<[Arc<BezPath>]>,
}

impl Raster {
    /// Returns the same pixels at a region of the same size starting at `origin`.
    pub(crate) fn moved_to(&self, origin: Point<i32, Device>) -> Self {
        Self {
            region: Rect::new(origin, self.region.size),
            ..self.clone()
        }
    }

    /// Returns the texel that holds the device pixel `(x, y)`, premultiplied, or transparent
    /// outside the region.
    pub(crate) fn at(&self, x: i32, y: i32) -> [f32; 4] {
        let (column, row) = (x - self.region.left(), y - self.region.top());
        if column < 0 || row < 0 || column >= self.region.width() || row >= self.region.height() {
            return [0.0; 4];
        }
        let texel =
            self.pixels.data()[row as usize * self.region.width() as usize + column as usize];
        [texel.r, texel.g, texel.b, texel.a].map(|channel| f32::from(channel) / 255.0)
    }

    /// Returns the bytes the raster holds.
    pub(crate) fn bytes(&self) -> u64 {
        self.region.width().max(0) as u64 * self.region.height().max(0) as u64 * 4
    }
}

/// What rasterising a pass reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Inputs<'a> {
    pub(crate) paths: &'a [FramePath],
    pub(crate) clips: &'a [ClipEntry],
    pub(crate) paints: &'a [FramePaint],
    pub(crate) spaces: &'a [Matrix4],
}

/// Rasterises the paths `items` of a pass over `region`, in order, each through its residual.
///
/// A path whose residual holds a link with no outline is left out.
pub(crate) fn rasterise(
    context: &mut RenderContext,
    resources: &mut Resources,
    inputs: &Inputs<'_>,
    region: Rect<i32, Device>,
    items: &[PlannedPath],
) -> Raster {
    let width = region.width().clamp(1, i32::from(u16::MAX)) as u16;
    let height = region.height().clamp(1, i32::from(u16::MAX)) as u16;
    context.reset_and_resize(width, height);
    let shift = Affine::translate((-f64::from(region.left()), -f64::from(region.top())));
    let mut outlines = Vec::new();
    for planned in items {
        let Some(path) = inputs.paths.get(planned.path as usize) else {
            continue;
        };
        let Some(residual) = residual(inputs, planned) else {
            continue;
        };
        for outline in &residual {
            context.set_transform(shift * outline.transform);
            context.set_fill_rule(Fill::NonZero);
            context.push_clip_layer(&outline.path);
        }
        let placement = shift * path.transform;
        for clip in path.clips.iter() {
            context.set_transform(placement);
            context.set_fill_rule(fill_rule(clip.rule));
            context.push_clip_layer(&clip.path);
            outlines.push(Arc::clone(&clip.path));
        }
        place(context, inputs.paints, path, placement);
        for _ in 0..residual.len() + path.clips.len() {
            context.pop_layer();
        }
        outlines.push(Arc::clone(&path.path));
    }
    let mut pixels = vello_cpu::Pixmap::new(width, height);
    context.flush();
    context.render_with(
        &mut pixels,
        resources,
        vello_cpu::RasterizerSettings {
            render_mode: RenderMode::OptimizeQuality,
            ..Default::default()
        },
    );
    Raster {
        region: Rect::new(
            region.origin,
            Size::new(i32::from(width), i32::from(height)),
        ),
        pixels: Arc::new(pixels),
        _outlines: outlines.into(),
    }
}

// The outlines of the residual clip of `planned`, or `None` when a link has none.
fn residual(inputs: &Inputs<'_>, planned: &PlannedPath) -> Option<Vec<ClipOutline>> {
    if planned.residual == FRAME_NO_CLIP {
        return Some(Vec::new());
    }
    let Some(entry) = inputs.clips.get(planned.residual as usize) else {
        return Some(Vec::new());
    };
    entry
        .links
        .iter()
        .map(|link| ClipOutline::of(link, inputs.spaces))
        .collect()
}

// Fills and then strokes `path` under `placement`.
fn place(context: &mut RenderContext, paints: &[FramePaint], path: &FramePath, placement: Affine) {
    let painting = |reference: PaintRef| {
        if reference.kind == PaintKind::NONE {
            return None;
        }
        paints.get(reference.index as usize).and_then(brush)
    };
    context.set_transform(placement);
    if let Some(fill) = path.fill
        && let Some(painted) = painting(fill.paint)
    {
        context.set_paint_transform(path.brush * painted.transform.unwrap_or(Affine::IDENTITY));
        context.set_paint(painted.brush);
        context.set_fill_rule(fill_rule(fill.rule));
        context.fill_path(&path.path);
    }
    if let Some(stroke) = &path.stroke
        && let Some(painted) = painting(stroke.paint)
    {
        context.set_paint_transform(path.brush * painted.transform.unwrap_or(Affine::IDENTITY));
        context.set_paint(painted.brush);
        context.set_stroke(stroke.style.clone());
        context.stroke_path(&path.path);
    }
}

fn fill_rule(rule: FillRule) -> Fill {
    if rule == FillRule::EVEN_ODD {
        Fill::EvenOdd
    } else {
        Fill::NonZero
    }
}
