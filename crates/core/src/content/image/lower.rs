//! Image items lowered into colour sprites.

use iceblink::{AtlasTile, ColorSprite, Extend, Image, ImageFilter, ImageFlags, SpriteFlags};

use crate::content::cache::ContentCache;
use crate::content::error::ContentError;
use crate::content::image::cut::cut_frame;
use crate::content::image::place::ImageTileRef;
use crate::content::image::store::{ImageKey, ImageStore};
use crate::content::lowering::{Lowered, Lowering};
use crate::content::target::Target;

impl ContentCache {
    /// Lowers an image item drawing the image of `image` at `target`, appending one colour
    /// sprite and its tile to `out`.
    ///
    /// The sprite's tile is the item's source rectangle on the image's page. On an axis that
    /// does not repeat, a source edge between texels is widened to the next texel, and the bounds
    /// are widened with it so every texel stays where the item places it. The frame is then cut to
    /// the item's bounds on that axis, so the sprite draws nothing outside them. A mask ignores
    /// `GRAYSCALE`.
    ///
    /// # Errors
    ///
    /// Fails as [`ContentCache::image_tile`] does.
    pub fn lower_image(
        &mut self,
        images: &ImageStore,
        image: ImageKey,
        item: &Image,
        target: &Target,
        out: &mut Lowering,
    ) -> Result<(), ContentError> {
        let placed = self.image_tile(images, image)?;
        let [ox, oy] = target.offset;
        let placed_bounds = [
            item.bounds[0] + ox,
            item.bounds[1] + oy,
            item.bounds[2],
            item.bounds[3],
        ];
        let mut bounds = placed_bounds;
        let mut frame = [
            item.frame[0] + ox,
            item.frame[1] + oy,
            item.frame[2],
            item.frame[3],
        ];
        let repeat = item.extend.map(|extend| extend == Extend::REPEAT);
        let rect = source_rect(item.source, placed, repeat, &mut bounds);

        let mut radii = item.radii;
        for axis in 0..2 {
            if bounds[axis] != placed_bounds[axis] || bounds[2 + axis] != placed_bounds[2 + axis] {
                cut_frame(&mut frame, &mut radii, placed_bounds, axis);
            }
        }

        let mut flags = SpriteFlags::default();
        let mask = item.flags.contains(ImageFlags::MASK);
        if mask {
            flags |= SpriteFlags::MASK;
        } else if item.flags.contains(ImageFlags::GRAYSCALE) {
            flags |= SpriteFlags::GRAYSCALE;
        }
        if repeat[0] {
            flags |= SpriteFlags::REPEAT_X;
        }
        if repeat[1] {
            flags |= SpriteFlags::REPEAT_Y;
        }
        if item.filter == ImageFilter::NEAREST {
            flags |= SpriteFlags::NEAREST;
        }

        out.items.push(Lowered::Color(ColorSprite {
            bounds,
            frame,
            radii,
            shapes: item.shapes,
            tile: AtlasTile {
                page: placed.tile.page,
                rect,
            },
            paint: item.paint,
            paint_origin: target.offset,
            opacity: item.opacity.clamp(0.0, 1.0),
            clip: target.clip,
            space: target.space,
            flags,
        }));
        out.tiles.push(placed.key);
        Ok(())
    }
}

/// Returns the page rectangle of the image texels `source`, widening `bounds` on each axis that
/// does not repeat by what rounding the source out to whole tile texels added.
fn source_rect(
    source: [f32; 4],
    placed: ImageTileRef,
    repeat: [bool; 2],
    bounds: &mut [f32; 4],
) -> [u32; 4] {
    let mut rect = [0u32; 4];
    for axis in 0..2 {
        let limit = placed.tile.rect[2 + axis] as f32;
        let start = source[axis] * placed.scale;
        let extent = source[2 + axis] * placed.scale;
        let (low, high) = if repeat[axis] {
            let low = start.round().clamp(0.0, limit);
            (low, (start + extent).round().clamp(low, limit))
        } else {
            let low = start.floor().clamp(0.0, limit);
            (low, (start + extent).ceil().clamp(low, limit))
        };
        if !repeat[axis] && extent > 0.0 {
            let per_texel = bounds[2 + axis] / extent;
            bounds[axis] -= (start - low) * per_texel;
            bounds[2 + axis] = (high - low) * per_texel;
        }
        rect[axis] = placed.tile.rect[axis] + low as u32;
        rect[2 + axis] = (high - low) as u32;
    }
    rect
}
