//! Rasterised glyphs on atlas pages, and glyph outlines.

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::GlyphRaster;
use iceblink::kurbo::BezPath;
use iceblink::{
    AtlasTile, FontData, FontId, GlyphBitmap, GlyphKey, GlyphPlacement, OutlineKey, PageFormat,
    RasterStyle,
};

use crate::naive::content::pages::{PageUse, Pages};

/// The result of rasterising a glyph.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Raster {
    /// The font has no such glyph, or its bitmap fits no page, so it draws nothing.
    Missing,
    /// The glyph has no pixels, such as a space.
    Blank,
    /// The glyph's bitmap on a page, and where its top-left corner lies from the glyph origin.
    Tile(AtlasTile, GlyphPlacement),
}

/// Glyph tiles and outlines, kept until their page is evicted or the caches are dropped.
#[derive(Default)]
pub(crate) struct Glyphs {
    tiles: HashMap<GlyphKey, Raster>,
    outlines: HashMap<OutlineKey, Option<Arc<BezPath>>>,
    /// The id of each set of variation coordinates of a font. Zero is the default instance.
    instances: HashMap<(FontId, Vec<i16>), u32>,
}

impl Glyphs {
    /// Returns the id of the variation coordinates `coords` of `font`.
    pub(crate) fn instance(&mut self, font: FontId, coords: &[i16]) -> u32 {
        if coords.is_empty() {
            return 0;
        }

        let next = self.instances.len() as u32 + 1;
        *self
            .instances
            .entry((font, coords.to_vec()))
            .or_insert(next)
    }

    /// Returns the tile of `key`, rasterising it onto a page of `side` texels the first time.
    pub(crate) fn raster(
        &mut self,
        pages: &mut Pages,
        side: u32,
        raster: &dyn GlyphRaster,
        font: &FontData,
        key: &GlyphKey,
        coords: &[i16],
    ) -> Raster {
        if let Some(known) = self.tiles.get(key) {
            return *known;
        }

        let mut bitmap = GlyphBitmap::default();
        let result = match raster.rasterize(font, key, coords, &mut bitmap) {
            None => Raster::Missing,
            Some(_) if bitmap.is_empty() => Raster::Blank,
            Some(_) if !bitmap.is_well_formed(key.style) => Raster::Missing,
            Some(placement) => place(pages, side, key.style, &bitmap, placement),
        };

        self.tiles.insert(*key, result);
        result
    }

    /// Returns the outline of `key`, or `None` when the font has no outline for it.
    pub(crate) fn outline(
        &mut self,
        raster: &dyn GlyphRaster,
        font: &FontData,
        key: &OutlineKey,
        coords: &[i16],
    ) -> Option<Arc<BezPath>> {
        self.outlines
            .entry(*key)
            .or_insert_with(|| {
                let mut path = BezPath::new();
                raster
                    .outline(font, key, coords, &mut path)
                    .then(|| Arc::new(path))
            })
            .clone()
    }

    /// Forgets every tile on `page`.
    pub(crate) fn evict(&mut self, page: u32) {
        self.tiles
            .retain(|_, raster| !matches!(raster, Raster::Tile(tile, _) if tile.page == page));
    }
}

/// Copies a glyph bitmap onto a page of the format its style reads.
fn place(
    pages: &mut Pages,
    side: u32,
    style: RasterStyle,
    bitmap: &GlyphBitmap,
    placement: GlyphPlacement,
) -> Raster {
    let format = match style {
        RasterStyle::Mono => PageFormat::A8,
        RasterStyle::Subpixel => PageFormat::SUBPIXEL,
        RasterStyle::Color => PageFormat::RGBA8_PREMULTIPLIED,
    };

    let size = [bitmap.width, bitmap.height];
    let Some((page, at)) = pages.place(format, PageUse::Glyphs, side, size) else {
        return Raster::Missing;
    };

    let texels = texels(style, &bitmap.bytes);
    pages.write(page, at, bitmap.width, &texels);
    let tile = AtlasTile {
        page,
        rect: [at[0], at[1], bitmap.width, bitmap.height],
    };
    Raster::Tile(tile, placement)
}

/// Converts bitmap bytes to the texels of the page format of `style`.
fn texels(style: RasterStyle, bytes: &[u8]) -> Vec<u8> {
    match style {
        RasterStyle::Mono => bytes.to_vec(),
        RasterStyle::Subpixel => bytes
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|stripes| [stripes[0], stripes[1], stripes[2], 0])
            .collect(),
        // Colour bitmaps are straight alpha, and pages hold premultiplied texels.
        RasterStyle::Color => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|texel| {
                let alpha = u32::from(texel[3]);
                let scale = |channel: u8| ((u32::from(channel) * alpha + 127) / 255) as u8;
                [scale(texel[0]), scale(texel[1]), scale(texel[2]), texel[3]]
            })
            .collect(),
    }
}
