//! Boxes, rounded and bordered.

use iceblink::MarkBox;

use crate::draw::marks::mapping::{Mapping, Sample};
use crate::math::edge::{Distance, sdf_coverage};
use crate::math::saturate;
use crate::math::sdf::{CORNER_ROUND, pick_corner_radii, quad_sdf_impl};
use crate::math::vec::length;

/// Returns the distance-space quad a box draws over, as its two corners.
pub(crate) fn box_quad(mapping: &Mapping<'_>, prim: &MarkBox) -> ([f32; 2], [f32; 2]) {
    let margin = mapping.margin;
    let low = mapping.payload_point([prim.rect[0], prim.rect[1]]);
    let high = mapping.payload_point([prim.rect[2], prim.rect[3]]);
    (
        [low[0] - margin, low[1] - margin],
        [high[0] + margin, high[1] + margin],
    )
}

/// Returns the coverage of a box at `sample`: its area, less the area inside its border when it
/// has one.
pub(crate) fn box_coverage(mapping: &Mapping<'_>, prim: &MarkBox, sample: &Sample) -> f32 {
    let low = mapping.payload_point([prim.rect[0], prim.rect[1]]);
    let high = mapping.payload_point([prim.rect[2], prim.rect[3]]);
    let rect = [low[0], low[1], high[0], high[1]];
    let outer = box_cover(sample, rect, &prim.radii, prim.shape[0]);
    let border = prim.shape[1];
    if border <= 0.0 {
        return outer;
    }
    let inner_low = [rect[0] + border, rect[1] + border];
    let inner = [
        inner_low[0],
        inner_low[1],
        (rect[2] - border).max(inner_low[0]),
        (rect[3] - border).max(inner_low[1]),
    ];
    let radii = prim.radii.map(|radius| (radius - border).max(0.0));
    saturate(outer - box_cover(sample, inner, &radii, prim.shape[0]))
}

// The coverage of the box `rect` (x0 y0 x1 y1) at `sample`.
fn box_cover(sample: &Sample, rect: [f32; 4], radii: &[f32; 8], shape: f32) -> f32 {
    let half = [0.5 * (rect[2] - rect[0]), 0.5 * (rect[3] - rect[1])];
    let centre_to_point = [
        sample.point[0] - 0.5 * (rect[0] + rect[2]),
        sample.point[1] - 0.5 * (rect[1] + rect[3]),
    ];
    let flip = centre_to_point.map(|value| if value >= 0.0 { 1.0 } else { -1.0 });
    let r = pick_corner_radii(centre_to_point, radii);
    let corner = [
        centre_to_point[0].abs() - half[0],
        centre_to_point[1].abs() - half[1],
    ];
    let from_centre = [corner[0] + r[0], corner[1] + r[1]];
    if from_centre[0] > 0.0 && from_centre[1] > 0.0 && r[0] > 0.0 && r[1] > 0.0 {
        let d = quad_sdf_impl(from_centre, r, shape);
        let gradient = if shape == CORNER_ROUND && r[0] == r[1] {
            let length = length(from_centre);
            [
                from_centre[0] / length * flip[0],
                from_centre[1] / length * flip[1],
            ]
        } else {
            let step = 0.01 * length(sample.across).max(length(sample.down)).max(1e-6);
            [
                (quad_sdf_impl([from_centre[0] + step, from_centre[1]], r, shape) - d) / step
                    * flip[0],
                (quad_sdf_impl([from_centre[0], from_centre[1] + step], r, shape) - d) / step
                    * flip[1],
            ]
        };
        return sdf_coverage(
            Distance {
                d,
                gradient,
                band: 0.0,
            },
            sample.across,
            sample.down,
        );
    }
    let x = Distance {
        d: corner[0],
        gradient: [flip[0], 0.0],
        band: 0.0,
    };
    let y = Distance {
        d: corner[1],
        gradient: [0.0, flip[1]],
        band: 0.0,
    };
    sdf_coverage(x, sample.across, sample.down) * sdf_coverage(y, sample.across, sample.down)
}
