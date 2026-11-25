//! Content in a clipped scroll port.

use iceblink::{ClipNode, ClipShape, OrderEntry, PaintKind, ROUND, ScrollNode, TransformNode};

use crate::common::stories::{background, key};
use crate::common::story::Story;
use crate::common::{identity, props, quad};

/// Content in a clipped scroll port, scrolled by whole pixels twice.
pub(crate) const SCROLL: Story = Story {
    name: "scroll",
    steps: &[
        background,
        |tx, world| {
            let row = world.solid(tx, [0.2, 0.2, 0.7, 1.0]);
            let scroll = tx.create_scroll(ScrollNode {
                space: world.space,
                port: [0.0, 0.0, 128.0, 96.0],
                content: [128.0, 400.0],
                offset: [0.0, 0.0],
                behaviour: None,
            });
            world.scrolls.push(scroll);
            let content = tx.create_transform(TransformNode {
                scroll: Some(scroll),
                ..identity(Some(world.space))
            });
            world.transforms.push(content);

            let port = tx.create_clip(ClipNode {
                parent: None,
                space: world.space,
                shape: ClipShape::RoundedRect {
                    rect: [0.0, 0.0, 128.0, 96.0],
                    radii: [0.0; 8],
                    shapes: ROUND,
                },
            });
            world.clips.push(port);

            let mut content_props = props(content);
            content_props.clip = Some(port);
            let mut chunk = tx.record(key(2), content_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, row);
            for index in 0..10 {
                chunk.quad(quad([8.0, index as f32 * 40.0 + 4.0, 112.0, 24.0], fill));
            }
            world.append(tx, OrderEntry::Chunk(key(2)));
        },
        |tx, world| tx.scroll_to(world.scrolls[0], [0.0, 30.0]),
        |tx, world| tx.scroll_to(world.scrolls[0], [0.0, 50.0]),
    ],
};
