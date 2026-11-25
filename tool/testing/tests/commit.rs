//! Which transactions a commit rejects, and that a rejected commit changes nothing.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::geom::{Point, Size};
use iceblink::{
    Animation, AnimationStart, AnimationTarget, ChunkKey, ClipNode, ClipShape, DeviceRect,
    Direction, Easing, EffectNode, FillMode, IdSpace, ImageDesc, ImageFormat, ImageWrite,
    Keyframes, OrderEntry, OrderNode, Paint, PaintKind, RejectReason, ShaderDesc, ShaderMode,
    ShaderQuad, ShaderReads, ShaderSource, Timing, Transaction,
};

use crate::common::naive::{Harness, quad, root_node};

/// Commits `transaction`, which must be rejected, and returns the reason.
fn rejected(harness: &mut Harness, transaction: Transaction) -> RejectReason {
    harness
        .compositor
        .commit(transaction)
        .expect_err("the transaction is rejected")
        .reason
}

#[test]
fn a_rejected_commit_changes_nothing_and_frees_the_ids_it_minted() {
    let mut harness = Harness::new();
    assert!(harness.frame());

    let mut transaction = harness.transaction();
    let minted = transaction.create_order(OrderNode::default());
    let props = harness.props();
    transaction
        .record(ChunkKey::new(1, 0), props)
        .quad(quad([0.0, 0.0, 8.0, 8.0], [9; 4]));
    harness.append(&mut transaction, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    transaction.set_offset(ChunkKey::new(7, 0), [1.0, 1.0]);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::UnknownChunk
    );

    assert!(!harness.frame(), "nothing of the rejected commit applied");
    let next = harness.transaction().create_order(OrderNode::default());
    assert_eq!(next.slot(), minted.slot());
    assert_eq!(next.generation(), minted.generation() + 1);
}

#[test]
fn a_stale_or_foreign_id_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let order = transaction.create_order(OrderNode::default());
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.remove(order);
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.set_order_effect(order, None);
    assert_eq!(rejected(&mut harness, transaction), RejectReason::StaleId);

    let mut foreign = Transaction::new(IdSpace::new(harness.client));
    foreign.create_order(OrderNode::default());
    assert_eq!(rejected(&mut harness, foreign), RejectReason::StaleId);
}

#[test]
fn removing_what_a_chunk_still_names_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let paint = transaction.create_paint(Paint::Solid(iceblink::Color([1.0; 4])));
    let props = harness.props();
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);
    let mut item = quad([0.0, 0.0, 8.0, 8.0], [9; 4]);
    item.fill = recorder.paint_ref(PaintKind::SOLID, paint);
    recorder.quad(item);
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.remove(paint);
    let error = harness.compositor.commit(transaction).unwrap_err();
    assert_eq!(error.reason, RejectReason::InUse);
    assert_eq!(error.command, Some(0));

    let mut transaction = harness.transaction();
    transaction.remove(paint);
    transaction.remove_chunk(ChunkKey::new(1, 0));
    harness.commit(transaction);
}

#[test]
fn parent_chains_that_close_on_themselves_are_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let first = transaction.create_transform(root_node());
    let second = transaction.create_transform(root_node());
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.set_transform_node(
        first,
        iceblink::TransformNode {
            parent: Some(second),
            ..root_node()
        },
    );
    transaction.set_transform_node(
        second,
        iceblink::TransformNode {
            parent: Some(first),
            ..root_node()
        },
    );
    assert_eq!(rejected(&mut harness, transaction), RejectReason::Cycle);

    let mut transaction = harness.transaction();
    let node = transaction.create_order(OrderNode::default());
    transaction.splice(node, 0, 0, &[OrderEntry::Node(node)]);
    assert_eq!(rejected(&mut harness, transaction), RejectReason::Cycle);
}

#[test]
fn an_order_node_or_chunk_key_in_two_places_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let root = harness.root;
    let key = OrderEntry::Chunk(ChunkKey::new(1, 0));
    transaction.splice(root, 0, 0, &[key, key]);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::OrderConflict
    );

    let mut transaction = harness.transaction();
    let child = transaction.create_order(OrderNode::default());
    transaction.splice(
        root,
        0,
        0,
        &[OrderEntry::Node(child), OrderEntry::Node(child)],
    );
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::OrderConflict
    );
}

#[test]
fn a_splice_past_the_end_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let root = harness.root;
    transaction.splice(root, 1, 0, &[]);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::SpliceOutOfRange
    );
}

#[test]
fn a_paint_that_changes_kind_or_a_reference_of_another_kind_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let paint = transaction.create_paint(Paint::Solid(iceblink::Color([1.0; 4])));
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.set_paint(paint, Paint::Sides([iceblink::Color([1.0; 4]); 4]));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::PaintKindChanged
    );

    let mut transaction = harness.transaction();
    let props = harness.props();
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);
    let mut item = quad([0.0, 0.0, 8.0, 8.0], [9; 4]);
    item.fill = recorder.paint_ref(PaintKind::GRADIENT, paint);
    recorder.quad(item);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::PaintKindChanged
    );
}

