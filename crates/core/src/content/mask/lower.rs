//! Path items lowered into mono masks or frame paths.

use std::sync::Arc;

use iceblink::{AtlasTile, FillRule, PaintKind, PaintRef, PathItem, Sprite};
use kurbo::{Affine, Stroke};

use crate::content::cache::ContentCache;
use crate::content::key::TileKey;
use crate::content::lowering::{Lowered, Lowering};
use crate::content::mask::density::{density_of, phase_of, uniform_axes};
use crate::content::mask::fingerprint::Fingerprint;
use crate::content::mask::shape::{MaskPlan, Shape};
use crate::content::mask::{MaskAnswer, Part, PathOwner};
use crate::content::path::{PlacedPath, frame_path, stroke_reach};
use crate::content::target::Target;

/// How a path item was lowered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRoute {
    /// One mono sprite for the fill, then one for the stroke, over mask tiles.
    Masks,
    /// One frame path.
    Path,
    /// Nothing, since the item has neither a fill nor a stroke.
    Nothing,
}

impl ContentCache {
    /// Lowers a path item drawn at `target`, appending its instances and tiles to `out`.
    ///
    /// The path becomes masks when all of these hold:
    ///
    /// 1. The item has no path clips.
    /// 2. Its fill and stroke paints are solid.
    /// 3. A stroke's placement scales both axes alike.
    /// 4. The world matrix is two-dimensional, keeps the axes or turns them by quarter turns, and
    ///    scales both axes alike when there is a stroke.
    /// 5. Each part's mask is at least one texel and at most 1024 by 256 texels and 65536 texels
    ///    in all, with finite points and a stroke of positive finite lengths.
    /// 6. The path's route history admits it, as described below.
    /// 7. The atlas holds its masks.
    ///
    /// Otherwise it becomes one frame path.
    ///
    /// # Route history
    ///
    /// The history of path `owner` keeps the two most recent distinct masks of each part. When no
    /// part asks for a mask other than its most recent one, the path takes the route it took
    /// last. Otherwise it takes masks, unless it is volatile and has more than 4096 segments or
    /// its new masks would take the frame past 256 new masks or 2²⁰ texels of volatile paths. A
    /// path turns volatile once a part asked for a mask it kept neither of in three of the last
    /// four frames, and stays volatile until four frames pass without one.
    ///
    /// A history is forgotten once its masks did not differ for eight counted frames (see
    /// [`ContentCache::count_frame`]), and a path without a history takes masks.
    pub fn lower_path(
        &mut self,
        item: &PathItem,
        target: &Target,
        owner: PathOwner,
        out: &mut Lowering,
    ) -> PathRoute {
        if item.fill.is_none() && item.stroke.is_none() {
            return PathRoute::Nothing;
        }

        let placement =
            Affine::translate((f64::from(target.offset[0]), f64::from(target.offset[1])))
                * item.placement;
        if self.lower_masks(item, placement, target, owner, out) {
            return PathRoute::Masks;
        }

        let placed = PlacedPath {
            path: &item.path,
            placement,
            brush: item.brush,
            fill: item.fill,
            stroke: item.stroke.clone(),
            clips: &item.clips,
        };
        out.items
            .push(Lowered::Path(frame_path(placed, &target.world)));
        PathRoute::Path
    }

    /// Appends the masks of `item`, and returns false when the item does not qualify.
    ///
    /// Both parts become masks or neither does.
    fn lower_masks(
        &mut self,
        item: &PathItem,
        placement: Affine,
        target: &Target,
        owner: PathOwner,
        out: &mut Lowering,
    ) -> bool {
        if !item.clips.is_empty() {
            return false;
        }

        let solid = |paint: PaintRef| paint.kind == PaintKind::SOLID;
        if item.fill.is_some_and(|fill| !solid(fill.paint))
            || item
                .stroke
                .as_ref()
                .is_some_and(|stroke| !solid(stroke.paint))
        {
            return false;
        }

        let stroked = item.stroke.is_some();
        if stroked && !uniform_axes(item.placement) {
            return false;
        }
        let Some(world) = target.affine() else {
            return false;
        };
        let Some(density) = density_of(world, stroked) else {
            return false;
        };

        let plan = MaskPlan {
            item,
            placement,
            density,
            phase: phase_of(world),
            target,
        };
        let bounds = placement.transform_rect_bbox(item.path.control_box());

        let mut shapes: [Option<(Shape, PaintRef)>; 2] = [None, None];
        if let Some(fill) = item.fill {
            let part = Part::Fill(fill.rule == FillRule::EVEN_ODD);
            let Some(shape) = plan.shape(part, bounds) else {
                return false;
            };
            shapes[0] = Some((shape, fill.paint));
        }
        if let Some(stroke) = &item.stroke {
            // The stroke is measured in path space, which the placement scales.
            let scale = placement_scale(item.placement);
            let style = scaled(&stroke.style, scale);
            let reach = stroke_reach(stroke) * scale;
            let local = bounds.inflate(reach, reach);
            let Some(shape) = plan.shape(Part::Stroke(&style), local) else {
                return false;
            };
            shapes[1] = Some((shape, stroke.paint));
        }

        let masks = shapes
            .each_ref()
            .map(|shape| shape.as_ref().map(|(shape, _)| &*shape.mask));
        if !self.masks.route(owner, masks, &item.path) {
            return false;
        }

        // One part alone as a mask would draw a different picture, so parts are appended only
        // once both have tiles.
        let mut parts: [Option<(Sprite, TileKey)>; 2] = [None, None];
        for (part, (slot, shape)) in parts.iter_mut().zip(&shapes).enumerate() {
            if let Some((shape, paint)) = shape {
                let Some((tile, key)) = self.mask_tile(&shape.mask) else {
                    return false;
                };
                self.masks.track(owner, part, key);
                *slot = Some((plan.sprite(shape, tile, *paint), key));
            }
        }

        for (sprite, key) in parts.into_iter().flatten() {
            out.items.push(Lowered::Mono(sprite));
            out.tiles.push(key);
        }
        true
    }

    /// Returns the tile of `mask`, evicting the oldest tiles once when the pool is full, or
    /// `None` when the atlas cannot hold it.
    fn mask_tile(&mut self, mask: &Arc<Fingerprint>) -> Option<(AtlasTile, TileKey)> {
        match self.masks.tile_for(&mut self.tiles, mask) {
            MaskAnswer::Tile(tile, key) => Some((tile, key)),
            MaskAnswer::Refused => None,
            MaskAnswer::NoRoom => {
                if !self.evict_oldest() {
                    return None;
                }
                match self.masks.tile_for(&mut self.tiles, mask) {
                    MaskAnswer::Tile(tile, key) => Some((tile, key)),
                    MaskAnswer::Refused | MaskAnswer::NoRoom => None,
                }
            }
        }
    }
}

/// Returns the scale of `placement`'s first axis, which a stroke's lengths take.
fn placement_scale(placement: Affine) -> f64 {
    let [a, b, ..] = placement.as_coeffs();
    a.hypot(b)
}

/// Returns `stroke` with its width and dashes scaled by `scale`.
fn scaled(stroke: &Stroke, scale: f64) -> Stroke {
    let mut scaled = stroke.clone();
    scaled.width *= scale;
    scaled.dash_offset *= scale;
    for dash in scaled.dash_pattern.iter_mut() {
        *dash *= scale;
    }
    scaled
}
