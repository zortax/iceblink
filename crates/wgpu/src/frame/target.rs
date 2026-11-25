//! The targets a planned pass draws into.

use iceblink::geom::{Device, Point, Rect, Size};

use crate::target::group_pool::GroupSlot;
use crate::target::scale::TargetScale;

/// A target a pass draws into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetRef {
    /// The target the frame is composed into.
    Composed,
    /// A lent group target.
    Pool(GroupSlot),
    /// A mark bin page.
    MarksPage(u32),
}

impl TargetRef {
    /// Returns the resolution the target holds its pixels at.
    pub(crate) fn scale(self) -> TargetScale {
        match self {
            Self::Composed | Self::MarksPage(_) => TargetScale::Full,
            Self::Pool(slot) => slot.scale(),
        }
    }

    /// Returns the device point the target's first texel holds.
    pub(crate) fn origin(self) -> Point<i32, Device> {
        match self {
            Self::Composed | Self::MarksPage(_) => Point::ORIGIN,
            Self::Pool(slot) => slot.origin(),
        }
    }

    /// Returns the pool slot of a lent group target.
    pub(crate) fn slot(self) -> Option<GroupSlot> {
        match self {
            Self::Composed | Self::MarksPage(_) => None,
            Self::Pool(slot) => Some(slot),
        }
    }
}

/// The target a frame is composed into: the kept target, or the presented image in buffer-age
/// mode.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Composed<'a> {
    pub(crate) texture: &'a wgpu::Texture,
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) format: wgpu::TextureFormat,
    /// The pixels in use, from the origin.
    pub(crate) used: Rect<i32, Device>,
}

impl Composed<'_> {
    /// Returns the size the texture is allocated at.
    pub(crate) fn allocated(&self) -> Size<i32, Device> {
        Size::new(self.texture.width() as i32, self.texture.height() as i32)
    }
}
