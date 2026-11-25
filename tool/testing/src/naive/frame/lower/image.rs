//! Images, drawn as colour sprites from pages of their own.

use iceblink::{
    AtlasTile, BatchKind, ColorSprite, Extend, Image, ImageFilter, ImageFlags, Instance,
    SpriteFlags,
};

use crate::naive::content::ImagePage;
use crate::naive::frame::builder::Builder;
use crate::naive::frame::item;
use crate::naive::geometry::Bounds;
use crate::naive::scene::Chunk;

pub(crate) fn image(builder: &mut Builder<'_>, chunk: &Chunk, image: &Image) -> Bounds {
    let Some(placed) = item::place(builder, chunk, image.space, image.clip) else {
        return Bounds::EMPTY;
    };

    let ink = placed.ink(local_ink(image), builder.env.output);
    if !item::shows(builder, ink) {
        return ink;
    }

    let Some(page) = builder.image_page(image.image) else {
        return ink;
    };

    let Some(fitted) = Fitted::new(image, &page) else {
        return ink;
    };

    let sprite = ColorSprite {
        bounds: fitted.bounds,
        frame: fitted.frame,
        radii: fitted.radii,
        shapes: image.shapes,
        tile: AtlasTile {
            page: page.page,
            rect: fitted.tile,
        },
        paint: builder.paint(image.paint),
        paint_origin: [0.0; 2],
        opacity: image.opacity,
        clip: placed.clip,
        space: placed.space,
        flags: flags(image),
    }
    .moved(placed.offset);
    builder.instance(
        |lanes| &mut lanes.color,
        sprite,
        BatchKind::Color { page: page.page },
    );
    ink
}

/// An image's sprite geometry, fitted to whole texels of its page.
struct Fitted {
    bounds: [f32; 4],
    frame: [f32; 4],
    radii: [f32; 8],
    /// The texels of the page the sprite samples.
    tile: [u32; 4],
}

impl Fitted {
    /// Fits `image` to whole texels of `page`, or returns `None` when it covers no texel.
    fn new(image: &Image, page: &ImagePage) -> Option<Self> {
        let reduced = page.reduced as f32;
        let repeat = image.extend.map(|extend| extend == Extend::REPEAT);
        let mut bounds = image.bounds;
        let mut frame = image.frame;
        let mut radii = image.radii;
        let mut tile = [0u32; 4];
        for axis in 0..2 {
            let start = image.source[axis] / reduced;
            let length = image.source[axis + 2] / reduced;
            let size = page.size[axis] as f32;
            if length <= 0.0 || bounds[axis + 2] <= 0.0 {
                return None;
            }

            // The tile is whole texels. A repeating axis rounds its source to the nearest texels
            // and keeps its period, and any other axis rounds it out and widens the bounds by the
            // texels it takes in, so each texel stays where it was.
            let (first, last) = if repeat[axis] {
                let first = start.round().clamp(0.0, size);
                (first, (start + length).round().clamp(first, size))
            } else {
                let first = start.floor().clamp(0.0, size);
                (first, (start + length).ceil().clamp(first, size))
            };
            if last <= first {
                return None;
            }

            if !repeat[axis] {
                let per_texel = bounds[axis + 2] / length;
                let widened = [
                    bounds[axis] - (start - first) * per_texel,
                    (last - first) * per_texel,
                ];
                if widened != [bounds[axis], bounds[axis + 2]] {
                    cut_frame(&mut frame, &mut radii, bounds, axis);
                }
                [bounds[axis], bounds[axis + 2]] = widened;
            }
            tile[axis] = first as u32;
            tile[axis + 2] = (last - first) as u32;
        }

        Some(Self {
            bounds,
            frame,
            radii,
            tile,
        })
    }
}

/// Returns the sprite flags of an image's flags, extend modes and filter.
fn flags(image: &Image) -> SpriteFlags {
    let repeat = image.extend.map(|extend| extend == Extend::REPEAT);
    let mut flags = SpriteFlags::default();
    if image.flags.contains(ImageFlags::GRAYSCALE) {
        flags |= SpriteFlags::GRAYSCALE;
    }
    if image.flags.contains(ImageFlags::MASK) {
        flags |= SpriteFlags::MASK;
    }
    if repeat[0] {
        flags |= SpriteFlags::REPEAT_X;
    }
    if repeat[1] {
        flags |= SpriteFlags::REPEAT_Y;
    }
    if image.filter == ImageFilter::NEAREST {
        flags |= SpriteFlags::NEAREST;
    }

    flags
}

/// Returns an image's local ink: `bounds ∩ frame`, widened to `frame` along an axis that repeats.
fn local_ink(image: &Image) -> Bounds {
    let bounds = Bounds::of(image.bounds);
    let frame = Bounds::of(image.frame);
    let mut ink = bounds.intersect(frame);
    if image.extend[0] == Extend::REPEAT {
        ink.x0 = frame.x0;
        ink.x1 = frame.x1;
    }
    if image.extend[1] == Extend::REPEAT {
        ink.y0 = frame.y0;
        ink.y1 = frame.y1;
    }
    ink
}

/// Cuts `frame` to `bounds` on `axis`, zero for x and one for y, so the picture stops where the
/// item's bounds do.
///
/// A corner the cut removes by at least its radius on `axis` loses its rounding.
fn cut_frame(frame: &mut [f32; 4], radii: &mut [f32; 8], bounds: [f32; 4], axis: usize) {
    let start = frame[axis].max(bounds[axis]);
    let end = (frame[axis] + frame[axis + 2]).min(bounds[axis] + bounds[axis + 2]);
    let cuts = [start - frame[axis], frame[axis] + frame[axis + 2] - end];
    frame[axis] = start;
    frame[axis + 2] = (end - start).max(0.0);

    // The corners clockwise from the top left, on the low and the high side of the axis.
    let sides = if axis == 0 {
        [[0, 3], [1, 2]]
    } else {
        [[0, 1], [3, 2]]
    };
    for (corners, cut) in sides.into_iter().zip(cuts) {
        for corner in corners {
            if cut > 0.0 && cut >= radii[2 * corner + axis] {
                radii[2 * corner] = 0.0;
                radii[2 * corner + 1] = 0.0;
            }
        }
    }
}
