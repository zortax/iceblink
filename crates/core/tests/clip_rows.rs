//! Rows of chunks drawn through a clip whose edges move, into a kept target on a device, compared
//! with the same pictures redrawn whole.

mod common;

use iceblink::{ChunkKey, ClipId, ClipNode, ClipShape, OrderEntry, PaintKind, ROUND};
use iceblink_testing::gpu::device_lock;

use common::twin::Twin;
use common::{props, quad};

/// Returns a clip node over `rect` in `space`, without radii.
fn square(space: iceblink::TransformId, rect: [f32; 4]) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

#[test]
fn rows_a_moving_clip_edge_passes_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let clip: ClipId = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        world.boxes(
            tx,
            ChunkKey::new(1, 0),
            white,
            &[[0.0, 0.0, 128.0, 128.0]],
            true,
        );
        let clip = tx.create_clip(square(world.space, [0.0, 0.0, 128.0, 100.0]));
        let paint = world.solid(tx, [0.2, 0.3, 0.7, 1.0]);
        for row in 0..10 {
            let key = ChunkKey::new(2, row);
            let mut row_props = props(world.space);
            row_props.clip = Some(clip);
            let mut chunk = tx.record(key, row_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, paint);
            chunk.quad(quad(
                [4.0 + row as f32, row as f32 * 12.0, 80.0, 10.5],
                fill,
            ));
            world.append(tx, OrderEntry::Chunk(key));
        }
        clip
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    let edges: [(&str, [f32; 4]); 7] = [
        ("the bottom edge up", [0.0, 0.0, 128.0, 95.0]),
        ("the bottom edge past two rows", [0.0, 0.0, 128.0, 70.25]),
        ("down over rows it cut away", [0.0, 0.0, 128.0, 128.0]),
        ("the top edge down", [0.0, 30.0, 128.0, 98.0]),
        ("the left edge in", [40.5, 30.0, 87.5, 98.0]),
        ("to nothing", [0.0, 0.0, 128.0, 0.0]),
        ("back", [0.0, 0.0, 128.0, 100.0]),
    ];
    for (name, rect) in edges {
        twin.commit(&mut (), |tx, world, _| {
            tx.set_clip(clip, square(world.space, rect));
        });
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "{name} differs: {found:?}");
    }
}
