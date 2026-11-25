//! The glyph keys a prefetch worked out, kept for the lowering of the same runs.

use core::ops::Range;

use iceblink::{FontId, Glyph, GlyphKey};

use crate::content::glyph::keys::{RunKeys, Spot};

/// The most remembered runs one search looks at.
const SEARCHED: usize = 64;

/// What one glyph of a run on the atlas route lowers from.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Drawn {
    /// A colour glyph of a run that draws colour glyphs as nothing.
    Nothing,
    /// A colour glyph, drawn from the tile of the key at the spot.
    Color(GlyphKey, Spot),
    /// Another glyph, drawn from the tile of the key at the spot.
    Plain(GlyphKey, Spot),
}

/// The keys and spots of the glyphs of the runs the last prefetch looked at.
///
/// A run is found again only with the same font, run keys and glyphs, so the keys and spots found
/// equal the ones computed afresh.
#[derive(Debug, Default)]
pub(crate) struct KeyMemo {
    runs: Vec<Remembered>,
    glyphs: Vec<(Glyph, Drawn)>,
    /// The run after the one found last, which a lowering in prefetch order asks for next.
    next: usize,
}

/// One remembered run.
#[derive(Debug)]
struct Remembered {
    font: FontId,
    keys: RunKeys,
    glyphs: Range<usize>,
}

impl KeyMemo {
    /// Forgets every run.
    pub(crate) fn clear(&mut self) {
        self.runs.clear();
        self.glyphs.clear();
        self.next = 0;
    }

    /// Returns where the glyphs of the next run to remember start.
    pub(crate) fn start(&self) -> usize {
        self.glyphs.len()
    }

    /// Adds what `glyph` of the run being remembered lowers from.
    pub(crate) fn push(&mut self, glyph: Glyph, drawn: Drawn) {
        self.glyphs.push((glyph, drawn));
    }

    /// Remembers the run of `font` with `keys` whose glyphs were added since `start`.
    pub(crate) fn finish(&mut self, font: FontId, keys: RunKeys, start: usize) {
        self.runs.push(Remembered {
            font,
            keys,
            glyphs: start..self.glyphs.len(),
        });
    }

    /// Returns the positions of what the glyphs of a run of `font` with `keys` lower from, when
    /// such a run with the same glyphs is remembered.
    ///
    /// The search starts after the run found last and looks at 64 runs at most.
    pub(crate) fn find(
        &mut self,
        font: FontId,
        keys: &RunKeys,
        glyphs: &[Glyph],
    ) -> Option<Range<usize>> {
        let same = |run: &Remembered| {
            run.font == font
                && run.keys == *keys
                && run.glyphs.len() == glyphs.len()
                && self.glyphs[run.glyphs.clone()]
                    .iter()
                    .zip(glyphs)
                    .all(|((held, _), glyph)| held == glyph)
        };

        // Lowerings mostly ask in prefetch order, and a long memo is only searched from there.
        let found = (self.next..self.runs.len())
            .chain(0..self.next.min(self.runs.len()))
            .take(SEARCHED)
            .find(|&index| same(&self.runs[index]))?;
        self.next = found + 1;
        Some(self.runs[found].glyphs.clone())
    }

    /// Returns what the remembered glyph at `position` lowers from.
    pub(crate) fn drawn(&self, position: usize) -> Drawn {
        self.glyphs[position].1
    }
}
