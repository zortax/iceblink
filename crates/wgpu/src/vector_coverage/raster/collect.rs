//! The fills of a round's passes.

use core::ops::Range;

use iceblink::{DeviceRect, FillRule, FrameClipShape, FramePath, PlannedPath};
use kurbo::Affine;

use crate::upload::{TEXEL, TableTexture};
use crate::vector::paint::{flat, resolve};
use crate::vector::{ClipOutline, ResourcedPass, VectorFrame, VectorTarget};
use crate::vector_coverage::geometry;
use crate::vector_coverage::instance::{Item, Run};
use crate::vector_coverage::raster::CoverageRaster;
use crate::vector_coverage::raster::band::band;

/// How many table texels one fill takes.
const ITEM_TEXELS: usize = size_of::<Item>() / TEXEL;

impl CoverageRaster {
    /// Collects the fills of the passes of `round`, and returns the first pass that did not fit,
    /// if any.
    ///
    /// A pass with no target, or whose fills would take the tables past what a texture holds,
    /// ends the collection, and the passes before it are kept.
    pub(super) fn collect_round(
        &mut self,
        frame: &VectorFrame<'_>,
        round: Range<usize>,
    ) -> Option<usize> {
        let limit = TableTexture::limit(&self.device);
        for index in round {
            let pass = &frame.plan.passes[index];
            if pass.target == VectorTarget::NONE {
                return Some(index);
            }

            let before = (
                self.items.len(),
                self.segments.len(),
                self.runs.len(),
                self.bands.len(),
            );
            let span = self.collect(frame, pass);
            let fits = self.items.len() * ITEM_TEXELS <= limit
                && self.segments.len() <= limit
                && self.runs.len() <= limit
                && self.bands.len() <= limit;
            if !fits {
                self.items.truncate(before.0);
                self.segments.truncate(before.1);
                self.runs.truncate(before.2);
                self.bands.truncate(before.3);
                return Some(index);
            }
            self.spans.push(span);
        }
        None
    }

    /// Appends the fills of the paths of `pass`, and returns the first fill and how many there
    /// are.
    fn collect(&mut self, frame: &VectorFrame<'_>, pass: &ResourcedPass) -> (u32, u32) {
        let first = self.items.len() as u32;
        // The layer's extent, since the draw maps onto the whole layer.
        let (width, height) = self.scratch.extent();
        let viewport = [width as f32, height as f32, 0.0, 0.0];
        let shift = Affine::translate((
            f64::from(pass.raster_region.origin.x - pass.region.origin.x),
            f64::from(pass.raster_region.origin.y - pass.region.origin.y),
        ));

        for planned in frame.plan.items_of(pass) {
            let Some(path) = frame.path(planned) else {
                continue;
            };
            let Some(clips) = self.clip_runs(frame, planned, path, shift) else {
                self.last.unclippable += 1;
                continue;
            };
            let bounds = bounds(planned, pass);
            let placement = shift * path.transform;
            let tolerance = geometry::tolerance(placement);
            let mut painted = false;

            if let Some(fill) = &path.fill
                && let Some(color) = resolve(fill.paint, frame.paints).and_then(flat)
            {
                self.outlines
                    .outline(&mut self.flattened, &path.path, None, placement, tolerance);
                let even_odd = fill.rule == FillRule::EVEN_ODD;
                self.push(bounds, viewport, color, even_odd, clips);
                painted = true;
            }
            if let Some(stroke) = &path.stroke
                && let Some(color) = resolve(stroke.paint, frame.paints).and_then(flat)
            {
                self.outlines.outline(
                    &mut self.flattened,
                    &path.path,
                    Some(&stroke.style),
                    placement,
                    tolerance,
                );
                // A stroke's outline is a boundary and is filled by the non-zero rule, whatever
                // the rule of the fill.
                self.push(bounds, viewport, color, false, clips);
                painted = true;
            }
            if !painted {
                self.last.unpaintable += 1;
            }
        }
        (first, self.items.len() as u32 - first)
    }

    /// Appends the clip runs of `planned`, and returns the first run and how many there are, or
    /// `None` when a residual link has no outline.
    ///
    /// The residual's links go through their own spaces, and the path's own clips through the
    /// path's transform.
    fn clip_runs(
        &mut self,
        frame: &VectorFrame<'_>,
        planned: &PlannedPath,
        path: &FramePath,
        shift: Affine,
    ) -> Option<(usize, usize)> {
        let residual = frame.residual(planned);
        if residual
            .iter()
            .any(|link| matches!(link.shape, FrameClipShape::Mask { .. }))
        {
            return None;
        }
        let first = self.runs.len();
        for link in residual {
            let outline = ClipOutline::of(link, frame.spaces)?;
            let placement = shift * outline.transform;
            self.run(
                &outline.path,
                placement,
                geometry::tolerance(placement),
                false,
            );
        }
        let placement = shift * path.transform;
        let tolerance = geometry::tolerance(placement);
        for clip in path.clips.iter() {
            self.run(
                &clip.path,
                placement,
                tolerance,
                clip.rule == FillRule::EVEN_ODD,
            );
        }
        Some((first, self.runs.len() - first))
    }

    /// Appends one clip run of `outline` under `placement`.
    fn run(&mut self, outline: &kurbo::BezPath, placement: Affine, tolerance: f64, even_odd: bool) {
        let start = self.segments.len();
        geometry::flatten(outline, placement, tolerance, &mut self.segments);
        let count = self.segments.len() - start;
        self.runs.push(Run::new(start, count, even_odd));
        self.last.segments += count as u32;
        self.last.clip_layers += 1;
    }

    /// Appends one fill of the outline in `flattened`.
    fn push(
        &mut self,
        bounds: [f32; 4],
        viewport: [f32; 4],
        color: [f32; 4],
        even_odd: bool,
        (clip_first, clip_count): (usize, usize),
    ) {
        self.last.segments += self.flattened.len() as u32;
        let banding = band(
            &self.flattened,
            bounds,
            &mut self.bands,
            &mut self.segments,
            &mut self.tally,
        );
        self.items.push(Item {
            bounds,
            viewport,
            color,
            control: [
                banding.first as f32,
                banding.count as f32,
                f32::from(u8::from(even_odd)),
                clip_first as f32,
            ],
            bands: [clip_count as f32, banding.top, banding.tall, 0.0],
        });
    }
}

/// Returns the quad that fills `planned` in the layer, as origin and extent.
///
/// The quad is the path's ink in the pass's bin with a pixel of slack on each side, kept inside
/// the bin when the pass is clamped.
fn bounds(planned: &PlannedPath, pass: &ResourcedPass) -> [f32; 4] {
    let origin = pass.raster_region.origin;
    let bounds = [
        (origin.x + planned.ink.origin.x) as f32 - 1.0,
        (origin.y + planned.ink.origin.y) as f32 - 1.0,
        planned.ink.size.width as f32 + 2.0,
        planned.ink.size.height as f32 + 2.0,
    ];
    if pass.clamped {
        clamped(bounds, pass.raster_region)
    } else {
        bounds
    }
}

/// Returns `bounds`, as origin and extent, cut to `region`.
fn clamped(bounds: [f32; 4], region: DeviceRect) -> [f32; 4] {
    let left = bounds[0].max(region.left() as f32);
    let top = bounds[1].max(region.top() as f32);
    let right = (bounds[0] + bounds[2]).min(region.right() as f32);
    let bottom = (bounds[1] + bounds[3]).min(region.bottom() as f32);
    [left, top, (right - left).max(0.0), (bottom - top).max(0.0)]
}
