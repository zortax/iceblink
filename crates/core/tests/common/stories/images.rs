//! Images drawn plain, repeated and as masks, and a mask clip.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    ClipNode, ClipRef, ClipShape, Extend, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat,
    ImageWrite, OrderEntry, PaintKind, ROUND, SpaceRef, Transaction,
};

use crate::common::stories::{background, key};
use crate::common::story::Story;
use crate::common::{props, quad};

/// Writes a checker of `side` by `side` texels with cells of `cell` texels into `image`, in
/// straight RGBA.
fn checker(tx: &mut Transaction, image: iceblink::ImageId, side: u32, cell: u32) {
    let mut bytes = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let on = ((x / cell) + (y / cell)).is_multiple_of(2);
            bytes.extend_from_slice(if on {
                &[200, 40, 40, 255]
            } else {
                &[40, 40, 200, 128]
            });
        }
    }

    tx.write_image(
        image,
        ImageWrite {
            rect: Rect::new(Point::new(0, 0), Size::new(side as i32, side as i32)),
            bytes: Arc::from(bytes),
            offset: 0,
            stride: side * 4,
        },
    );
}

/// An image drawn plain, repeated and as a tinted mask, and a mask clip, then the image rewritten
/// with smaller cells.
pub(crate) const IMAGES: Story = Story {
    name: "images",
    steps: &[
        background,
        |tx, world| {
            let image = tx.create_image(ImageDesc {
                width: 16,
                height: 16,
                format: ImageFormat::RGBA8,
            });
            world.images.push(image);
            checker(tx, image, 16, 4);
            let tint = world.solid(tx, [0.0, 0.5, 0.0, 1.0]);

            let mask = tx.create_clip(ClipNode {
                parent: None,
                space: world.space,
                shape: ClipShape::Mask {
                    image,
                    rect: [70.0, 70.0, 48.0, 48.0],
                    source: [0.0, 0.0, 16.0, 16.0],
                    mode: iceblink::MaskMode::ALPHA,
                },
            });
            world.clips.push(mask);

            let mut chunk = tx.record(key(2), props(world.space));
            let slot = chunk.image_slot(image);
            let picture = |bounds: [f32; 4], frame: [f32; 4]| Image {
                bounds,
                frame,
                source: [0.0, 0.0, 16.0, 16.0],
                radii: [0.0; 8],
                shapes: ROUND,
                paint: iceblink::PaintRef::NONE,
                image: slot,
                opacity: 1.0,
                clip: ClipRef::INHERIT,
                space: SpaceRef::INHERIT,
                flags: ImageFlags::default(),
                extend: [Extend::NONE; 2],
                filter: ImageFilter::NEAREST,
            };

            chunk.image(picture([8.0, 8.0, 32.0, 32.0], [8.0, 8.0, 32.0, 32.0]));
            let mut repeated = picture([48.0, 8.0, 16.0, 16.0], [48.0, 8.0, 64.0, 32.0]);
            repeated.extend = [Extend::REPEAT; 2];
            chunk.image(repeated);
            let mut tinted = picture([8.0, 56.0, 32.0, 32.0], [8.0, 56.0, 32.0, 32.0]);
            tinted.flags = ImageFlags::MASK;
            tinted.paint = chunk.paint_ref(PaintKind::SOLID, tint);
            chunk.image(tinted);

            let masked = chunk.clip_ref(mask);
            let mut fill = quad(
                [70.0, 70.0, 48.0, 48.0],
                chunk.paint_ref(PaintKind::SOLID, tint),
            );
            fill.clip = masked;
            chunk.quad(fill);
            world.append(tx, OrderEntry::Chunk(key(2)));
        },
        |tx, world| checker(tx, world.images[0], 16, 2),
    ],
};
