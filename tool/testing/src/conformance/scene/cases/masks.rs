//! Damage of image writes under the clips that mask with the image.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Built, ClipId, ClipNode, ClipShape, EffectId, EffectNode, ImageDesc, ImageFormat, ImageId,
    ImageWrite, LocalClip, LocalParent, MaskMode, OrderEntry, OrderNode, ROUND, Transaction,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{faded, key, pixels, quad, record_quads};
use crate::conformance::scene::stage::{Stage, covers};

const RED: [u8; 4] = [255, 0, 0, 255];

/// Writing an image damages every item drawn through a chunk-local clip or a clip node that masks
/// with it, and every group drawn through an effect clip that does.
pub fn mask_writes(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let local = image(&mut tx);
    let node = image(&mut tx);

    let mut chunk = tx.record(key(1), stage.props());
    let masked = chunk.local_clip(LocalClip {
        parent: LocalParent::Chunk,
        shape: mask(local, [0.0, 0.0, 60.0, 128.0]),
    });
    let nested = chunk.local_clip(LocalClip {
        parent: LocalParent::Local(0),
        shape: ClipShape::RoundedRect {
            rect: [0.0, 60.0, 60.0, 68.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let mut through = quad([10.0, 10.0, 30.0, 30.0], RED);
    through.clip = masked;
    chunk.quad(through);
    let mut under = quad([10.0, 70.0, 30.0, 30.0], RED);
    under.clip = nested;
    chunk.quad(under);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));

    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: mask(node, [70.0, 0.0, 58.0, 128.0]),
    });
    let grouped = clipped(&mut tx, clip, 0.5);
    let in_place = clipped(&mut tx, clip, 1.0);
    for (part, effect, bounds) in [
        (2, grouped, [80.0, 10.0, 30.0, 30.0]),
        (3, in_place, [80.0, 70.0, 30.0, 30.0]),
    ] {
        let order = tx.create_order(OrderNode {
            effect: Some(effect),
        });
        record_quads(&mut tx, key(part), stage.props(), &[(bounds, RED)]);
        tx.splice(order, 0, 0, &[OrderEntry::Chunk(key(part))]);
        stage.append(&mut tx, OrderEntry::Node(order));
    }
    stage.commit(tx);
    stage.draw();

    let mut tx = stage.tx();
    write(&mut tx, local);
    stage.commit(tx);
    assert_eq!(
        stage.draw(),
        Built::Frame,
        "a write of a local mask builds a frame"
    );
    for (rect, what) in [
        (
            [10.0, 10.0, 30.0, 30.0],
            "an item drawn through a local mask clip",
        ),
        (
            [10.0, 70.0, 30.0, 30.0],
            "an item drawn through a local clip under a local mask",
        ),
    ] {
        damaged(&stage, rect, what);
    }

    let mut tx = stage.tx();
    write(&mut tx, node);
    stage.commit(tx);
    assert_eq!(
        stage.draw(),
        Built::Frame,
        "a write of an effect's mask builds a frame"
    );
    for (rect, what) in [
        (
            [80.0, 10.0, 30.0, 30.0],
            "a group drawn through its effect's mask clip",
        ),
        (
            [80.0, 70.0, 30.0, 30.0],
            "an item under an effect in place with a mask clip",
        ),
    ] {
        damaged(&stage, rect, what);
    }
}

/// Asserts that the last frame's draw damage covers `rect`, where `what` is drawn.
#[track_caller]
fn damaged(stage: &Stage, rect: [f32; 4], what: &str) {
    let pixels = pixels(rect);
    assert!(
        covers(stage.damage(), pixels),
        "a mask write misses {what}: the damage {:?} misses {pixels:?}",
        stage.damage()
    );
}

/// Creates an 8 by 8 image with every texel written.
fn image(tx: &mut Transaction) -> ImageId {
    let id = tx.create_image(ImageDesc {
        width: 8,
        height: 8,
        format: ImageFormat::RGBA8_PREMULTIPLIED,
    });
    tx.write_image(id, opaque(255));
    id
}

/// Writes every texel of image `id` with new contents.
fn write(tx: &mut Transaction, id: ImageId) {
    tx.write_image(id, opaque(128));
}

/// A write of every texel of an 8 by 8 image with `value` in every channel.
fn opaque(value: u8) -> ImageWrite {
    ImageWrite {
        rect: Rect::new(Point::new(0, 0), Size::new(8, 8)),
        bytes: Arc::from(vec![value; 8 * 8 * 4]),
        offset: 0,
        stride: 32,
    }
}

/// A mask of the whole image `id` over `rect`.
fn mask(id: ImageId, rect: [f32; 4]) -> ClipShape {
    ClipShape::Mask {
        image: id,
        rect,
        source: [0.0, 0.0, 8.0, 8.0],
        mode: MaskMode::ALPHA,
    }
}

/// Creates an effect with `opacity` and `clip`, which isolates below an opacity of one.
fn clipped(tx: &mut Transaction, clip: ClipId, opacity: f32) -> EffectId {
    tx.create_effect(EffectNode {
        clip: Some(clip),
        ..faded(opacity)
    })
}
