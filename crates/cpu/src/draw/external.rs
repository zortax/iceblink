//! External quads: an image the renderer did not draw, stretched over a rectangle.

use iceblink::ExternalQuad;
use iceblink::geom::{Device, Rect};

use crate::draw::footprint::{footprint, holds};
use crate::draw::{Scene, paint_area};
use crate::external::ExternalImage;
use crate::math::vec::scale4;
use crate::target::Surface;

/// Draws `quad` with `image` into `surface`, cut to `scissor`.
pub(crate) fn draw_external(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    quad: &ExternalQuad,
    image: &ExternalImage,
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    let space = scene.space(quad.space.0);
    let Some(area) = footprint(space, quad.bounds, within) else {
        return;
    };
    let clip = scene.clip(quad.clip.0);
    let opacity = quad.opacity.clamp(0.0, 1.0);
    paint_area(surface, area, row, |device| {
        let Some(point) = space.local(device) else {
            return [0.0; 4];
        };
        if !holds(quad.bounds, point) {
            return [0.0; 4];
        }
        let coverage = scene.clip_coverage(clip, device);
        if coverage <= 0.0 {
            return [0.0; 4];
        }
        let uv = [
            (point[0] - quad.bounds[0]) / quad.bounds[2],
            (point[1] - quad.bounds[1]) / quad.bounds[3],
        ];
        scale4(image.sample(uv), coverage * opacity)
    });
}
