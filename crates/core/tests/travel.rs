//! Inks of content that scroll nodes carry, kept equal to a rebuild through scrolls and edits.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::{
    Anchor, BlendMode, ChunkKey, ClipId, ClipNode, ClipShape, EffectId, EffectNode, Filter,
    LocalClip, LocalParent, Matrix4, OrderEntry, OrderNode, PaintId, PaintKind, ROUND, ScrollId,
    ScrollNode, StickyAnchor, Transaction, TransformId, TransformNode,
};
use iceblink_testing::capture::CaptureRenderer;

use common::numbers::Numbers;
use common::slice::Slice;
use common::{FULL, identity, props, quad, stat};

/// The nodes a scene of scrolled rows is drawn under, and the paint and number of its rows.
struct Rows {
    scroll: ScrollId,
    inner: ScrollId,
    /// The scroll node a fixed space inside the content applies.
    pinned: ScrollId,
    content: TransformId,
    space: TransformId,
    port: ClipId,
    /// The effect of the faded group.
    fade: EffectId,
    paint: PaintId,
    count: u32,
}

/// Returns a clip node over `rect` in `space`, without radii.
fn rect_clip(space: TransformId, rect: [f32; 4], parent: Option<ClipId>) -> ClipNode {
    ClipNode {
        parent,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

/// Records rows in a clipped scroll port over an opaque backing, and returns the ids `change`
/// writes.
///
/// Each row is drawn in a random space, through a random clip, under a random order node:
///
/// - The space is the scrolled content, a nested scroll port, a sticky or a fixed space, or a
///   fixed space that applies a scroll node of its own.
/// - The clip is the port clip, a clip of its own, no clip or a clip with fractional edges.
/// - The node is a list, a blurred group or a faded group clipped by the port, or a node inside
///   one of them.
fn scene(tx: &mut Transaction, world: &mut common::story::World, numbers: &mut Numbers) -> Rows {
    let space = world.space;
    let backing = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
    let paint = world.solid(tx, [0.2, 0.3, 0.8, 0.9]);
    world.boxes(
        tx,
        ChunkKey::new(1, 0),
        backing,
        &[[0.0, 0.0, 128.0, 128.0]],
        true,
    );
    let scroll = tx.create_scroll(ScrollNode {
        space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [200.0, 1000.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(space))
    });
    let port = tx.create_clip(rect_clip(space, [0.0, 0.0, 100.0, 100.0], None));
    let inner_port = tx.create_clip(rect_clip(content, [10.0, 300.0, 60.0, 60.0], Some(port)));
    let inner = tx.create_scroll(ScrollNode {
        space: content,
        port: [10.0, 300.0, 60.0, 60.0],
        content: [60.0, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let nested = tx.create_transform(TransformNode {
        scroll: Some(inner),
        ..identity(Some(content))
    });
    let sticky = tx.create_transform(TransformNode {
        anchor: Anchor::Sticky(StickyAnchor {
            scroll,
            rect: [0.0, 40.0, 100.0, 10.0],
            limit: [0.0, 0.0, 100.0, 1000.0],
            inset: [0.0, f32::NAN, f32::NAN, f32::NAN],
        }),
        ..identity(Some(content))
    });
    let fixed = tx.create_transform(TransformNode {
        anchor: Anchor::Fixed,
        ..identity(Some(content))
    });
    let pinned = tx.create_scroll(ScrollNode {
        space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, 800.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let fixed_scrolled = tx.create_transform(TransformNode {
        anchor: Anchor::Fixed,
        scroll: Some(pinned),
        ..identity(Some(content))
    });
    let own = tx.create_clip(rect_clip(content, [0.0, 0.0, 90.0, 1000.0], Some(port)));
    let fractional = tx.create_clip(rect_clip(space, [0.0, 0.0, 100.5, 100.0], None));

    let list = tx.create_order(OrderNode::default());
    let node = tx.create_order(OrderNode::default());
    let grouped_node = tx.create_order(OrderNode::default());
    let blur = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![Filter::Blur(2.0)]),
        isolate: true,
    });
    let group = tx.create_order(OrderNode { effect: Some(blur) });
    let fade = tx.create_effect(EffectNode {
        clip: Some(port),
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
        isolate: false,
    });
    let faded = tx.create_order(OrderNode { effect: Some(fade) });
    let faded_node = tx.create_order(OrderNode::default());

    let spaces = [
        content,
        content,
        content,
        nested,
        sticky,
        fixed,
        content,
        fixed_scrolled,
    ];
    let clips = [
        Some(port),
        Some(own),
        None,
        Some(inner_port),
        Some(port),
        Some(port),
        Some(fractional),
        Some(port),
    ];
    let count = 30 + numbers.below(30) as u32;
    let mut entries: [Vec<OrderEntry>; 6] = Default::default();
    for row in 0..count {
        let kind = numbers.below(spaces.len() as u64) as usize;
        let key = ChunkKey::new(10, row);
        let mut row_props = props(spaces[kind]);
        row_props.clip = clips[kind];
        let mut chunk = tx.record(key, row_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let y = row as f32 * 13.0 + numbers.below(4) as f32 * 0.25;
        chunk.quad(quad([4.0, y, 50.0 + numbers.below(40) as f32, 7.0], fill));
        if numbers.below(3) == 0 {
            let local = chunk.local_clip(LocalClip {
                parent: LocalParent::Chunk,
                shape: ClipShape::RoundedRect {
                    rect: [0.0, y, 30.0, 4.0],
                    radii: [0.0; 8],
                    shapes: ROUND,
                },
            });
            let mut clipped = quad([10.0, y, 40.0, 7.0], fill);
            clipped.clip = local;
            chunk.quad(clipped);
        }
        let parent = match numbers.below(10) {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 5,
            _ => 0,
        };
        entries[parent].push(OrderEntry::Chunk(key));
    }
    let [
        mut rows,
        inside,
        mut grouped,
        grouped_inside,
        mut fading,
        faded_inside,
    ] = entries;
    tx.splice(node, 0, 0, &inside);
    tx.splice(faded_node, 0, 0, &faded_inside);
    fading.push(OrderEntry::Node(faded_node));
    tx.splice(faded, 0, 0, &fading);
    rows.push(OrderEntry::Node(faded));
    tx.splice(grouped_node, 0, 0, &grouped_inside);
    grouped.push(OrderEntry::Node(grouped_node));
    tx.splice(group, 0, 0, &grouped);
    rows.push(OrderEntry::Node(node));
    tx.splice(list, 0, 0, &rows);
    world.append(tx, OrderEntry::Node(list));
    world.append(tx, OrderEntry::Node(group));
    Rows {
        scroll,
        inner,
        pinned,
        content,
        space,
        port,
        fade,
        paint,
        count,
    }
}

/// Adds a random change to `tx`.
///
/// Most changes are scrolls of whole or half pixels, some with the fixed space's scroll. The
/// others are an inner scroll, a chunk offset, a recording, a move of the content, a new effect
/// for the faded group or a new port.
fn change(tx: &mut Transaction, rows: &Rows, numbers: &mut Numbers, offset: &mut f32) {
    let row = numbers.below(u64::from(rows.count)) as u32;
    match numbers.below(11) {
        0..=4 => {
            let half = if numbers.below(5) == 0 { 0.5 } else { 0.0 };
            *offset = (*offset + numbers.below(30) as f32 - 10.0 + half).clamp(0.0, 900.0);
            tx.scroll_to(rows.scroll, [0.0, *offset]);
            if numbers.below(2) == 0 {
                tx.scroll_to(rows.pinned, [0.0, numbers.below(70) as f32 * 10.0]);
            }
        }
        5 => tx.scroll_to(rows.inner, [0.0, numbers.below(300) as f32]),
        6 => tx.set_offset(ChunkKey::new(10, row), [numbers.below(3) as f32, 0.0]),
        7 => {
            let mut row_props = props(rows.content);
            row_props.clip = Some(rows.port);
            let mut chunk = tx.record(ChunkKey::new(10, row), row_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, rows.paint);
            let width = 30.0 + numbers.below(50) as f32;
            chunk.quad(quad([4.0, row as f32 * 13.0, width, 7.0], fill));
        }
        8 => {
            let x = numbers.below(3) as f32;
            tx.set_transform(rows.content, Matrix4::translation(x, 0.0, 0.0));
        }
        9 => {
            let (clip, filters) = match numbers.below(3) {
                0 => (None, Vec::new()),
                1 => (Some(rows.port), vec![Filter::Blur(1.0)]),
                _ => (Some(rows.port), Vec::new()),
            };
            tx.set_effect(
                rows.fade,
                EffectNode {
                    clip,
                    opacity: 0.5,
                    blend: BlendMode::NORMAL,
                    filters: Arc::from(filters),
                    isolate: false,
                },
            );
        }
        _ => {
            let height = 90.0 + numbers.below(10) as f32;
            tx.set_clip(
                rows.port,
                rect_clip(rows.space, [0.0, 0.0, 100.0, height], None),
            );
        }
    }
}

#[test]
fn random_scrolls_and_edits_keep_every_ink_equal_to_a_rebuild() {
    for seed in 0..24 {
        let mut numbers = Numbers(seed * 7919 + 13);
        let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
        slice.draw();

        let mut tx = slice.tx();
        let rows = scene(&mut tx, &mut slice.world, &mut numbers);
        slice.compositor.commit(tx).expect("the rows commit");
        slice.draw();

        let mut offset = 0.0;
        for step in 0..30 {
            let mut tx = slice.tx();
            for _ in 0..1 + numbers.below(3) {
                change(&mut tx, &rows, &mut numbers, &mut offset);
            }
            slice.compositor.commit(tx).expect("the change commits");
            slice.draw();
            let mismatches = stat(&slice.compositor, "verification_mismatches");
            assert_eq!(mismatches, 0, "seed {seed}, step {step}");
        }
    }
}
