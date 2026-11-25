//! Drop and inset shadows.

use iceblink::{ClipRef, Color, FRAME_NO_CLIP, FrameShadow, ROUND, Shadow, SpaceRef};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Blurred edges: a pixel may differ by a few steps across the falloff.
// A renderer may integrate the Gaussian by rows.
const FALLOFF: Tolerance = Tolerance {
    max_channel: 4,
    max_pixels: 96,
};

fn shadow(cell: [f32; 4], radius: f32, blur: f32, inset: bool) -> FrameShadow {
    let [x, y, width, height] = cell;
    let caster = [x + 6.0, y + 6.0, width - 12.0, height - 12.0];
    let shape = if inset {
        [
            caster[0] + 3.0,
            caster[1] + 3.0,
            caster[2] - 6.0,
            caster[3] - 6.0,
        ]
    } else {
        [caster[0] + 3.0, caster[1] + 3.0, caster[2], caster[3]]
    };
    let bounds = if inset {
        caster
    } else {
        let reach = 3.0 * blur;
        [
            shape[0] - reach,
            shape[1] - reach,
            shape[2] + 2.0 * reach,
            shape[3] + 2.0 * reach,
        ]
    };
    FrameShadow {
        bounds,
        shadow: Shadow {
            shape_bounds: shape,
            radii: [radius; 8],
            caster_bounds: caster,
            caster_radii: [radius; 8],
            shapes: ROUND,
            color: Color([0.0, 0.0, 0.0, 0.6]),
            blur,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            inset: u32::from(inset),
        },
    }
}

/// Drop shadows hard and blurred, square and rounded, and inset shadows.
pub fn shadows(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let shadows = [
        shadow(cell(0), 0.0, 0.0, false),
        shadow(cell(1), 6.0, 2.0, false),
        shadow(cell(2), 10.0, 4.0, false),
        shadow(cell(3), 0.0, 2.0, true),
        shadow(cell(4), 8.0, 3.0, true),
        shadow(cell(5), 4.0, 0.0, true),
    ];
    for shadow in shadows {
        builder.shadow(shadow);
    }
    golden("shadows", &canvas.draw(builder.finish()), FALLOFF);
}
