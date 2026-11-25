//! Compositing a rasterised vector pass at its place in paint order.

use iceblink::geom::{Device, Point, Rect};
use iceblink::{PlannedPass, VectorPlan};

use crate::draw::Scene;
use crate::math::vec::scale4;
use crate::target::Surface;
use crate::vector::Raster;

/// Composites `raster`, the raster of `pass`, into `surface`, cut to `scissor`.
///
/// A pass that is not instanced is composited whole through the pass clip. An instanced pass is
/// composited one path at a time over the path's ink, through the path's own clip.
pub(crate) fn draw_pass(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    plan: &VectorPlan,
    pass: &PlannedPass,
    raster: &Raster,
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    if !pass.instanced {
        composite(
            scene,
            surface,
            within,
            raster.region,
            pass.clip,
            raster,
            row,
        );
        return;
    }
    for item in plan.items_of(pass) {
        let bounds = Rect::new(
            Point::new(
                raster.region.left() + item.ink.left(),
                raster.region.top() + item.ink.top(),
            ),
            item.ink.size,
        );
        composite(scene, surface, within, bounds, item.clip, raster, row);
    }
}

fn composite(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    within: Rect<i32, Device>,
    bounds: Rect<i32, Device>,
    clip: u32,
    raster: &Raster,
    row: &mut Vec<[f32; 4]>,
) {
    let Some(area) = bounds.intersection(within) else {
        return;
    };
    let clip = scene.clip(clip);
    for y in area.top()..area.bottom() {
        row.clear();
        row.extend((area.left()..area.right()).map(|x| {
            let device = [x as f32 + 0.5, y as f32 + 0.5];
            let coverage = scene.clip_coverage(clip, device);
            if coverage <= 0.0 {
                return [0.0; 4];
            }
            scale4(raster.at(x, y), coverage)
        }));
        surface.over_row(area.left(), y, row);
    }
}
