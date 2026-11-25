//! Damage soundness: every change damages the pixels it can change, and a precise compositor
//! damages little more.

use std::sync::Arc;

use iceblink::OutputConfig;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Built, ClipNode, ClipRef, ClipShape, Color, Extend, ExternalQuad, Image, ImageDesc,
    ImageFilter, ImageFlags, ImageFormat, ImageWrite, Matrix4, OrderEntry, OrderNode, Paint,
    PaintKind, PaintRef, ROUND, SpaceRef, TextureDesc,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{
    device, faded, identity, key, pixels, props, quad, record_quads,
};
use crate::conformance::scene::stage::{SIDE, Stage, area, covers};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Asserts that the last frame's draw damage covers `rect`.
#[track_caller]
fn damaged(stage: &Stage, rect: [f32; 4], what: &str) {
    let pixels = pixels(rect);
    assert!(
        covers(stage.damage(), pixels),
        "{what}: the damage {:?} misses {pixels:?}",
        stage.damage()
    );
}

/// Asserts that a precise compositor's last frame damaged at most `limit` pixels.
#[track_caller]
fn at_most(stage: &Stage, case: &str, limit: i64, what: &str) {
    if !stage.precise(case) {
        return;
    }
    let damaged = area(stage.damage());
    assert!(
        damaged <= limit,
        "{what}: a precise compositor damaged {damaged} pixels, where {limit} hold the change"
    );
}

/// Builds the next frame and asserts that it is one.
#[track_caller]
fn frame(stage: &mut Stage, what: &str) {
    assert_eq!(stage.draw(), Built::Frame, "{what} builds a frame");
}

/// Recording, recording again, moving and removing a chunk damage its old and new ink.
pub fn chunk_changes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let old = [10.0, 10.0, 20.0, 20.0];
    let new = [70.5, 60.25, 20.0, 20.0];
    stage.add_quads(key(1), &[(old, RED)]);
    frame(&mut stage, "a new chunk");
    damaged(&stage, old, "a new chunk");
    at_most(&stage, "chunk_changes", 21 * 21, "a new chunk");

    let mut tx = stage.tx();
    record_quads(&mut tx, key(1), stage.props(), &[(new, RED)]);
    stage.commit(tx);
    frame(&mut stage, "a chunk recorded again");
    damaged(&stage, old, "the old ink of a recorded chunk");
    damaged(&stage, new, "the new ink of a recorded chunk");
    at_most(
        &stage,
        "chunk_changes",
        2 * 21 * 21,
        "a chunk recorded again",
    );

    let mut tx = stage.tx();
    tx.set_offset(key(1), [-40.0, 3.0]);
    stage.commit(tx);
    frame(&mut stage, "a chunk offset");
    damaged(&stage, new, "the old ink of a moved chunk");
    damaged(
        &stage,
        [30.5, 63.25, 20.0, 20.0],
        "the new ink of a moved chunk",
    );

    let mut tx = stage.tx();
    tx.remove_chunk(key(1));
    stage.commit(tx);
    frame(&mut stage, "a removed chunk");
    damaged(
        &stage,
        [30.5, 63.25, 20.0, 20.0],
        "the ink of a removed chunk",
    );
}

/// Writing a paint, a transform or a clip damages what is drawn with it.
pub fn node_and_slot_writes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 0.0, 0.0, 1.0])));
    let moved = tx.create_transform(identity(Some(stage.space)));
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 64.0, 128.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let mut chunk = tx.record(key(1), stage.props());
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let mut painted = quad([4.0, 4.0, 16.0, 16.0], RED);
    painted.fill = fill;
    chunk.quad(painted);
    record_quads(
        &mut tx,
        key(2),
        props(moved),
        &[([40.0, 40.0, 16.0, 16.0], BLUE)],
    );
    let mut clipped = props(stage.space);
    clipped.clip = Some(clip);
    record_quads(
        &mut tx,
        key(3),
        clipped,
        &[([32.0, 100.0, 64.0, 16.0], BLUE)],
    );
    for part in 1..=3 {
        stage.append(&mut tx, OrderEntry::Chunk(key(part)));
    }
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    tx.set_paint(paint, Paint::Solid(Color([0.0, 1.0, 0.0, 1.0])));
    stage.commit(tx);
    frame(&mut stage, "a paint write");
    damaged(&stage, [4.0, 4.0, 16.0, 16.0], "a paint write");
    at_most(&stage, "node_and_slot_writes", 17 * 17, "a paint write");

    let mut tx = stage.tx();
    tx.set_transform(moved, Matrix4::translation(30.0, 20.0, 0.0));
    stage.commit(tx);
    frame(&mut stage, "a transform write");
    damaged(
        &stage,
        [40.0, 40.0, 16.0, 16.0],
        "the old ink under a transform write",
    );
    damaged(
        &stage,
        [70.0, 60.0, 16.0, 16.0],
        "the new ink under a transform write",
    );

    let mut tx = stage.tx();
    tx.set_clip(
        clip,
        ClipNode {
            parent: None,
            space: stage.space,
            shape: ClipShape::RoundedRect {
                rect: [0.0, 0.0, 90.0, 128.0],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        },
    );
    stage.commit(tx);
    frame(&mut stage, "a clip write");
    damaged(
        &stage,
        [64.0, 100.0, 26.0, 16.0],
        "the pixels a clip write uncovers",
    );
    at_most(&stage, "node_and_slot_writes", 2 * 64 * 16, "a clip write");
}

