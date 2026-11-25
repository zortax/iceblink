//! Glyphs drawn from atlas tiles.

use iceblink::{
    AtlasTile, BatchKind, ColorSprite, Glyph, GlyphFlags, GlyphKey, GlyphPlacement, Matrix4,
    PenPosition, ROUND, RasterStyle, Sprite, SpriteFlags,
};

use crate::naive::content::{Content, Raster};
use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::{self, Placed};
use crate::naive::frame::lower::glyphs::{Run, origin};
use crate::naive::geometry::{Bounds, plane_affine};

/// Draws a glyph from a tile of `style`, and returns its device ink.
///
/// Under a uniform positive scale the glyph is rasterised at its device size and phase and lands
/// on whole device pixels. Under any other matrix it is rasterised at its em size with phase
/// zero, placed at its rounded local origin, and resampled through the matrix.
pub(super) fn tile(
    builder: &mut Builder<'_>,
    placed: &Placed,
    run: &Run<'_>,
    glyph: &Glyph,
    style: RasterStyle,
) -> Bounds {
    let (size, phase, position) = Position::of(placed, run, glyph);
    let key = key(run, glyph, size, phase, style);

    let raster = builder
        .env
        .raster
        .expect("glyph runs lower only with a rasteriser");
    let side = Content::shared_side(builder.env.caps.max_texture_size);
    let content = &mut *builder.content;
    let Raster::Tile(tile, placement) = content.glyphs.raster(
        &mut content.pages,
        side,
        raster,
        &run.font,
        &key,
        run.coords,
    ) else {
        return Bounds::EMPTY;
    };

    let (bounds, device) = position.bounds(tile, placement, &placed.world);
    let ink = placed.cut(device.unwrap_or(builder.env.output));
    if !item::shows(builder, ink) {
        return ink;
    }

    builder.pages.insert(tile.page);
    push(builder, placed, run, tile, bounds, style);
    ink
}

/// Returns the cache key of a glyph rasterised at `size` and `phase` in `style`.
fn key(run: &Run<'_>, glyph: &Glyph, size: f32, phase: u8, style: RasterStyle) -> GlyphKey {
    GlyphKey {
        font: run.run.font,
        glyph: glyph.id,
        size_bits: size.to_bits(),
        instance: run.instance,
        phase,
        style,
        hint: run.run.flags.contains(GlyphFlags::HINT),
        palette: if style == RasterStyle::Color {
            run.run.palette
        } else {
            0
        },
        embolden_bits: run.run.embolden.to_bits(),
        skew_bits: run.run.skew.to_bits(),
    }
}

/// Appends a sprite of `style` with local `bounds` to the lane of its style.
fn push(
    builder: &mut Builder<'_>,
    placed: &Placed,
    run: &Run<'_>,
    tile: AtlasTile,
    bounds: [f32; 4],
    style: RasterStyle,
) {
    if style == RasterStyle::Color {
        let sprite = ColorSprite {
            bounds,
            frame: bounds,
            radii: [0.0; 8],
            shapes: ROUND,
            tile,
            paint: run.paint,
            paint_origin: placed.offset,
            opacity: 1.0,
            clip: placed.clip,
            space: placed.space,
            flags: SpriteFlags::PAINT_ALPHA,
        };
        builder.instance(
            |lanes| &mut lanes.color,
            sprite,
            BatchKind::Color { page: tile.page },
        );
        return;
    }

    let sprite = Sprite {
        bounds,
        paint: run.paint,
        paint_origin: placed.offset,
        tile,
        clip: placed.clip,
        space: placed.space,
        _pad: 0,
    };
    if style == RasterStyle::Subpixel {
        builder.instance(
            |lanes| &mut lanes.subpixel,
            sprite,
            BatchKind::Subpixel { page: tile.page },
        );
    } else {
        builder.instance(
            |lanes| &mut lanes.mono,
            sprite,
            BatchKind::Mono { page: tile.page },
        );
    }
}

/// Where a glyph's bitmap is placed.
enum Position {
    /// On whole device pixels, from the glyph origin's device position.
    Device {
        pen: PenPosition,
        device: [f32; 2],
        scale: f32,
        translation: [f32; 2],
    },
    /// In local space, at the glyph origin rounded to whole local units.
    Local([f32; 2]),
}

impl Position {
    /// Returns the size and phase a glyph is rasterised at, and where its bitmap is placed.
    fn of(placed: &Placed, run: &Run<'_>, glyph: &Glyph) -> (f32, u8, Self) {
        let [x, y] = origin(placed, glyph);
        let Some(scale) = run.scale else {
            return (run.run.size, 0, Self::Local([x.round(), y.round()]));
        };

        let [.., tx, ty] = plane_affine(&placed.world)
            .expect("a uniform scale is affine")
            .as_coeffs()
            .map(|coefficient| coefficient as f32);
        let device = [scale * x + tx, scale * y + ty];
        let pen = PenPosition::of(device[0]);
        let position = Self::Device {
            pen,
            device,
            scale,
            translation: [tx, ty],
        };
        (run.run.size * scale, pen.phase(), position)
    }

    /// Returns the local bounds of a bitmap of `tile` placed here, and its device box, or `None`
    /// when the box cannot be mapped to the device.
    fn bounds(
        self,
        tile: AtlasTile,
        placement: GlyphPlacement,
        world: &Matrix4,
    ) -> ([f32; 4], Option<Bounds>) {
        let [width, height] = [tile.rect[2] as f32, tile.rect[3] as f32];
        match self {
            Self::Device {
                pen,
                device,
                scale,
                translation,
            } => {
                let [left, top] = pen.bitmap_origin(device[1], placement);
                let device = Bounds {
                    x0: left,
                    y0: top,
                    x1: left + width,
                    y1: top + height,
                };
                let bounds = [
                    (left - translation[0]) / scale,
                    (top - translation[1]) / scale,
                    width / scale,
                    height / scale,
                ];
                (bounds, Some(device))
            }
            Self::Local([x, y]) => {
                let bounds = [
                    x + placement.left as f32,
                    y - placement.top as f32,
                    width,
                    height,
                ];
                (bounds, Bounds::of(bounds).mapped(world))
            }
        }
    }
}
