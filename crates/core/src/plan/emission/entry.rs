//! Entries of the emission stream.

use iceblink::DeviceRect;

use crate::plan::emission::Instance;

/// The cover index of an instance with no opaque cover.
pub(crate) const NO_COVER: u32 = u32::MAX;

/// One entry of the emission stream.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Entry {
    /// An instance, with its index into the emission's covers or [`NO_COVER`].
    Instance { instance: Instance, cover: u32 },
    /// The start of a block of the next `len` entries, which are all instances.
    Block { len: u32 },
    /// The path at `index` of `Frame::paths`.
    Path {
        index: u32,
        ink: DeviceRect,
        /// The pixels the outline covers before any clip.
        reach: DeviceRect,
        clip: u32,
    },
    /// The external quad at `index` of `Frame::externals`.
    External { index: u32, ink: DeviceRect },
    /// The backdrop at `index` of `Frame::backdrops`.
    Backdrop { index: u32, ink: DeviceRect },
    /// The start of the group at `index` of `Frame::groups`, whose filters read `reach` device
    /// pixels past what they write.
    GroupBegin {
        index: u32,
        bounds: DeviceRect,
        reach: i32,
    },
    /// The end of the innermost open group.
    GroupEnd,
}
