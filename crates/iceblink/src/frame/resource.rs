//! Atlas pages and shaders a renderer creates, writes and destroys.

use std::sync::Arc;

use crate::bits::enumeration;
use crate::geometry::DeviceRect;
use crate::resource::ShaderDesc;

/// The page and shader operations of a frame, which the renderer applies in order before it
/// draws.
#[derive(Clone, Debug, Default)]
pub struct ResourceWork {
    /// Atlas page operations.
    pub pages: Vec<PageOp>,
    /// Shader operations.
    pub shaders: Vec<ShaderOp>,
}

impl ResourceWork {
    /// Empties both lists, keeping the allocations.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.shaders.clear();
    }

    /// Reports whether the frame has no resource work.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() && self.shaders.is_empty()
    }
}

/// An operation on an atlas page.
///
/// Pages are never larger than the renderer's `max_texture_size`.
///
/// A page has one or more levels of detail. Level 0 has the page's size, and each further level
/// is half the size of the one before, rounded down and at least one texel. A renderer samples
/// the levels of a page for colour sprites drawn smaller than their tile.
#[derive(Clone, Debug)]
pub enum PageOp {
    /// To the renderer: create a page of transparent texels.
    Create {
        /// The page to create.
        page: u32,
        /// The texel format.
        format: PageFormat,
        /// The width and height of level 0, in texels.
        size: [u32; 2],
        /// The number of levels, at least 1.
        levels: u32,
    },
    /// To the renderer: replace the texels of a rectangle of one level of a page.
    Upload {
        /// The page written.
        page: u32,
        /// The level written.
        level: u32,
        /// The texels written, in texels of the level.
        rect: DeviceRect,
        /// The bytes the texels are read from.
        bytes: Arc<[u8]>,
        /// Where the first texel starts in `bytes`.
        offset: usize,
        /// Bytes from one row to the next.
        stride: u32,
    },
    /// To the renderer: destroy a page.
    Destroy {
        /// The page to destroy.
        page: u32,
    },
}

impl PageOp {
    /// Returns the width and height of level `level` of a page whose level 0 is `size`.
    pub const fn level_size(size: [u32; 2], level: u32) -> [u32; 2] {
        let shift = if level < 32 { level } else { 31 };
        let width = size[0] >> shift;
        let height = size[1] >> shift;
        [
            if width == 0 { 1 } else { width },
            if height == 0 { 1 } else { height },
        ]
    }
}

enumeration! {
    /// The texel format of an atlas page.
    pub struct PageFormat(u8) {
        /// One byte per texel: coverage or alpha.
        A8 = 0,
        /// Four bytes per texel: premultiplied gamma-encoded sRGB.
        RGBA8_PREMULTIPLIED = 1,
        /// Four bytes per texel: the coverage of the three colour stripes from left to right, and
        /// an unused byte.
        SUBPIXEL = 2,
    }
}

/// An operation on a frame shader slot.
#[derive(Clone, Debug)]
pub enum ShaderOp {
    /// To the renderer: create the shader of a slot.
    Create {
        /// The frame shader slot.
        shader: u32,
        /// The shader.
        desc: Arc<ShaderDesc>,
    },
    /// To the renderer: destroy the shader of a slot.
    Destroy {
        /// The frame shader slot.
        shader: u32,
    },
}