/// Splicing entries in and out of the order tree damages their subtrees, and a new root damages
/// the whole output.
pub fn order_changes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let node = tx.create_order(OrderNode::default());
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([8.0, 8.0, 24.0, 24.0], RED)],
    );
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(1))]);
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.commit(tx);
    frame(&mut stage, "an inserted entry");
    damaged(&stage, [8.0, 8.0, 24.0, 24.0], "an inserted subtree");

    let mut tx = stage.tx();
    tx.splice(stage.root, 0, 1, &[]);
    stage.commit(tx);
    frame(&mut stage, "a removed entry");
    damaged(&stage, [8.0, 8.0, 24.0, 24.0], "a removed subtree");

    let mut tx = stage.tx();
    let other = tx.create_order(OrderNode::default());
    tx.set_root(stage.output, Some(other));
    stage.commit(tx);
    frame(&mut stage, "a new root");
    assert!(
        covers(stage.damage(), pixels([0.0, 0.0, SIDE as f32, SIDE as f32])),
        "a new root damages the whole output"
    );
}

/// Writing an effect damages the region of the group it forms.
pub fn effect_writes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let effect = tx.create_effect(faded(0.5));
    let node = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([20.0, 20.0, 30.0, 30.0], RED)],
    );
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(1))]);
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    tx.set_effect(effect, faded(0.25));
    stage.commit(tx);
    frame(&mut stage, "an effect write");
    damaged(&stage, [20.0, 20.0, 30.0, 30.0], "an opacity write");

    let mut tx = stage.tx();
    tx.set_order_effect(node, None);
    stage.commit(tx);
    frame(&mut stage, "an order effect write");
    damaged(
        &stage,
        [20.0, 20.0, 30.0, 30.0],
        "an effect taken off its node",
    );
}

/// Growing the output damages the pixels it gains, and a new scale or opacity damages it all.
pub fn output_changes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.add_quads(key(1), &[([8.0, 8.0, 16.0, 16.0], RED)]);
    stage.draw();

    let grown = OutputConfig {
        size: [SIDE + 32, SIDE],
        scale: 1.0,
        opaque: true,
    };
    stage.compositor.configure_output(stage.output, grown);
    frame(&mut stage, "a larger output");
    assert!(
        covers(stage.damage(), device(SIDE as i32, 0, 32, SIDE as i32)),
        "a larger output damages the pixels it gains: {:?}",
        stage.damage()
    );

    stage.compositor.configure_output(
        stage.output,
        OutputConfig {
            scale: 2.0,
            ..grown
        },
    );
    frame(&mut stage, "a new scale");
    assert!(
        stage.damage().is_full()
            || covers(
                stage.damage(),
                pixels([0.0, 0.0, (SIDE + 32) as f32, SIDE as f32])
            ),
        "a new scale damages the whole output"
    );
}

/// Writing an image, a parameter block or an external texture's frame damages what draws it.
pub fn resource_writes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let image = tx.create_image(ImageDesc {
        width: 8,
        height: 8,
        format: ImageFormat::RGBA8,
    });
    let texture = tx.create_texture(TextureDesc {
        width: 8,
        height: 8,
        premultiplied: true,
    });
    let mut chunk = tx.record(key(1), stage.props());
    let slot = chunk.image_slot(image);
    chunk.image(Image {
        bounds: [8.0, 8.0, 32.0, 32.0],
        frame: [8.0, 8.0, 32.0, 32.0],
        source: [0.0, 0.0, 8.0, 8.0],
        radii: [0.0; 8],
        shapes: ROUND,
        paint: PaintRef::NONE,
        image: slot,
        opacity: 1.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::default(),
        extend: [Extend::NONE; 2],
        filter: ImageFilter::LINEAR,
    });
    let texture_slot = chunk.texture_slot(texture);
    chunk.external(ExternalQuad {
        bounds: [60.0, 60.0, 30.0, 30.0],
        texture: texture_slot,
        opacity: 1.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(8, 8)),
            bytes: Arc::from(vec![200; 8 * 8 * 4]),
            offset: 0,
            stride: 32,
        },
    );
    stage.commit(tx);
    frame(&mut stage, "an image write");
    damaged(&stage, [8.0, 8.0, 32.0, 32.0], "an image write");

    let mut tx = stage.tx();
    tx.texture_frame(texture);
    stage.commit(tx);
    frame(&mut stage, "a texture frame");
    damaged(&stage, [60.0, 60.0, 30.0, 30.0], "a texture frame");
}
