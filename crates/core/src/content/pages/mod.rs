//! Atlas pages as the renderer sees them: page numbers, texel formats and the page operations.

mod held;
mod replay;
mod sink;

use iceblink::PageFormat;
use iceblink_alloc::atlas::{TextureId, TextureKind};

pub(crate) use self::sink::PageSink;

/// Returns the page number of an atlas texture.
///
/// The number is unique among the textures that exist at one time. A texture destroyed and created
/// again gets the same number back.
pub(crate) const fn page_of(texture: TextureId) -> u32 {
    (texture.index << 2) | texture.kind.index() as u32
}

/// Returns the texel format of the pages of `kind`.
pub(crate) const fn format_of(kind: TextureKind) -> PageFormat {
    match kind {
        TextureKind::Mono => PageFormat::A8,
        TextureKind::Subpixel => PageFormat::SUBPIXEL,
        TextureKind::Color | TextureKind::Image => PageFormat::RGBA8_PREMULTIPLIED,
    }
}
