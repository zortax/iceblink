//! Images: how fractional sources become the whole-texel tiles of colour sprites and mask clips.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    ClipNode, ClipRef, ClipShape, Extend, FrameClipShape, Image, ImageDesc, ImageFilter,
    ImageFlags, ImageFormat, ImageWrite, MaskMode, OrderEntry, PaintRef, ROUND, SpaceRef,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{key, quad};
use crate::conformance::scene::stage::Stage;

const GREEN: [u8; 4] = [0, 255, 0, 255];

/// A fractional source is rounded out on an axis without repeat, which widens the sprite's bounds
/// and cuts its frame to the item's bounds, and to the nearest texels on a repeating axis, which
/// keeps the bounds.
pub fn image_sources(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let id = tx.create_image(ImageDesc {
        width: 8,
        height: 8,
        format: ImageFormat::RGBA8_PREMULTIPLIED,
    });
    tx.write_image(
        id,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(8, 8)),
            bytes: Arc::from(vec![255; 256]),
            offset: 0,
            stride: 32,
        },
    );
    let mut chunk = tx.record(key(1), stage.props());
    let slot = chunk.image_slot(id);
    chunk.image(Image {
        bounds: [10.0, 10.0, 30.0, 30.0],
        frame: [0.0, 10.0, 60.0, 30.0],
        source: [0.5, 0.4, 3.0, 2.9],
        radii: [5.0; 8],
        shapes: ROUND,
        paint: PaintRef::NONE,
        image: slot,
        opacity: 1.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::default(),
        extend: [Extend::NONE, Extend::REPEAT],
        filter: ImageFilter::LINEAR,
    });
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    let picture = stage.redraw();

    assert_eq!(picture.colors.len(), 1, "an image draws one colour sprite");
    let sprite = &picture.colors[0];
    assert_eq!(
        [sprite.tile.rect[2], sprite.tile.rect[3]],
        [4, 3],
        "the source is rounded out to 0..4 across and to the nearest 0..3 down"
    );
    assert_eq!(
        [sprite.bounds[0], sprite.bounds[2]],
        [5.0, 40.0],
        "the bounds widen by the half texel taken in on each side across"
    );
    assert_eq!(
        [sprite.bounds[1], sprite.bounds[3]],
        [10.0, 30.0],
        "the bounds keep their place on the repeating axis"
    );
    assert_eq!(
        [sprite.frame[0], sprite.frame[2]],
        [10.0, 30.0],
        "the frame is cut to the item's bounds across"
    );
    assert_eq!(
        sprite.radii, [0.0; 8],
        "corners the cut removes by more than their radius lose their rounding"
    );
}

/// A source reaching past the image is cut to it, which shrinks the bounds on an axis without
/// repeat, and a mask clip's tile is its source rounded to the nearest texels.
pub fn image_edges(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let id = tx.create_image(ImageDesc {
        width: 8,
        height: 8,
        format: ImageFormat::RGBA8_PREMULTIPLIED,
    });
    tx.write_image(
        id,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(8, 8)),
            bytes: Arc::from(vec![255; 256]),
            offset: 0,
            stride: 32,
        },
    );
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::Mask {
            image: id,
            rect: [60.0, 60.0, 20.0, 20.0],
            source: [0.4, 0.6, 3.2, 3.2],
            mode: MaskMode::ALPHA,
        },
    });
    let mut chunk = tx.record(key(1), stage.props());
    let slot = chunk.image_slot(id);
    chunk.image(Image {
        bounds: [10.0, 10.0, 30.0, 30.0],
        frame: [10.0, 10.0, 30.0, 30.0],
        source: [6.0, 0.0, 4.0, 8.0],
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
    let masked = chunk.clip_ref(clip);
    let mut cut = quad([60.0, 60.0, 20.0, 20.0], GREEN);
    cut.clip = masked;
    chunk.quad(cut);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    let picture = stage.redraw();

    let sprite = &picture.colors[0];
    assert_eq!(
        (sprite.tile.rect[2], sprite.bounds[0], sprite.bounds[2]),
        (2, 10.0, 15.0),
        "a source of 6..10 across an image 8 wide is cut to 6..8, and the bounds keep 7.5 units a \
         texel"
    );
    let mask = picture.quad(GREEN).clip.as_ref().and_then(|entry| {
        entry.links.iter().find_map(|link| match link.shape {
            FrameClipShape::Mask { tile, .. } => Some([tile.rect[2], tile.rect[3]]),
            FrameClipShape::RoundedRect { .. } => None,
        })
    });
    assert_eq!(
        mask,
        Some([4, 3]),
        "a mask source of 0.4..3.6 by 0.6..3.8 is rounded to 0..4 by 1..4"
    );
}
