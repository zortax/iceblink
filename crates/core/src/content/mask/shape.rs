//! Where one part of a path's masks lies, and the sprite that draws it.

use std::sync::Arc;

use iceblink::{AtlasTile, PaintRef, PathItem, Sprite};
use kurbo::{Affine, Rect};

use crate::content::mask::density::fits;
use crate::content::mask::fingerprint::Fingerprint;
use crate::content::mask::{MaskRequest, Part};
use crate::content::target::Target;

/// What every part of one path's masks shares.
pub(super) struct MaskPlan<'a> {
    pub(super) item: &'a PathItem,
    /// Maps path space into local space, chunk offset included.
    pub(super) placement: Affine,
    pub(super) density: [f32; 2],
    pub(super) phase: [f32; 2],
    pub(super) target: &'a Target,
}

/// One part's mask and where it lies.
pub(super) struct Shape {
    pub(super) mask: Arc<Fingerprint>,
    /// The mask's corner in mask space.
    corner: [f64; 2],
}

impl MaskPlan<'_> {
    /// Returns the mask of one part whose ink in local space is `local`, or `None` when the mask
    /// is too small, too large or not finite.
    pub(super) fn shape(&self, part: Part<'_>, local: Rect) -> Option<Shape> {
        let [kx, ky] = self.density.map(f64::from);
        let [px, py] = self.phase.map(f64::from);
        let left = (local.x0 * kx + px).floor();
        let top = (local.y0 * ky + py).floor();
        let right = (local.x1 * kx + px).ceil();
        let bottom = (local.y1 * ky + py).ceil();
        if ![left, top, right, bottom]
            .iter()
            .all(|edge| edge.is_finite())
        {
            return None;
        }

        let (width, height) = ((right - left) as f32, (bottom - top) as f32);
        if !fits(width, height) {
            return None;
        }

        let request = MaskRequest {
            path: &self.item.path,
            placement: self.placement,
            part,
            density: self.density,
            phase: self.phase,
            bounds: [left as i32, top as i32, width as i32, height as i32],
        };
        Some(Shape {
            mask: Arc::new(Fingerprint::of(&request)?),
            corner: [left, top],
        })
    }

    /// Returns the sprite that draws `shape` from `tile` with `paint`.
    pub(super) fn sprite(&self, shape: &Shape, tile: AtlasTile, paint: PaintRef) -> Sprite {
        let [kx, ky] = self.density.map(f64::from);
        let [px, py] = self.phase.map(f64::from);
        let [left, top] = shape.corner;
        let [width, height] = shape.mask.size.map(|side| side as f32);
        Sprite {
            bounds: [
                ((left - px) / kx) as f32,
                ((top - py) / ky) as f32,
                width / self.density[0],
                height / self.density[1],
            ],
            paint,
            paint_origin: self.target.offset,
            tile,
            clip: self.target.clip,
            space: self.target.space,
            _pad: 0,
        }
    }
}
