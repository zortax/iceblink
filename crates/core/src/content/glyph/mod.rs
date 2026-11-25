//! Glyph runs: the glyph cache, the two text routes and parallel rasterisation.

pub(crate) mod answers;
pub(crate) mod color;
pub(crate) mod instances;
pub(crate) mod keys;
pub(crate) mod memo;
pub(crate) mod outlines;
pub(crate) mod raster;
pub(crate) mod route;
pub(crate) mod texels;

mod lower;
mod prefetch;

use std::sync::Arc;

use iceblink::PathClip;

use crate::content::glyph::answers::GlyphCache;
use crate::content::glyph::color::ColorGlyphs;
use crate::content::glyph::instances::Instances;
use crate::content::glyph::memo::KeyMemo;
use crate::content::glyph::outlines::OutlineCache;

/// Everything the cache knows about glyphs besides their tiles.
#[derive(Debug)]
pub(crate) struct GlyphState {
    pub(crate) answers: GlyphCache,
    pub(crate) outlines: OutlineCache,
    pub(crate) instances: Instances,
    pub(crate) colors: ColorGlyphs,
    /// The keys of the runs the last prefetch looked at.
    pub(crate) memo: KeyMemo,
    /// The empty clip list every glyph path shares.
    pub(crate) no_clips: Arc<[PathClip]>,
}

impl Default for GlyphState {
    fn default() -> Self {
        Self {
            answers: GlyphCache::default(),
            outlines: OutlineCache::default(),
            instances: Instances::default(),
            colors: ColorGlyphs::default(),
            memo: KeyMemo::default(),
            no_clips: Arc::from([]),
        }
    }
}

impl GlyphState {
    /// Forgets every answer, outline and colour answer.
    ///
    /// Interned variation coordinates stay, since glyph keys elsewhere may refer to them.
    pub(crate) fn clear(&mut self) {
        self.answers.clear();
        self.outlines.clear();
        self.colors.clear();
    }
}
