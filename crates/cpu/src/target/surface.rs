//! A mutable view of target rows at one texel per device pixel, which instances draw into.

use iceblink::BlendMode;
use iceblink::geom::{Device, Point, Rect, Size};

use crate::math::blend::blend;
use crate::target::texture::{Texels, Texture};
use crate::target::{Precision, from_byte, to_byte};

/// The texels of a surface.
#[derive(Debug)]
pub(crate) enum TexelsMut<'a> {
    /// Eight bits per channel, rounded after every write.
    Byte(&'a mut [[u8; 4]]),
    /// A float per channel.
    Float(&'a mut [[f32; 4]]),
}

/// Rows of a target, one texel per device pixel, at a device position.
#[derive(Debug)]
pub(crate) struct Surface<'a> {
    origin: Point<i32, Device>,
    width: usize,
    height: usize,
    texels: TexelsMut<'a>,
}

impl<'a> Surface<'a> {
    /// Returns a surface over `texels`, which hold `height` rows of `width` texels.
    pub(crate) fn new(
        origin: Point<i32, Device>,
        width: usize,
        height: usize,
        texels: TexelsMut<'a>,
    ) -> Self {
        Self {
            origin,
            width,
            height,
            texels,
        }
    }

    /// Returns the device pixels the surface holds.
    pub(crate) fn extent(&self) -> Rect<i32, Device> {
        Rect::new(
            self.origin,
            Size::new(self.width as i32, self.height as i32),
        )
    }

    /// Returns how many bits the surface holds per channel.
    pub(crate) fn precision(&self) -> Precision {
        match self.texels {
            TexelsMut::Byte(_) => Precision::Byte,
            TexelsMut::Float(_) => Precision::Float,
        }
    }

