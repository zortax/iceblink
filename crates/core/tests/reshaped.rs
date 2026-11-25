//! A bordered rounded quad whose shape changes, drawn into a kept target on a device, against
//! the same pictures redrawn whole.

mod common;

use iceblink::{ChunkKey, OrderEntry, PaintRef, Quad};
use iceblink_testing::gpu::device_lock;

use common::quad;
use common::twin::Twin;

/// The shapes the quad takes in turn: bounds, radii and border widths.
const SHAPES: [(&str, [f32; 4], f32, f32); 10] = [
    ("the right edge out", [10.0, 10.0, 88.0, 60.0], 8.0, 3.0),
    ("by a fraction", [10.0, 10.0, 88.5, 60.25], 8.0, 3.0),
    ("the top edge down", [10.0, 17.5, 88.5, 52.75], 8.0, 3.0),
    ("rounder", [10.0, 17.5, 88.5, 52.75], 14.0, 3.0),
    ("a wider border", [10.0, 17.5, 88.5, 52.75], 14.0, 6.5),
    ("borderless", [10.0, 17.5, 88.5, 52.75], 14.0, 0.0),
    ("both sides out", [6.0, 17.5, 96.5, 52.75], 14.0, 0.0),
    ("every edge out", [4.5, 15.25, 99.75, 58.5], 14.0, 0.0),
    ("square and borderless", [12.0, 4.0, 100.0, 90.0], 0.0, 0.0),
    ("back", [10.0, 10.0, 80.0, 60.0], 8.0, 3.0),
];

/// Returns a quad over `bounds` with every radius `radius` and every border `border` wide.
fn shaped(bounds: [f32; 4], radius: f32, border: f32, fill: PaintRef, stroke: PaintRef) -> Quad {
    Quad {
        radii: [radius; 8],
        border: [border; 4],
        stroke,
        ..quad(bounds, fill)
    }
}

#[test]
fn a_quad_that_changes_its_shape_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let key = ChunkKey::new(2, 0);
    let paints = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        world.boxes(
            tx,
            ChunkKey::new(1, 0),
            white,
            &[[0.0, 0.0, 128.0, 128.0]],
            true,
        );
        let fill = PaintRef::color([40, 90, 200, 255]);
        let stroke = PaintRef::color([200, 40, 40, 255]);
        let mut chunk = tx.record(key, common::props(world.space));
        chunk.quad(shaped([10.0, 10.0, 80.0, 60.0], 8.0, 3.0, fill, stroke));
        world.append(tx, OrderEntry::Chunk(key));
        (fill, stroke)
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    let (fill, stroke) = paints;
    for (name, bounds, radius, border) in SHAPES {
        twin.commit(&mut (), |tx, world, _| {
            let mut chunk = tx.record(key, common::props(world.space));
            chunk.quad(shaped(bounds, radius, border, fill, stroke));
        });
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "{name} differs: {found:?}");
    }
}
