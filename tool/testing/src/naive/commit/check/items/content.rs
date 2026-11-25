//! Glyph runs, images, external quads and backdrops.

use iceblink::{Backdrop, ExternalQuad, GlyphRun, Image, RejectReason};

use crate::naive::commit::check::finite::numbers;
use crate::naive::commit::check::items::range;
use crate::naive::commit::check::items::scope::Scope;
use crate::naive::commit::check::nodes::valid;

pub(super) fn glyph_run(scope: &Scope<'_>, run: &GlyphRun) -> Result<(), RejectReason> {
    numbers(&[run.size, run.embolden, run.skew])?;
    valid(run.flags.is_valid())?;
    scope.font(run.font)?;
    scope.paint(run.paint)?;
    scope.clip(run.clip)?;
    scope.space(run.space)?;

    let glyphs = scope
        .chunk
        .glyphs
        .get(range(run.glyphs.start, run.glyphs.len)?)
        .ok_or(RejectReason::InvalidItem)?;
    glyphs
        .iter()
        .try_for_each(|glyph| numbers(&[glyph.x, glyph.y]))?;

    let coords = range(run.coords.start, run.coords.len)?;
    valid(coords.end <= scope.chunk.coords.len())
}

pub(super) fn image(scope: &Scope<'_>, image: &Image) -> Result<(), RejectReason> {
    numbers(&image.bounds)?;
    numbers(&image.frame)?;
    numbers(&image.source)?;
    numbers(&image.radii)?;
    numbers(&image.shapes)?;
    numbers(&[image.opacity])?;
    valid(image.flags.is_valid())?;
    valid(image.extend.iter().all(|extend| extend.is_valid()))?;
    valid(image.filter.is_valid())?;
    scope.paint(image.paint)?;
    scope.image(image.image)?;
    scope.clip(image.clip)?;
    scope.space(image.space)
}

pub(super) fn external(scope: &Scope<'_>, quad: &ExternalQuad) -> Result<(), RejectReason> {
    numbers(&quad.bounds)?;
    numbers(&[quad.opacity])?;
    scope.texture(quad.texture)?;
    scope.clip(quad.clip)?;
    scope.space(quad.space)
}

pub(super) fn backdrop(scope: &Scope<'_>, backdrop: &Backdrop) -> Result<(), RejectReason> {
    numbers(&backdrop.bounds)?;
    valid((backdrop.filters as usize) < scope.chunk.filters.len())?;
    scope.clip(backdrop.clip)?;
    scope.space(backdrop.space)
}
