//! What one frame may spend on new masks of volatile paths.

/// The most new masks of volatile paths one frame admits.
// A volatile path rasterises again in most frames. Past this the frame declines the rest, which
// the path rasteriser draws.
const BUDGET_TILES: u32 = 256;

/// The most texels those new masks may cover together.
const BUDGET_TEXELS: u64 = 1024 * 1024;

/// What one frame has spent on new masks of volatile paths.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Budget {
    tiles: u32,
    texels: u64,
}

impl Budget {
    /// Spends `tiles` more masks covering `texels`, and reports whether they stay within
    /// [`BUDGET_TILES`] masks and [`BUDGET_TEXELS`] texels.
    ///
    /// Nothing is spent when they do not.
    pub(super) fn spend(&mut self, tiles: u32, texels: u64) -> bool {
        let admitted = self.tiles + tiles <= BUDGET_TILES && self.texels + texels <= BUDGET_TEXELS;
        if admitted {
            self.tiles += tiles;
            self.texels += texels;
        }
        admitted
    }
}
