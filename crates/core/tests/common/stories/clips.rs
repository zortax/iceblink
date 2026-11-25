//! Content drawn through nested clips whose edges and corners change.

use iceblink::{ClipNode, ClipShape, OrderEntry, PaintKind, Quad, ROUND, TransformId};

use crate::common::stories::{background, key};
use crate::common::story::Story;
use crate::common::{props, quad};

/// Returns a clip node under `parent` over `rect` with `radii`.
fn rounded(
    space: TransformId,
    parent: Option<iceblink::ClipId>,
    rect: [f32; 4],
    radii: [f32; 8],
) -> ClipNode {
    ClipNode {
        parent,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii,
            shapes: ROUND,
        },
    }
}

/// Quads drawn through a square clip inside a rounded one, then the rounded clip's edge moved,
/// its top left corner enlarged, and the square clip narrowed.
pub(crate) const CLIPS: Story = Story {
    name: "clips",
    steps: &[
        background,
        |tx, world| {
            let space = world.space;
            let outer = tx.create_clip(rounded(space, None, [16.0, 16.0, 96.0, 96.0], [10.0; 8]));
            let inner = tx.create_clip(rounded(
                space,
                Some(outer),
                [8.0, 30.0, 112.0, 60.0],
                [0.0; 8],
            ));
            world.clips.extend([outer, inner]);

            let teal = world.solid(tx, [0.1, 0.6, 0.6, 1.0]);
            let violet = world.solid(tx, [0.4, 0.1, 0.5, 0.7]);
            let mut chunk_props = props(space);
            chunk_props.clip = Some(inner);
            let mut chunk = tx.record(key(2), chunk_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, teal);
            chunk.quad(quad([0.0, 0.0, 128.0, 128.0], fill));
            let fill = chunk.paint_ref(PaintKind::SOLID, violet);
            chunk.quad(Quad {
                radii: [14.0; 8],
                ..quad([30.25, 20.5, 60.0, 80.0], fill)
            });
            world.append(tx, OrderEntry::Chunk(key(2)));
        },
        |tx, world| {
            let node = rounded(world.space, None, [16.0, 16.0, 96.0, 70.5], [10.0; 8]);
            tx.set_clip(world.clips[0], node);
        },
        |tx, world| {
            let mut radii = [10.0; 8];
            radii[0] = 30.0;
            radii[1] = 30.0;
            let node = rounded(world.space, None, [16.0, 16.0, 96.0, 70.5], radii);
            tx.set_clip(world.clips[0], node);
        },
        |tx, world| {
            let node = rounded(
                world.space,
                Some(world.clips[0]),
                [24.0, 30.0, 80.0, 60.0],
                [0.0; 8],
            );
            tx.set_clip(world.clips[1], node);
        },
    ],
};
