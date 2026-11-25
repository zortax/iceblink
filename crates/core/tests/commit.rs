//! Commit rejection reasons and atomicity.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::kurbo::{self};
use iceblink::smallvec::smallvec;
use iceblink::{
    Anchor, AnyId, Built, ChunkKey, ClipNode, ClipShape, Color, EffectNode, Filter, Gradient,
    GradientShape, GradientStop, Interpolation, MarkPayload, Matrix4, OrderEntry, OrderNode, Paint,
    PaintKind, PathItem, PathStroke, ROUND, RejectReason, ScrollId, ScrollNode, StickyAnchor,
    Transaction, TransformNode,
};
use iceblink::{
    Animation, AnimationStart, AnimationTarget, Direction, Easing, FillMode, Keyframe, Keyframes,
    Timing,
};

use common::{Scene, identity, props, quad};

fn reject(scene: &mut Scene, tx: Transaction) -> RejectReason {
    scene
        .compositor
        .commit(tx)
        .expect_err("the transaction is rejected")
        .reason
}

fn clip_node(space: iceblink::TransformId) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 50.0, 50.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

#[test]
fn a_stale_ref_of_a_chunk_is_rejected() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color::TRANSPARENT));
    scene.commit(tx);
    let mut tx = scene.tx();
    tx.remove(paint);
    scene.commit(tx);

    let mut tx = scene.tx();
    let replacement = tx.create_paint(Paint::Solid(Color::TRANSPARENT));
    assert_eq!(replacement.slot(), paint.slot(), "the slot is minted again");
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 0.0, 4.0, 4.0], fill));
    assert_eq!(reject(&mut scene, tx), RejectReason::StaleId);
}

#[test]
fn creating_into_a_live_slot_is_stale() {
    let mut scene = Scene::new();
    // Freeing a live id by hand makes the space mint its slot again.
    scene.ids.free(&[AnyId::Order(scene.root)]);
    let mut tx = scene.tx();
    let clash = tx.create_order(OrderNode::default());
    assert_eq!(clash.slot(), scene.root.slot());
    assert_eq!(reject(&mut scene, tx), RejectReason::StaleId);
}

#[test]
fn removing_a_named_paint_is_in_use() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color::TRANSPARENT));
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 0.0, 4.0, 4.0], fill));
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.remove(paint);
    assert_eq!(reject(&mut scene, tx), RejectReason::InUse);
}

#[test]
fn an_effect_is_in_use_while_an_order_node_names_it() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: iceblink::BlendMode::NORMAL,
        filters: Arc::from(Vec::<Filter>::new()),
        isolate: false,
    });
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(node)]);
    scene.commit(tx);

    // Order nodes without effects change in the same commits.
    let mut tx = scene.tx();
    tx.create_order(OrderNode::default());
    tx.remove(effect);
    assert_eq!(reject(&mut scene, tx), RejectReason::InUse);

    let mut tx = scene.tx();
    tx.create_order(OrderNode::default());
    tx.set_order_effect(node, None);
    tx.remove(effect);
    scene.commit(tx);
}

#[test]
fn removing_a_chunk_and_its_clip_together_is_legal() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let clip = tx.create_clip(clip_node(scene.space));
    let mut chunk_props = props(scene.space);
    chunk_props.clip = Some(clip);
    let mut chunk = tx.record(ChunkKey::new(1, 0), chunk_props);
    chunk.quad(quad([0.0, 0.0, 4.0, 4.0], iceblink::PaintRef::NONE));
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.remove(clip);
    tx.remove_chunk(ChunkKey::new(1, 0));
    scene.commit(tx);
}

#[test]
fn an_order_node_in_two_places_conflicts() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let child = tx.create_order(OrderNode::default());
    tx.splice(
        scene.root,
        0,
        0,
        &[OrderEntry::Node(child), OrderEntry::Node(child)],
    );
    assert_eq!(reject(&mut scene, tx), RejectReason::OrderConflict);
}

#[test]
fn a_chunk_key_in_two_places_conflicts() {
    let mut scene = Scene::new();
    let key = OrderEntry::Chunk(ChunkKey::new(9, 9));
    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 0, &[key, key]);
    assert_eq!(reject(&mut scene, tx), RejectReason::OrderConflict);
}

#[test]
fn an_order_node_inside_itself_is_a_cycle() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let child = tx.create_order(OrderNode::default());
    tx.splice(child, 0, 0, &[OrderEntry::Node(child)]);
    assert_eq!(reject(&mut scene, tx), RejectReason::Cycle);
}

