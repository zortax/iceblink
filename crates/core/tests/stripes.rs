//! Subpixel text on a device, drawn for both stripe orders.

mod common;

use iceblink::{ChunkKey, GlyphFlags, OrderEntry, PaintKind, SubpixelOrder};
use iceblink::{Compositor, Renderer};
use iceblink_testing::golden::RgbaImage;
use iceblink_testing::gpu::device_lock;

use common::gpu::{presented, slice};
use common::slice::SIDE;
use common::text::run_with;
use common::{props, quad};

/// Draws white subpixel text on black for a display of `order`, and returns the image.
///
/// The renderer and the frame request are both told the order, as a client tells them. Returns
/// `None` when no device opens.
fn drawn(order: SubpixelOrder) -> Option<RgbaImage> {
    let mut slice = slice()?;
    assert!(
        slice.renderer.capabilities().subpixel_text,
        "the device blends per-channel coverage"
    );
    slice.renderer.set_subpixel_order(order);
    slice.subpixel = Some(order);

    let mut tx = slice.tx();
    let world = &mut slice.world;
    let black = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let white = world.solid(&mut tx, [1.0, 1.0, 1.0, 1.0]);
    let font = world.font;
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, black);
    chunk.quad(quad([0.0, 0.0, SIDE as f32, SIDE as f32], fill));
    let fill = chunk.paint_ref(PaintKind::SOLID, white);
    let flags = GlyphFlags::SUBPIXEL | GlyphFlags::HINT;
    run_with(&mut chunk, font, "W", 24.0, [10.0, 40.0], fill, flags);
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the run commits");

    assert!(slice.draw(), "the first frame is built");
    assert!(
        !slice.frame.lanes.subpixel.sources.is_empty(),
        "the run is drawn with subpixel sprites"
    );
    Some(presented(&slice))
}

#[test]
fn the_blue_to_red_order_draws_the_red_and_blue_coverage_swapped() {
    let _device = device_lock();
    let (Some(rgb), Some(bgr)) = (
        drawn(SubpixelOrder::RedToBlue),
        drawn(SubpixelOrder::BlueToRed),
    ) else {
        return;
    };

    let mut fringed = 0;
    for (straight, mirrored) in rgb
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(bgr.pixels().as_chunks::<4>().0)
    {
        assert_eq!(
            [mirrored[2], mirrored[1], mirrored[0]],
            [straight[0], straight[1], straight[2]],
            "a pixel of one order is the other's with red and blue exchanged"
        );
        fringed += usize::from(straight[0] != straight[2]);
    }
    assert!(fringed > 0, "the glyph has coloured fringes");
}
