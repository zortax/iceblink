//! The frame instances items lower into.

use iceblink::{ColorSprite, FramePath, Sprite};

use crate::content::key::TileKey;

/// One lowered instance.
#[derive(Clone, Debug)]
pub enum Lowered {
    /// A coverage sprite with one coverage per pixel.
    Mono(Sprite),
    /// A coverage sprite with one coverage per colour stripe.
    Subpixel(Sprite),
    /// A full-colour sprite: a colour glyph or an image.
    Color(ColorSprite),
    /// A filled or stroked path.
    Path(FramePath),
}

/// The instances one or more items lowered into, and the tiles those instances read.
///
/// Lowering appends, so one value can collect a whole chunk. Instances keep the paint order of
/// the items and of the glyphs within a run.
#[derive(Clone, Debug, Default)]
pub struct Lowering {
    /// The instances, in paint order.
    pub items: Vec<Lowered>,
    /// The tiles the instances read, one entry per instance that reads one.
    pub tiles: Vec<TileKey>,
}

impl Lowering {
    /// Empties both lists, keeping the allocations.
    pub fn clear(&mut self) {
        self.items.clear();
        self.tiles.clear();
    }
}