    /// Returns the surface as a texture, for reading what it holds.
    pub(crate) fn texture(&self) -> Texture<'_> {
        let texels = match &self.texels {
            TexelsMut::Byte(texels) => Texels::Byte(texels),
            TexelsMut::Float(texels) => Texels::Float(texels),
        };
        Texture::new(self.origin, self.width, self.height, false, texels)
    }

    /// Returns the surface for the same rows, borrowed again.
    pub(crate) fn reborrow(&mut self) -> Surface<'_> {
        let texels = match &mut self.texels {
            TexelsMut::Byte(texels) => TexelsMut::Byte(texels),
            TexelsMut::Float(texels) => TexelsMut::Float(texels),
        };
        Surface::new(self.origin, self.width, self.height, texels)
    }

    /// Splits the surface into the rows above device row `y` and the rows from it on.
    pub(crate) fn split_rows(self, y: i32) -> (Surface<'a>, Surface<'a>) {
        let rows = (y - self.origin.y).clamp(0, self.height as i32) as usize;
        let at = rows * self.width;
        let (top, bottom) = match self.texels {
            TexelsMut::Byte(texels) => {
                let (top, bottom) = texels.split_at_mut(at);
                (TexelsMut::Byte(top), TexelsMut::Byte(bottom))
            }
            TexelsMut::Float(texels) => {
                let (top, bottom) = texels.split_at_mut(at);
                (TexelsMut::Float(top), TexelsMut::Float(bottom))
            }
        };
        let lower = Point::new(self.origin.x, self.origin.y + rows as i32);
        (
            Surface::new(self.origin, self.width, rows, top),
            Surface::new(lower, self.width, self.height - rows, bottom),
        )
    }

    /// Clears `rect` to transparent.
    pub(crate) fn clear(&mut self, rect: Rect<i32, Device>) {
        let Some((rows, columns)) = self.span_of(rect) else {
            return;
        };
        for row in rows {
            let range = row * self.width + columns.start..row * self.width + columns.end;
            match &mut self.texels {
                TexelsMut::Byte(texels) => texels[range].fill([0; 4]),
                TexelsMut::Float(texels) => texels[range].fill([0.0; 4]),
            }
        }
    }

    /// Draws `sources` over device row `y` from column `x`, premultiplied, by source-over.
    pub(crate) fn over_row(&mut self, x: i32, y: i32, sources: &[[f32; 4]]) {
        self.each(x, y, sources, true, true, |below, source| {
            let keep = 1.0 - source[3];
            [
                source[0] + below[0] * keep,
                source[1] + below[1] * keep,
                source[2] + below[2] * keep,
                source[3] + below[3] * keep,
            ]
        });
    }

    /// Blends `sources` onto device row `y` from column `x` with `mode`.
    pub(crate) fn blend_row(&mut self, x: i32, y: i32, sources: &[[f32; 4]], mode: BlendMode) {
        if mode == BlendMode::NORMAL {
            self.over_row(x, y, sources);
            return;
        }
        self.each(x, y, sources, true, false, |below, source| {
            blend(mode, below, source)
        });
    }

    /// Replaces device row `y` from column `x` with `sources`.
    pub(crate) fn replace_row(&mut self, x: i32, y: i32, sources: &[[f32; 4]]) {
        self.each(x, y, sources, false, false, |_, source| source);
    }

    /// Draws straight colours with a coverage per channel onto device row `y` from column `x`,
    /// leaving alpha as it is.
    pub(crate) fn subpixel_row(
        &mut self,
        x: i32,
        y: i32,
        sources: &[[f32; 4]],
        coverage: &[[f32; 3]],
    ) {
        let mut index = 0;
        self.each(x, y, sources, false, false, |below, source| {
            let factor = coverage[index];
            index += 1;
            [
                source[0] * factor[0] + below[0] * (1.0 - factor[0]),
                source[1] * factor[1] + below[1] * (1.0 - factor[1]),
                source[2] * factor[2] + below[2] * (1.0 - factor[2]),
                below[3],
            ]
        });
    }

    // Calls `apply` with each texel of row `y` from column `x` and its source, and stores what
    // it returns. Columns outside the surface are skipped, and so are transparent sources when
    // `transparent_keeps` says they change nothing. An opaque source is stored as it is into a
    // byte texel when `opaque_replaces` says it hides what is beneath.
    fn each(
        &mut self,
        x: i32,
        y: i32,
        sources: &[[f32; 4]],
        transparent_keeps: bool,
        opaque_replaces: bool,
        mut apply: impl FnMut([f32; 4], [f32; 4]) -> [f32; 4],
    ) {
        let row = y - self.origin.y;
        if row < 0 || row as usize >= self.height {
            return;
        }
        let start = x - self.origin.x;
        let skip = (-start).max(0) as usize;
        let first = start.max(0) as usize;
        let count = sources
            .len()
            .saturating_sub(skip)
            .min(self.width.saturating_sub(first));
        let base = row as usize * self.width + first;
        let sources = &sources[skip..skip + count];
        match &mut self.texels {
            TexelsMut::Byte(texels) => {
                for (texel, source) in texels[base..base + count].iter_mut().zip(sources) {
                    if transparent_keeps && *source == [0.0; 4] {
                        continue;
                    }
                    if opaque_replaces && source[3] >= 1.0 {
                        *texel = source.map(to_byte);
                        continue;
                    }
                    let below = texel.map(from_byte);
                    *texel = apply(below, *source).map(to_byte);
                }
            }
            TexelsMut::Float(texels) => {
                for (texel, source) in texels[base..base + count].iter_mut().zip(sources) {
                    if transparent_keeps && *source == [0.0; 4] {
                        continue;
                    }
                    *texel = apply(*texel, *source).map(|channel| channel.clamp(0.0, 1.0));
                }
            }
        }
    }

    // The surface rows and columns `rect` covers, as indices.
    fn span_of(
        &self,
        rect: Rect<i32, Device>,
    ) -> Option<(core::ops::Range<usize>, core::ops::Range<usize>)> {
        let cut = rect.intersection(self.extent())?;
        let left = (cut.left() - self.origin.x) as usize;
        let top = (cut.top() - self.origin.y) as usize;
        Some((
            top..top + cut.height() as usize,
            left..left + cut.width() as usize,
        ))
    }
}