#[test]
fn a_non_finite_number_or_an_unlisted_value_is_rejected() {
    let mut harness = Harness::new();
    let props = harness.props();

    let mut transaction = harness.transaction();
    transaction
        .record(ChunkKey::new(1, 0), props)
        .quad(quad([f32::NAN, 0.0, 8.0, 8.0], [9; 4]));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidItem
    );

    let mut transaction = harness.transaction();
    let mut item = quad([0.0, 0.0, 8.0, 8.0], [9; 4]);
    item.style = iceblink::BorderStyle(9);
    transaction.record(ChunkKey::new(1, 0), props).quad(item);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidItem
    );

    let mut transaction = harness.transaction();
    let space = harness.space;
    let mut matrix = iceblink::Matrix4::IDENTITY;
    matrix.columns[3][0] = f32::INFINITY;
    transaction.set_transform(space, matrix);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidItem
    );

    let mut transaction = harness.transaction();
    let mut item = quad([0.0, 0.0, 8.0, 8.0], [9; 4]);
    item.clip = iceblink::ClipRef(40);
    transaction.record(ChunkKey::new(1, 0), props).quad(item);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidItem
    );
}

#[test]
fn an_unknown_output_or_chunk_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let root = harness.root;
    transaction.set_root(iceblink::OutputId(9), Some(root));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::UnknownOutput
    );

    let mut transaction = harness.transaction();
    transaction.remove_chunk(ChunkKey::new(3, 3));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::UnknownChunk
    );
}

#[test]
fn an_image_write_outside_the_image_or_short_of_bytes_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let image = transaction.create_image(ImageDesc {
        width: 4,
        height: 4,
        format: ImageFormat::RGBA8,
    });
    harness.commit(transaction);

    let write = |x: i32, bytes: usize| ImageWrite {
        rect: DeviceRect::new(Point::new(x, 0), Size::new(2, 2)),
        bytes: Arc::from(vec![0u8; bytes]),
        offset: 0,
        stride: 8,
    };

    let mut transaction = harness.transaction();
    transaction.write_image(image, write(3, 16));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidWrite
    );

    let mut transaction = harness.transaction();
    transaction.write_image(image, write(0, 15));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidWrite
    );

    let mut transaction = harness.transaction();
    transaction.write_image(image, write(0, 16));
    harness.commit(transaction);
}

#[test]
fn a_shader_used_in_the_wrong_mode_or_with_large_parameters_is_rejected() {
    let mut harness = Harness::new();
    let desc = |mode, params_size| ShaderDesc {
        mode,
        source: ShaderSource {
            wgsl: Arc::from(""),
            prebuilt: None,
        },
        params_size,
        reads: ShaderReads::default(),
    };

    let mut transaction = harness.transaction();
    transaction.create_shader(desc(ShaderMode::PAINT, 65));
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidShader
    );

    let mut transaction = harness.transaction();
    let shader = transaction.create_shader(desc(ShaderMode::PAINT, 0));
    let params = transaction.create_params(iceblink::ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    transaction.create_effect(EffectNode {
        clip: None,
        opacity: 1.0,
        blend: Default::default(),
        filters: Arc::new([iceblink::Filter::Shader {
            shader,
            params,
            reach: 0.0,
        }]),
        isolate: false,
    });
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidShader
    );

    let mut transaction = harness.transaction();
    let shader = transaction.create_shader(desc(ShaderMode::FILTER, 0));
    let params = transaction.create_params(iceblink::ParamsBlock {
        pointer: [0.0; 2],
        hovered: 0.0,
        _reserved: 0.0,
        user: [0; 64],
    });
    let props = harness.props();
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);
    let item = ShaderQuad {
        bounds: [0.0, 0.0, 4.0, 4.0],
        radii: [0.0; 8],
        shapes: iceblink::ROUND,
        border: [0.0; 4],
        fill: iceblink::PaintRef::NONE,
        stroke: iceblink::PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: iceblink::ClipRef::INHERIT,
        space: iceblink::SpaceRef::INHERIT,
        shader: recorder.shader_slot(shader),
        params: recorder.params_slot(params),
        opacity: 1.0,
    };
    recorder.shader_quad(item);
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::InvalidShader
    );
}

#[test]
fn an_animation_is_rejected_as_unsupported() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let space = harness.space;
    transaction.animate(Animation {
        target: AnimationTarget::Transform(space),
        keyframes: Keyframes::Scalar(Default::default()),
        timing: Timing {
            duration_ns: 1,
            delay_ns: 0,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::None,
            easing: Easing::Linear,
        },
        start: AnimationStart::Next,
    });
    assert_eq!(
        rejected(&mut harness, transaction),
        RejectReason::Unsupported
    );
}

#[test]
fn removing_a_chunk_and_the_clip_it_names_together_commits() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let space = harness.space;
    let clip = transaction.create_clip(ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 8.0, 8.0],
            radii: [0.0; 8],
            shapes: iceblink::ROUND,
        },
    });
    let props = iceblink::ChunkProps {
        clip: Some(clip),
        ..harness.props()
    };
    transaction
        .record(ChunkKey::new(1, 0), props)
        .quad(quad([0.0, 0.0, 8.0, 8.0], [9; 4]));
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.remove(clip);
    assert_eq!(rejected(&mut harness, transaction), RejectReason::InUse);

    let mut transaction = harness.transaction();
    transaction.remove_chunk(ChunkKey::new(1, 0));
    transaction.remove(clip);
    harness.commit(transaction);
}