#[test]
fn order_nodes_made_inside_each_other_are_a_cycle() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let a = tx.create_order(OrderNode::default());
    let b = tx.create_order(OrderNode::default());
    let leaf = tx.create_order(OrderNode::default());
    tx.splice(b, 0, 0, &[OrderEntry::Node(leaf)]);
    tx.splice(a, 0, 0, &[OrderEntry::Node(b)]);
    tx.splice(b, 1, 0, &[OrderEntry::Node(a)]);
    assert_eq!(reject(&mut scene, tx), RejectReason::Cycle);
}

#[test]
fn an_output_root_spliced_into_a_sequence_conflicts() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let holder = tx.create_order(OrderNode::default());
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(holder)]);
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.splice(holder, 0, 0, &[OrderEntry::Node(scene.root)]);
    assert_eq!(reject(&mut scene, tx), RejectReason::OrderConflict);
}

#[test]
fn moving_an_order_node_within_one_transaction_is_legal() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let a = tx.create_order(OrderNode::default());
    let b = tx.create_order(OrderNode::default());
    let child = tx.create_order(OrderNode::default());
    tx.splice(
        scene.root,
        0,
        0,
        &[OrderEntry::Node(a), OrderEntry::Node(b)],
    );
    tx.splice(a, 0, 0, &[OrderEntry::Node(child)]);
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.splice(b, 0, 0, &[OrderEntry::Node(child)]);
    tx.splice(a, 0, 1, &[]);
    scene.commit(tx);
}

#[test]
fn a_dead_slot_in_an_item_is_invalid() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let mut item = quad([0.0, 0.0, 4.0, 4.0], iceblink::PaintRef::NONE);
    item.space = iceblink::SpaceRef(77);
    chunk.quad(item);
    assert_eq!(reject(&mut scene, tx), RejectReason::InvalidItem);
}

#[test]
fn an_animation_whose_keyframes_do_not_fit_its_target_is_invalid() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let transform = tx.create_transform(identity(None));
    tx.animate(iceblink::Animation {
        target: iceblink::AnimationTarget::Transform(transform),
        keyframes: iceblink::Keyframes::Scalar(Default::default()),
        timing: iceblink::Timing {
            duration_ns: 1,
            delay_ns: 0,
            iterations: 1.0,
            direction: iceblink::Direction::Normal,
            fill: iceblink::FillMode::None,
            easing: iceblink::Easing::Linear,
        },
        start: iceblink::AnimationStart::Next,
    });
    assert_eq!(reject(&mut scene, tx), RejectReason::InvalidItem);
}

#[test]
fn a_rejected_commit_changes_nothing() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    assert_eq!(scene.draw(), Built::Frame);
    let counters = *scene.compositor.counters();

    let mut tx = scene.tx();
    let order = tx.create_order(OrderNode::default());
    tx.remove_chunk(key);
    tx.set_offset(ChunkKey::new(7, 7), [1.0, 1.0]);
    assert_eq!(reject(&mut scene, tx), RejectReason::UnknownChunk);

    assert_eq!(
        scene.build(),
        Built::Idle { wake_ns: None },
        "no damage was added"
    );
    assert_eq!(scene.compositor.counters().commit, counters.commit);
    let mut tx = scene.tx();
    let again = tx.create_order(OrderNode::default());
    assert_eq!(again.slot(), order.slot());
    assert_eq!(again.generation(), order.generation() + 1);
    tx.set_offset(key, [3.0, 0.0]);
    scene.commit(tx);
}

#[test]
fn a_rejected_commit_leaves_the_next_commit_its_own_tier_changes() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let animated = tx.create_transform(identity(None));
    let written = tx.create_transform(identity(None));
    tx.animate(Animation {
        target: AnimationTarget::Transform(animated),
        keyframes: Keyframes::Matrix(smallvec![
            Keyframe {
                offset: 0.0,
                value: Matrix4::IDENTITY,
                easing: Easing::Linear,
            },
            Keyframe {
                offset: 1.0,
                value: Matrix4::translation(10.0, 0.0, 0.0),
                easing: Easing::Linear,
            },
        ]),
        timing: Timing {
            duration_ns: 1_000_000_000,
            delay_ns: 0,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::Forwards,
            easing: Easing::Linear,
        },
        start: AnimationStart::At(0),
    });
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.set_transform(written, Matrix4::IDENTITY);
    tx.set_transform(written, Matrix4::IDENTITY);
    tx.set_offset(ChunkKey::new(7, 7), [1.0, 1.0]);
    assert_eq!(reject(&mut scene, tx), RejectReason::UnknownChunk);

    let mut tx = scene.tx();
    tx.create_paint(Paint::Solid(Color::TRANSPARENT));
    let report = scene
        .compositor
        .commit(tx)
        .expect("the next commit applies");
    assert!(report.animations.is_empty());
}

