//! Transform anchors, images, clients and outputs.

mod common;

use std::sync::Arc;

use iceblink::{
    Anchor, Built, ChunkKey, ClipRef, Extend, FrameClock, FrameRequest, Image, ImageDesc,
    ImageFilter, ImageFlags, ImageFormat, ImageWrite, OrderEntry, OrderNode, PaintRef, ROUND,
    RenderCapabilities, ScrollNode, SpaceRef, StickyAnchor, TransformNode,
};
use iceblink::{Compositor, OutputConfig};

use common::{Scene, identity, props, quad, rect};

#[test]
fn a_sticky_header_holds_still_while_its_content_scrolls() {
    let mut scene = Scene::with_caps(RenderCapabilities::MINIMAL);
    let mut tx = scene.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: scene.space,
        port: [0.0, 0.0, 200.0, 200.0],
        content: [200.0, 1000.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(scene.space))
    });
    let header = tx.create_transform(TransformNode {
        anchor: Anchor::Sticky(StickyAnchor {
            scroll,
            rect: [0.0, 0.0, 200.0, 10.0],
            limit: [0.0, 0.0, 200.0, 1000.0],
            inset: [0.0, f32::NAN, f32::NAN, f32::NAN],
        }),
        ..identity(Some(content))
    });
    let fixed = tx.create_transform(TransformNode {
        anchor: Anchor::Fixed,
        ..identity(Some(content))
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(content));
    chunk.quad(quad([100.0, 50.0, 10.0, 10.0], PaintRef::NONE));
    let mut chunk = tx.record(ChunkKey::new(2, 0), props(header));
    chunk.quad(quad([0.0, 0.0, 50.0, 10.0], PaintRef::NONE));
    let mut chunk = tx.record(ChunkKey::new(3, 0), props(fixed));
    chunk.quad(quad([0.0, 150.0, 50.0, 10.0], PaintRef::NONE));
    let entries = [1, 2, 3].map(|owner| OrderEntry::Chunk(ChunkKey::new(owner, 0)));
    tx.splice(scene.root, 0, 0, &entries);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.scroll_to(scroll, [0.0, 20.0]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    let mut rects = scene.damage().expect("the damage is not full");
    rects.sort_by_key(|rect| rect.origin.y);
    assert_eq!(
        rects,
        vec![rect(100, 30, 10, 10), rect(100, 50, 10, 10)],
        "only the scrolled quad is redrawn"
    );
}

#[test]
fn an_image_write_damages_the_items_drawing_it() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let image = tx.create_image(ImageDesc {
        width: 2,
        height: 2,
        format: ImageFormat::RGBA8,
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let slot = chunk.image_slot(image);
    chunk.image(Image {
        bounds: [20.0, 20.0, 16.0, 16.0],
        frame: [20.0, 20.0, 16.0, 16.0],
        source: [0.0, 0.0, 2.0, 2.0],
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
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.write_image(
        image,
        ImageWrite {
            rect: rect(0, 0, 2, 2),
            bytes: Arc::from(vec![255_u8; 16]),
            offset: 0,
            stride: 8,
        },
    );
    scene.commit(tx);
    assert_eq!(scene.compositor.counters().commit.images_written, 1);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(20, 20, 16, 16)]));
    assert_eq!(scene.compositor.memory().image_bytes, 16);
}

#[test]
fn unregistering_a_client_clears_the_outputs_it_drew() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 10.0, 10.0]]);
    scene.draw();

    scene.compositor.unregister_client(scene.client);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.damage.draw.is_full());
    assert!(scene.frame.batches.is_empty());

    scene.done(common::presented());
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });
}

#[test]
fn a_change_damages_only_the_output_that_draws_it() {
    let mut scene = Scene::new();
    let second = scene.compositor.create_output(OutputConfig {
        size: [100, 100],
        scale: 1.0,
        opaque: true,
    });
    let mut tx = scene.tx();
    let other = tx.create_order(OrderNode::default());
    tx.set_root(second, Some(other));
    scene.commit(tx);

    let mut frame = iceblink::Frame::default();
    let request = FrameRequest {
        caps: &common::FULL,
        age: iceblink::BufferAge::Kept,
        clock: FrameClock::default(),
        subpixel: None,
    };
    assert_eq!(
        scene.compositor.build_frame(second, &request, &mut frame),
        Built::Frame
    );
    scene
        .compositor
        .frame_done(second, frame.id, &common::presented());
    scene.draw();

    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 10.0, 10.0]]);
    assert_eq!(
        scene.compositor.build_frame(second, &request, &mut frame),
        Built::Idle { wake_ns: None }
    );
    assert_eq!(scene.build(), Built::Frame);
}
