//! Text on the atlas and path routes.

use iceblink::{OrderEntry, PaintKind};

use crate::common::props;
use crate::common::stories::{background, key};
use crate::common::story::Story;
use crate::common::text::run;

/// Text through the glyph rasteriser: a small run on the atlas route and a large one on the path
/// route, then the small run edited and moved.
pub(crate) const TEXT: Story = Story {
    name: "text",
    steps: &[
        background,
        |tx, world| {
            let ink = world.solid(tx, [0.0, 0.0, 0.0, 1.0]);
            let font = world.font;
            let mut chunk = tx.record(key(2), props(world.space));
            let paint = chunk.paint_ref(PaintKind::SOLID, ink);
            run(&mut chunk, font, "Hello", 16.0, [8.0, 24.0], paint);
            world.append(tx, OrderEntry::Chunk(key(2)));

            let red = world.solid(tx, [0.7, 0.0, 0.0, 1.0]);
            let mut chunk = tx.record(key(3), props(world.space));
            let paint = chunk.paint_ref(PaintKind::SOLID, red);
            run(&mut chunk, font, "Ag", 300.0, [4.0, 120.0], paint);
            world.append(tx, OrderEntry::Chunk(key(3)));
        },
        |tx, world| {
            let font = world.font;
            let mut chunk = tx.record(key(2), props(world.space));
            let paint = chunk.paint_ref(PaintKind::SOLID, world.paints[1]);
            run(&mut chunk, font, "Hello, slice", 16.0, [8.0, 24.0], paint);
        },
        |tx, _| tx.set_offset(key(2), [0.5, 10.0]),
    ],
};
