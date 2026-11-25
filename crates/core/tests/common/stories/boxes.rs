//! Rounded boxes with borders, a gradient fill, a shadow and lines.

use iceblink::{
    ClipRef, Color, Line, LineStyle, OrderEntry, Paint, PaintKind, ROUND, Shadow, SpaceRef,
};

use crate::common::stories::{background, key, ramp};
use crate::common::story::Story;
use crate::common::{props, quad};

/// Rounded boxes with borders, a gradient fill, a shadow and lines, then the box fill recoloured
/// and the chunk moved.
pub(crate) const BOXES: Story = Story {
    name: "boxes",
    steps: &[
        background,
        |tx, world| {
            let blue = world.solid(tx, [0.1, 0.3, 0.8, 1.0]);
            let border = world.solid(tx, [0.0, 0.0, 0.0, 1.0]);
            let gradient = tx.create_paint(ramp());
            world.paints.push(gradient);

            let mut chunk = tx.record(key(2), props(world.space));
            let fill = chunk.paint_ref(PaintKind::SOLID, blue);
            let stroke = chunk.paint_ref(PaintKind::SOLID, border);

            chunk.shadow(Shadow {
                shape_bounds: [10.0, 10.0, 50.0, 30.0],
                radii: [6.0; 8],
                caster_bounds: [10.0, 10.0, 50.0, 30.0],
                caster_radii: [6.0; 8],
                shapes: ROUND,
                color: Color([0.0, 0.0, 0.0, 0.4]),
                blur: 4.0,
                clip: ClipRef::INHERIT,
                space: SpaceRef::INHERIT,
                inset: 0,
            });
            let mut rounded = quad([10.0, 10.0, 50.0, 30.0], fill);
            rounded.radii = [6.0; 8];
            rounded.border = [2.0; 4];
            rounded.stroke = stroke;
            chunk.quad(rounded);

            let ramp = chunk.paint_ref(PaintKind::GRADIENT, gradient);
            let mut ramped = quad([10.0, 60.0, 60.0, 20.0], ramp);
            ramped.paint_origin = [10.0, 60.0];
            chunk.quad(ramped);

            for (row, style) in [LineStyle::SOLID, LineStyle::DASHED, LineStyle::WAVY]
                .into_iter()
                .enumerate()
            {
                chunk.line(Line {
                    bounds: [80.0, 20.0 + row as f32 * 12.0, 40.0, 4.0],
                    color: Color([0.8, 0.1, 0.1, 1.0]),
                    thickness: 1.0,
                    clip: ClipRef::INHERIT,
                    space: SpaceRef::INHERIT,
                    style,
                    _pad: [0; 3],
                });
            }

            world.append(tx, OrderEntry::Chunk(key(2)));
        },
        |tx, world| {
            tx.set_paint(world.paints[1], Paint::Solid(Color([0.1, 0.6, 0.2, 1.0])));
        },
        |tx, _| tx.set_offset(key(2), [6.0, 4.0]),
    ],
};
