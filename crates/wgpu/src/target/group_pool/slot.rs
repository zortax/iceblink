//! A target lent from the pool.

use iceblink::geom::{Device, Point, Rect, Size};

use crate::target::scale::TargetScale;

/// A target lent from the pool for one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GroupSlot {
    pub(super) index: usize,
    pub(super) scale: TargetScale,
    pub(super) format: wgpu::TextureFormat,
    /// The device pixel the target's first texel holds.
    pub(super) origin: Point<i32, Device>,
    /// The size of the device region the lease covers.
    pub(super) covered: Size<i32, Device>,
    /// The texture's extent in texels.
    pub(super) allocated: Size<i32, Device>,
}

impl GroupSlot {
    pub(crate) fn origin(self) -> Point<i32, Device> {
        self.origin
    }

    pub(crate) fn covered(self) -> Size<i32, Device> {
        self.covered
    }

    /// Returns the texels the covered region takes.
    pub(crate) fn extent(self) -> Size<i32, Device> {
        self.scale.extent(self.covered)
    }

    pub(crate) fn allocated(self) -> Size<i32, Device> {
        self.allocated
    }

    /// Reports whether the texture holds the device pixels of `rect`.
    pub(crate) fn holds(self, rect: Rect<i32, Device>) -> bool {
        let pixels = match self.scale {
            TargetScale::Full => self.allocated,
            TargetScale::Half => Size::new(self.allocated.width * 2, self.allocated.height * 2),
        };
        Rect::new(self.origin, pixels).contains_rect(rect)
    }

    pub(crate) fn scale(self) -> TargetScale {
        self.scale
    }

    pub(crate) fn format(self) -> wgpu::TextureFormat {
        self.format
    }
}
