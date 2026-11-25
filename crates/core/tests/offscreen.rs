//! Random scenes that reach past the output's edges build frames that keep the frame contract.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::kurbo::{Affine, Circle, Shape};
use iceblink::{
    BlendMode, ChunkKey, ClipRef, Color, EffectId, EffectNode, Filter, MarkFlags, MarkPayload,
    Marks, Matrix4, OrderEntry, OrderId, OrderNode, Paint, PaintId, PaintKind, PathFill, PathItem,
    SpaceRef, Transaction, TransformId,
};
use iceblink_testing::capture::CaptureRenderer;

use common::numbers::Numbers;
use common::slice::Slice;
use common::story::World;
use common::{FULL, identity, props, quad};

/// How many chunks a scene records.
const CHUNKS: u32 = 10;

/// The ids the random changes of a scene refer to.
struct Ids {
    spaces: [TransformId; 3],
    paints: [PaintId; 2],
    blur: EffectId,
    nodes: [OrderId; 2],
}

/// Returns a random coordinate on a quarter-pixel grid, between 64 pixels before the output and
/// 64 past it.
fn coordinate(numbers: &mut Numbers) -> f32 {
    -64.0 + numbers.below(1024) as f32 * 0.25
}

/// Returns an opaque or a translucent colour.
fn colour(numbers: &mut Numbers) -> Paint {
    let alpha = if numbers.below(2) == 0 { 1.0 } else { 0.5 };
    Paint::Solid(Color([0.2 * alpha, 0.4 * alpha, 0.8 * alpha, alpha]))
}

/// Records chunk `part` with a few quads, a filled circle and a union of two discs, in random
/// places.
fn record(tx: &mut Transaction, ids: &Ids, numbers: &mut Numbers, part: u32) {
    let space = ids.spaces[numbers.below(3) as usize];
    let mut chunk = tx.record(ChunkKey::new(10, part), props(space));
    let paint = ids.paints[numbers.below(2) as usize];
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    for _ in 0..1 + numbers.below(3) {
        let (x, y) = (coordinate(numbers), coordinate(numbers));
        let size = [
            8.0 + numbers.below(120) as f32,
            8.0 + numbers.below(120) as f32,
        ];
        chunk.quad(quad([x, y, size[0], size[1]], fill));
    }

    let center = (
        f64::from(coordinate(numbers)),
        f64::from(coordinate(numbers)),
    );
    let radius = 4.0 + numbers.below(60) as f64;
    chunk.path(PathItem {
        path: Arc::new(Circle::new(center, radius).to_path(0.1)),
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

    let disc = |numbers: &mut Numbers| [coordinate(numbers), coordinate(numbers), 12.0, 0.0];
    let payload = Arc::new(MarkPayload {
        discs: vec![disc(numbers), disc(numbers)],
        ..MarkPayload::default()
    });
    let payload = chunk.payload(payload);
    chunk.marks(Marks {
        bounds: [-64.0, -64.0, 256.0, 256.0],
        paint: fill,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 2,
        boxes: 0,
        vertices: 0,
        placements: 0,
        first: 0,
        payload,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::UNION,
    });
}

/// Builds a scene of an opaque backing, a list and a blurred group holding the chunks, in three
/// spaces.
fn scene(tx: &mut Transaction, world: &mut World, numbers: &mut Numbers) -> Ids {
    let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
    world.boxes(
        tx,
        ChunkKey::new(1, 0),
        white,
        &[[0.0, 0.0, 128.0, 128.0]],
        true,
    );

    let spaces = [(); 3].map(|_| tx.create_transform(identity(Some(world.space))));
    let paints = [(); 2].map(|_| tx.create_paint(colour(numbers)));
    let blur = tx.create_effect(EffectNode {
        clip: None,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![Filter::Blur(2.0)]),
        isolate: true,
    });
    let nodes = [
        tx.create_order(OrderNode::default()),
        tx.create_order(OrderNode { effect: Some(blur) }),
    ];
    for node in nodes {
        world.append(tx, OrderEntry::Node(node));
    }

    let ids = Ids {
        spaces,
        paints,
        blur,
        nodes,
    };

    let mut held = [0u32; 2];
    for part in 0..CHUNKS {
        record(tx, &ids, numbers, part);
        let node = numbers.below(2) as usize;
        let entry = OrderEntry::Chunk(ChunkKey::new(10, part));
        tx.splice(ids.nodes[node], held[node], 0, &[entry]);
        held[node] += 1;
    }
    ids
}

/// Writes one random change: a move of a space, partly or wholly off the output, a paint turned
/// opaque or translucent, a new blur deviation or a recording.
fn change(tx: &mut Transaction, ids: &Ids, numbers: &mut Numbers) {
    match numbers.below(6) {
        0..=2 => {
            let space = ids.spaces[numbers.below(3) as usize];
            let (x, y) = (2.0 * coordinate(numbers), 2.0 * coordinate(numbers));
            tx.set_transform(space, Matrix4::translation(x, y, 0.0));
        }
        3 => {
            let paint = ids.paints[numbers.below(2) as usize];
            tx.set_paint(paint, colour(numbers));
        }
        4 => tx.set_effect(
            ids.blur,
            EffectNode {
                clip: None,
                opacity: 1.0,
                blend: BlendMode::NORMAL,
                filters: Arc::from(vec![Filter::Blur(numbers.below(6) as f32)]),
                isolate: true,
            },
        ),
        _ => {
            let part = numbers.below(u64::from(CHUNKS)) as u32;
            record(tx, ids, numbers, part);
        }
    }
}

#[test]
fn frames_of_content_past_the_output_edges_keep_the_frame_contract() {
    for seed in 0..48 {
        let mut numbers = Numbers(seed * 6_700_417 + 3);
        let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
        slice.draw();

        let mut tx = slice.tx();
        let ids = scene(&mut tx, &mut slice.world, &mut numbers);
        slice.compositor.commit(tx).expect("the scene commits");
        // Every frame a slice draws is checked against the frame contract.
        slice.draw();

        for _ in 0..16 {
            let mut tx = slice.tx();
            for _ in 0..1 + numbers.below(3) {
                change(&mut tx, &ids, &mut numbers);
            }
            slice.compositor.commit(tx).expect("the change commits");
            slice.draw();
        }
    }
}
