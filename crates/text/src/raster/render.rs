//! The swash render configured for one glyph key.

use iceblink::{GlyphKey, PHASES, RasterStyle};
use swash::scale::outline::Outline;
use swash::scale::{Render, Source, StrikeWith};
use swash::zeno::{Angle, Transform, Vector};

/// The sources of a mono or subpixel bitmap, in order of preference.
///
/// A face that ships only bitmap strikes draws from them.
const MASK_SOURCES: [Source; 2] = [Source::Outline, Source::Bitmap(StrikeWith::BestFit)];

/// The sources a bitmap of `key` draws from, in order of preference.
///
/// A colour bitmap takes the face's colour layers in the key's palette, then a colour strike, then
/// the plain outline.
pub(super) fn sources(key: &GlyphKey) -> SourceList {
    match key.style {
        RasterStyle::Color => SourceList::Color([
            Source::ColorOutline(key.palette),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ]),
        RasterStyle::Mono | RasterStyle::Subpixel => SourceList::Mask(MASK_SOURCES),
    }
}

/// The sources of one render.
pub(super) enum SourceList {
    Mask([Source; 2]),
    Color([Source; 3]),
}

impl SourceList {
    fn as_slice(&self) -> &[Source] {
        match self {
            Self::Mask(sources) => sources,
            Self::Color(sources) => sources,
        }
    }
}

/// Returns a render over `sources` with the phase offset and the slant of `key`.
pub(super) fn render<'a>(key: &GlyphKey, sources: &'a SourceList) -> Render<'a> {
    let mut render = Render::new(sources.as_slice());
    render.offset(Vector::new(phase(key), 0.0));

    let skew = key.skew();
    if skew != 0.0 {
        render.transform(Some(shear(skew)));
    }
    render
}

/// Slants `outline` as the render of `key` does.
pub(super) fn slant(key: &GlyphKey, outline: &mut Outline) {
    let skew = key.skew();
    if skew != 0.0 {
        outline.transform(&shear(skew));
    }
}

/// Returns the horizontal phase of `key`, in pixels.
pub(super) fn phase(key: &GlyphKey) -> f32 {
    f32::from(key.phase % PHASES) / f32::from(PHASES)
}

/// Returns the shear that leans a glyph forwards by `skew` degrees.
fn shear(skew: f32) -> Transform {
    Transform::skew(Angle::from_degrees(skew), Angle::ZERO)
}
