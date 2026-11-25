//! Discs, rings and squares.

use iceblink::MarkFlags;

use crate::draw::marks::mapping::{Mapping, Sample};
use crate::math::edge::{Distance, sdf_coverage};
use crate::math::saturate;

/// Returns the distance-space quad a disc draws over, as its two corners.
pub(crate) fn disc_quad(mapping: &Mapping<'_>, disc: [f32; 4]) -> ([f32; 2], [f32; 2]) {
    let centre = mapping.payload_point([disc[0], disc[1]]);
    let reach = disc[2] + mapping.margin;
    (
        [centre[0] - reach, centre[1] - reach],
        [centre[0] + reach, centre[1] + reach],
    )
}

/// Returns the coverage of a disc or ring at `sample`, or of a square with `SQUARE_DISCS`.
pub(crate) fn disc_coverage(
    mapping: &Mapping<'_>,
    flags: MarkFlags,
    disc: [f32; 4],
    sample: &Sample,
) -> f32 {
    let centre = mapping.payload_point([disc[0], disc[1]]);
    if !flags.contains(MarkFlags::SQUARE_DISCS) {
        let mut outer = Distance::radial(sample.point, centre, disc[2]);
        if disc[3] <= 0.0 {
            return sdf_coverage(outer, sample.across, sample.down);
        }
        outer.band = disc[2] - disc[3];
        let hole = Distance {
            d: disc[3] - (outer.d + disc[2]),
            gradient: [-outer.gradient[0], -outer.gradient[1]],
            band: outer.band,
        };
        return sdf_coverage(outer.max(hole), sample.across, sample.down);
    }
    let outer = square_cover(sample, centre, disc[2]);
    if disc[3] <= 0.0 {
        return outer;
    }
    saturate(outer - square_cover(sample, centre, disc[3]))
}

// The coverage of an axis-aligned square of half side `half` around `centre`.
fn square_cover(sample: &Sample, centre: [f32; 2], half: f32) -> f32 {
    let from = [sample.point[0] - centre[0], sample.point[1] - centre[1]];
    let flip = from.map(|value| if value >= 0.0 { 1.0 } else { -1.0 });
    let x = Distance {
        d: from[0].abs() - half,
        gradient: [flip[0], 0.0],
        band: 0.0,
    };
    let y = Distance {
        d: from[1].abs() - half,
        gradient: [0.0, flip[1]],
        band: 0.0,
    };
    sdf_coverage(x, sample.across, sample.down) * sdf_coverage(y, sample.across, sample.down)
}
