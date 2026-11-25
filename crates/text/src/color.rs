//! Whether a glyph is drawn from colour data.

use iceblink::FontData;
use skrifa::bitmap::{BitmapData, BitmapFormat, BitmapStrikes};
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// Reports whether `glyph` of `font` has colour layers, or a colour bitmap in some strike.
///
/// A font that cannot be read has no colour glyphs.
pub(crate) fn is_color(font: &FontData, glyph: u32) -> bool {
    let Ok(face) = FontRef::from_index(font.bytes(), font.index) else {
        return false;
    };
    let id = GlyphId::new(glyph);
    if face.color_glyphs().get(id).is_some() {
        return true;
    }

    [BitmapFormat::Sbix, BitmapFormat::Cbdt]
        .into_iter()
        .filter_map(|format| BitmapStrikes::with_format(&face, format))
        .any(|strikes| {
            strikes.iter().any(|strike| {
                strike
                    .get(id)
                    .is_some_and(|bitmap| !matches!(bitmap.data, BitmapData::Mask(_)))
            })
        })
}
