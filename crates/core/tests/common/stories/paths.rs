//! Paths on the mask route and as frame paths.

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Circle, Shape, Stroke};
use iceblink::{ClipRef, OrderEntry, PaintKind, PathFill, PathItem, PathStroke, SpaceRef};

use crate::common::props;
use crate::common::stories::{background, key, ramp};
use crate::common::story::Story;

/// Returns a closed circle path.
fn circle(center: (f64, f64), radius: f64) -> Arc<BezPath> {
    Arc::new(Circle::new(center, radius).to_path(0.1))
}

/// A small solid circle, which takes the mask route, and a large stroked curve with a gradient
/// fill, which takes the path route, then both moved.
pub(crate) const PATHS: Story = Story {
    name: "paths",
    steps: &[
        background,
        |tx, world| {
            let green = world.solid(tx, [0.0, 0.6, 0.2, 1.0]);
            let black = world.solid(tx, [0.0, 0.0, 0.0, 1.0]);
            let ramp = tx.create_paint(ramp());
            world.paints.push(ramp);

            let mut chunk = tx.record(key(2), props(world.space));
            let fill = chunk.paint_ref(PaintKind::SOLID, green);
            let stroke = chunk.paint_ref(PaintKind::SOLID, black);
            let shaded = chunk.paint_ref(PaintKind::GRADIENT, ramp);
            chunk.path(PathItem {
                path: circle((24.0, 24.0), 12.0),
                placement: Affine::IDENTITY,
                brush: Affine::IDENTITY,
                fill: Some(PathFill {
                    paint: fill,
                    rule: iceblink::FillRule::NON_ZERO,
                }),
                stroke: None,
                clips: Arc::from(Vec::new()),
                clip: ClipRef::INHERIT,
                space: SpaceRef::INHERIT,
            });

            let mut wave = BezPath::new();
            wave.move_to((8.0, 100.0));
            wave.curve_to((40.0, 40.0), (80.0, 160.0), (120.0, 70.0));
            wave.line_to((120.0, 120.0));
            wave.close_path();
            chunk.path(PathItem {
                path: Arc::new(wave),
                placement: Affine::IDENTITY,
                brush: Affine::IDENTITY,
                fill: Some(PathFill {
                    paint: shaded,
                    rule: iceblink::FillRule::EVEN_ODD,
                }),
                stroke: Some(PathStroke {
                    paint: stroke,
                    style: Stroke::new(2.0),
                }),
                clips: Arc::from(Vec::new()),
                clip: ClipRef::INHERIT,
                space: SpaceRef::INHERIT,
            });

            world.append(tx, OrderEntry::Chunk(key(2)));
        },
        |tx, _| tx.set_offset(key(2), [10.0, -4.0]),
    ],
};