#[test]
fn a_rejected_commit_puts_back_what_its_splices_removed() {
    let mut scene = Scene::new();
    let first = OrderEntry::Chunk(ChunkKey::new(1, 0));
    let second = OrderEntry::Chunk(ChunkKey::new(2, 0));
    let mut tx = scene.tx();
    let node = tx.create_order(OrderNode::default());
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(node)]);
    tx.splice(node, 0, 0, &[first, second]);
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.splice(node, 0, 1, &[]);
    tx.splice(node, 1, 0, &[first]);
    tx.splice(node, 0, 2, &[]);
    tx.set_offset(ChunkKey::new(7, 7), [1.0, 1.0]);
    assert_eq!(reject(&mut scene, tx), RejectReason::UnknownChunk);

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 0, &[first]);
    assert_eq!(
        reject(&mut scene, tx),
        RejectReason::OrderConflict,
        "the node holds the first key again"
    );
    let mut tx = scene.tx();
    tx.splice(node, 0, 2, &[]);
    scene.commit(tx);
}

#[test]
fn a_rejected_commit_puts_back_the_order_nodes_it_removed() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: iceblink::BlendMode::NORMAL,
        filters: Arc::from(Vec::<Filter>::new()),
        isolate: false,
    });
    let first = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let second = tx.create_order(OrderNode::default());
    tx.splice(first, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    let keys = [ChunkKey::new(2, 0), ChunkKey::new(2, 1)].map(OrderEntry::Chunk);
    tx.splice(second, 0, 0, &keys);
    tx.splice(
        scene.root,
        0,
        0,
        &[OrderEntry::Node(first), OrderEntry::Node(second)],
    );
    scene.commit(tx);

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 2, &[]);
    tx.remove(AnyId::Order(first));
    tx.remove(AnyId::Order(second));
    tx.splice(scene.root, 1, 0, &[]);
    assert_eq!(reject(&mut scene, tx), RejectReason::SpliceOutOfRange);

    // Each node is back with its own effect and entries.
    let mut tx = scene.tx();
    tx.set_order_effect(first, None);
    tx.remove(effect);
    tx.splice(second, 0, 2, &[]);
    scene.commit(tx);
}

#[test]
fn removed_ids_are_freed_with_the_next_generation() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let order = tx.create_order(OrderNode::default());
    scene.commit(tx);
    let mut tx = scene.tx();
    tx.remove(AnyId::Order(order));
    scene.commit(tx);

    let mut tx = scene.tx();
    let again = tx.create_order(OrderNode::default());
    assert_eq!(
        (again.slot(), again.generation()),
        (order.slot(), order.generation() + 1)
    );
}

/// Returns a marks item over a 50 pixel square drawing `counts` prims of payload 0, as
/// `[discs, vertices, placements]`, from `first`.
fn marks(counts: [u32; 3], first: u32) -> iceblink::Marks {
    iceblink::Marks {
        bounds: [0.0, 0.0, 50.0, 50.0],
        paint: iceblink::PaintRef::NONE,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 1.0,
        discs: counts[0],
        boxes: 0,
        vertices: counts[1],
        placements: counts[2],
        first,
        payload: 0,
        clip: iceblink::ClipRef::INHERIT,
        space: iceblink::SpaceRef::INHERIT,
        flags: iceblink::MarkFlags::default(),
    }
}

