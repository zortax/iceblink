//! When frames are built, what they damage, and what they tell the renderer.

mod common;

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    BatchKind, BufferAge, Built, ChunkKey, ChunkNote, ChunkProps, ClipRef, Extend, Frame,
    FrameClock, FrameOutcome, FrameRequest, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat,
    ImageWrite, OrderEntry, OrderNode, PageOp, PaintRef, ROUND, RenderCapabilities, SkipReason,
    SpaceRef,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_testing::FrameValidator;
use iceblink_testing::naive::NaiveCompositor;

use crate::common::naive::{Harness, quad, root_node};

#[test]
fn a_frame_damages_the_whole_output_and_draws_every_item_in_paint_order() {
    let mut harness = Harness::new();
    let red = quad([0.0, 0.0, 10.0, 10.0], [255, 0, 0, 255]);
    let blue = quad([5.0, 5.0, 10.0, 10.0], [0, 0, 255, 255]);
    harness.add_quads(ChunkKey::new(1, 0), &[red, blue]);

    assert!(harness.frame());
    let frame = &harness.frame;
    assert!(frame.damage.draw.is_full() && frame.damage.present.is_full());
    assert_eq!(frame.lanes.quads.items.len(), 2);
    assert_eq!(frame.lanes.quads.items[0].bounds, red.bounds);
    let kinds: Vec<_> = frame.batches.iter().map(|batch| batch.kind).collect();
    assert_eq!(kinds, [BatchKind::Quads, BatchKind::Quads]);
    assert!(frame.tables.paints.replace && frame.tables.spaces.replace);
}

#[test]
fn an_output_with_nothing_changed_is_idle() {
    let mut harness = Harness::new();
    harness.add_quads(ChunkKey::new(1, 0), &[quad([0.0; 4], [0; 4])]);

    assert!(harness.frame());
    assert!(!harness.frame());

    harness.add_quads(ChunkKey::new(2, 0), &[quad([1.0, 1.0, 4.0, 4.0], [9; 4])]);
    assert!(harness.frame());
}

#[test]
fn a_frame_that_recorded_nothing_is_drawn_again() {
    let mut harness = Harness::new();
    harness.add_quads(ChunkKey::new(1, 0), &[quad([0.0, 0.0, 4.0, 4.0], [9; 4])]);

    assert!(harness.frame_with(FrameOutcome::Skipped(SkipReason::Unacquired)));
    assert!(harness.frame_with(FrameOutcome::Skipped(SkipReason::Timeout)));
    assert!(!harness.frame());
}

#[test]
fn a_change_of_capabilities_or_size_damages_the_whole_output() {
    let mut harness = Harness::new();
    assert!(harness.frame());
    assert!(!harness.frame());

    harness.set_caps(RenderCapabilities {
        max_texture_size: 2048,
        ..RenderCapabilities::MINIMAL
    });
    assert!(harness.frame());

    let output = harness.output;
    harness.compositor.configure_output(
        output,
        OutputConfig {
            size: [32, 32],
            scale: 2.0,
            opaque: true,
        },
    );
    assert!(harness.frame());
    assert_eq!(harness.frame.size, [32, 32]);
    assert!(harness.frame.damage.draw.is_full());
}

#[test]
fn a_renderer_with_resident_chunks_is_reset_after_a_recovery() {
    let mut harness = Harness::new();
    harness.set_caps(RenderCapabilities {
        resident_chunks: true,
        ..RenderCapabilities::MINIMAL
    });
    harness.add_quads(ChunkKey::new(1, 0), &[quad([0.0, 0.0, 4.0, 4.0], [9; 4])]);

    assert!(harness.frame_with(FrameOutcome::Recovered));
    assert!(matches!(
        harness.frame.chunk_notes[..],
        [ChunkNote::ResetAll]
    ));
    assert!(harness.frame());
    assert!(matches!(
        harness.frame.chunk_notes[..],
        [ChunkNote::ResetAll]
    ));
    assert!(harness.frame.drawn.is_empty());

    harness.add_quads(ChunkKey::new(2, 0), &[quad([0.0, 0.0, 4.0, 4.0], [9; 4])]);
    assert!(harness.frame());
    assert!(harness.frame.chunk_notes.is_empty());
}

#[test]
fn frame_ids_count_up_by_one_per_built_frame() {
    let mut harness = Harness::new();
    assert!(harness.frame());
    let first = harness.frame.id;
    assert!(!harness.frame());

    harness.add_quads(ChunkKey::new(1, 0), &[quad([0.0; 4], [0; 4])]);
    assert!(harness.frame());
    assert_eq!(harness.frame.id.0, first.0 + 1);
}

#[test]
fn pages_fit_the_smallest_renderer_of_every_output() {
    let mut compositor = NaiveCompositor::new();
    let (client, _) = compositor.register_client();
    let config = OutputConfig {
        size: [64, 48],
        scale: 1.0,
        opaque: true,
    };
    let outputs = [
        compositor.create_output(config),
        compositor.create_output(config),
    ];
    let mut tx = compositor.transaction(client);
    let space = tx.create_transform(root_node());
    let image = tx.create_image(ImageDesc {
        width: 2000,
        height: 4,
        format: ImageFormat::RGBA8_PREMULTIPLIED,
    });
    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(2000, 4)),
            bytes: Arc::from(vec![255; 2000 * 4 * 4]),
            offset: 0,
            stride: 2000 * 4,
        },
    );
    for (index, output) in outputs.iter().enumerate() {
        let root = tx.create_order(OrderNode::default());
        let key = ChunkKey::new(index as u64, 0);
        let props = ChunkProps {
            space,
            clip: None,
            flags: Default::default(),
        };
        let mut chunk = tx.record(key, props);
        let slot = chunk.image_slot(image);
        chunk.image(Image {
            bounds: [0.0, 0.0, 64.0, 4.0],
            frame: [0.0, 0.0, 64.0, 4.0],
            source: [0.0, 0.0, 2000.0, 4.0],
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
        tx.splice(root, 0, 0, &[OrderEntry::Chunk(key)]);
        tx.set_root(*output, Some(root));
    }
    compositor.commit(tx).expect("the scene commits");

    for (output, limit) in outputs.into_iter().zip([4096, 1024]) {
        let caps = RenderCapabilities {
            max_texture_size: limit,
            ..RenderCapabilities::MINIMAL
        };
        let request = FrameRequest {
            caps: &caps,
            age: BufferAge::Unknown,
            clock: FrameClock::default(),
            subpixel: None,
        };
        let mut frame = Frame::default();
        assert_eq!(
            compositor.build_frame(output, &request, &mut frame),
            Built::Frame
        );
        let mut validator = FrameValidator::new();
        validator.set_capabilities(Some(caps));
        if let Err(violations) = validator.check(&frame) {
            panic!("a frame for a renderer of {limit} texels: {violations:?}");
        }
        let largest = frame
            .resources
            .pages
            .iter()
            .filter_map(|op| match op {
                PageOp::Create { size, .. } => Some(size[0].max(size[1])),
                _ => None,
            })
            .max();
        assert!(
            largest.is_some_and(|side| side <= limit),
            "every page a renderer of {limit} texels gets fits it, got {largest:?}"
        );
        compositor.frame_done(
            output,
            frame.id,
            &FrameOutcome::Presented(Default::default()),
        );
    }
}
