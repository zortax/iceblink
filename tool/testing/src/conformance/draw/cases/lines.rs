//! Decoration lines in every style.

use iceblink::{ClipRef, FRAME_NO_CLIP, Line, LineStyle, SpaceRef};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden};
use crate::golden::Tolerance;

/// Antialiased edges: a pixel may differ by a few steps where its coverage is partial.
const EDGES: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 24,
};

/// Solid, wavy, dashed, dotted and double lines at two thicknesses.
pub fn lines(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));

    let styles = [
        (LineStyle::SOLID, 2.0, 2.0),
        (LineStyle::WAVY, 1.5, 6.0),
        (LineStyle::DASHED, 2.0, 2.0),
        (LineStyle::DOTTED, 3.0, 3.0),
        (LineStyle::DOUBLE, 1.0, 6.0),
        (LineStyle::SOLID, 0.5, 1.0),
        (LineStyle::WAVY, 3.0, 10.0),
        (LineStyle::DASHED, 1.0, 1.5),
    ];
    for (index, (style, thickness, height)) in styles.into_iter().enumerate() {
        builder.line(Line {
            bounds: [6.3, 6.2 + index as f32 * 15.0, 115.0, height],
            color: opaque(20, 20, 120),
            thickness,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            style,
            _pad: [0; 3],
        });
    }
    golden("lines", &canvas.draw(builder.finish()), EDGES);
}