#[test]
fn non_finite_numbers_in_commands_are_invalid() {
    type Write = Box<dyn Fn(&mut Transaction, &Scene)>;
    let line = |points: [f64; 4]| {
        let mut path = kurbo::BezPath::new();
        path.move_to((points[0], points[1]));
        path.line_to((points[2], points[3]));
        Arc::new(path)
    };
    let path_item = move |path: Arc<kurbo::BezPath>, width: f64| PathItem {
        path,
        placement: kurbo::Affine::IDENTITY,
        brush: kurbo::Affine::IDENTITY,
        fill: None,
        stroke: Some(PathStroke {
            paint: iceblink::PaintRef::NONE,
            style: kurbo::Stroke::new(width),
        }),
        clips: Arc::from(Vec::new()),
        clip: iceblink::ClipRef::INHERIT,
        space: iceblink::SpaceRef::INHERIT,
    };
    let record = move |payload: MarkPayload, counts: [u32; 3]| -> Write {
        let payload = Arc::new(payload);
        Box::new(move |tx: &mut Transaction, scene: &Scene| {
            let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
            chunk.payload(Arc::clone(&payload));
            chunk.marks(marks(counts, 0));
        })
    };
    let stop = |offset: f32| GradientStop::new(offset, iceblink::color::Color::TRANSPARENT);
    let gradient = move |offset: f32| {
        Paint::Gradient(Gradient {
            shape: GradientShape::Linear {
                start: [0.0, 0.0],
                end: [1.0, 0.0],
            },
            stops: Arc::from(vec![stop(0.0), stop(offset)]),
            interpolation: Interpolation::default(),
            repeat: false,
        })
    };
    let sticky = |scroll: ScrollId, inset: f32| TransformNode {
        anchor: Anchor::Sticky(StickyAnchor {
            scroll,
            rect: [0.0; 4],
            limit: [0.0; 4],
            inset: [inset, f32::NAN, f32::NAN, f32::NAN],
        }),
        ..identity(None)
    };
    let separated = MarkPayload {
        vertices: vec![
            [f32::NAN, 0.0],
            [1.0, 1.0],
            [5.0, 5.0],
            [f32::NAN, f32::NAN],
        ],
        ..MarkPayload::default()
    };

    let cases: Vec<(&str, Write, Option<RejectReason>)> = vec![
        (
            "a stroke width",
            Box::new(move |tx, scene| {
                let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
                chunk.path(path_item(line([0.0, 0.0, 4.0, 4.0]), f64::INFINITY));
            }),
            Some(RejectReason::InvalidItem),
        ),
        (
            "a disc a marks item draws",
            record(
                MarkPayload {
                    discs: vec![[f32::NAN, 0.0, 1.0, 0.0]],
                    ..MarkPayload::default()
                },
                [1, 0, 0],
            ),
            Some(RejectReason::InvalidItem),
        ),
        (
            "a polyline with separators",
            record(separated.clone(), [0, 4, 0]),
            None,
        ),
        (
            "a vertex with an infinite y",
            record(
                MarkPayload {
                    vertices: vec![[f32::NAN, 0.0], [1.0, f32::INFINITY], [f32::NAN, 0.0]],
                    ..MarkPayload::default()
                },
                [0, 3, 0],
            ),
            Some(RejectReason::InvalidItem),
        ),
        (
            "a gradient stop",
            Box::new(move |tx, _| {
                tx.create_paint(gradient(f32::NAN));
            }),
            Some(RejectReason::InvalidItem),
        ),
        (
            "a transform matrix",
            Box::new(|tx, scene| {
                let mut local = Matrix4::IDENTITY;
                local.columns[3][0] = f32::INFINITY;
                tx.set_transform(scene.space, local);
            }),
            Some(RejectReason::InvalidItem),
        ),
        (
            "a NaN sticky inset",
            Box::new(move |tx, scene| {
                let scroll = tx.create_scroll(scroll_node(scene.space));
                tx.create_transform(sticky(scroll, f32::NAN));
            }),
            None,
        ),
        (
            "an infinite sticky inset",
            Box::new(move |tx, scene| {
                let scroll = tx.create_scroll(scroll_node(scene.space));
                tx.create_transform(sticky(scroll, f32::INFINITY));
            }),
            Some(RejectReason::InvalidItem),
        ),
        (
            "a NaN snap axis",
            Box::new(|tx, scene| {
                tx.create_scroll(ScrollNode {
                    behaviour: Some(iceblink::ScrollBehaviour {
                        physics: iceblink::ScrollPhysics::Kinetic,
                        snap: Arc::from(vec![iceblink::SnapPoint {
                            offset: [f32::NAN, 100.0],
                        }]),
                        snap_kind: iceblink::SnapKind::Mandatory,
                        bounce: iceblink::Sources::NONE,
                        user_scrollable: [true; 2],
                        chain: iceblink::Sources::NONE,
                    }),
                    ..scroll_node(scene.space)
                });
            }),
            None,
        ),
        (
            "a scroll offset",
            Box::new(|tx, scene| {
                let scroll = tx.create_scroll(scroll_node(scene.space));
                tx.scroll_to(scroll, [f32::NAN, 0.0]);
            }),
            Some(RejectReason::InvalidItem),
        ),
    ];

    for (name, write, expected) in cases {
        let mut scene = Scene::new();
        let mut tx = scene.tx();
        write(&mut tx, &scene);
        let result = scene
            .compositor
            .commit(tx)
            .map(|_| ())
            .map_err(|error| error.reason);
        assert_eq!(result.err(), expected, "{name}");
    }
}

fn scroll_node(space: iceblink::TransformId) -> ScrollNode {
    ScrollNode {
        space,
        port: [0.0, 0.0, 50.0, 50.0],
        content: [50.0, 500.0],
        offset: [0.0, 0.0],
        behaviour: None,
    }
}
