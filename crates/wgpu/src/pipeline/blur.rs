//! The blocks of the blur chain's passes.

use bytemuck::{Pod, Zeroable};
use iceblink::geom::{Device, Point, Rect, Size};

use crate::filter::EdgeMode;
use crate::target::scale::TargetScale;

/// The direction one pass of the blur chain blurs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlurAxis {
    Horizontal,
    Vertical,
}

impl BlurAxis {
    fn direction(self) -> [f32; 2] {
        match self {
            Self::Horizontal => [1.0, 0.0],
            Self::Vertical => [0.0, 1.0],
        }
    }
}

/// The uniform block of one pass of the blur chain.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct BlurParams {
    pub(crate) extents: [f32; 4],
    pub(crate) kernel: [f32; 4],
    pub(crate) sampling: [f32; 4],
    pub(crate) valid: [f32; 4],
    pub(crate) origins: [f32; 4],
    pub(crate) edge: [f32; 4],
}

impl BlurParams {
    /// The most taps an axis pass takes on each side of the centre.
    pub(crate) const MAX_TAPS: f32 = 16.0;

    /// Returns the block of the downsample from `source` at `source_scale` into the
    /// half-resolution `destination`, each given as extent and origin.
    ///
    /// Reads are clamped to the texels that hold the device pixels `written`.
    pub(crate) fn downsample(
        source: (Size<i32, Device>, Point<i32, Device>),
        source_scale: TargetScale,
        destination: (Size<i32, Device>, Point<i32, Device>),
        written: Rect<i32, Device>,
        edge: EdgeMode,
    ) -> Self {
        Self {
            extents: extents(source.0, destination.0),
            kernel: [0.0, 0.0, 0.0, 0.0],
            sampling: [source_scale.factor(), TargetScale::Half.factor(), 1.0, 1.0],
            valid: valid(written, source_scale, source.1),
            origins: origins(source.1, destination.1),
            edge: self::edge(edge),
        }
    }

    /// Returns the block of a pass along `axis` with the standard deviation `deviation`, from the
    /// first of `extents` and `origins` into the second.
    ///
    /// The taps spread out once three deviations take more than [`MAX_TAPS`](Self::MAX_TAPS).
    /// Reads are clamped to the texels that hold the device pixels `written`.
    pub(crate) fn axis(
        extents: [Size<i32, Device>; 2],
        scale: TargetScale,
        origins: [Point<i32, Device>; 2],
        axis: BlurAxis,
        deviation: f32,
        written: Rect<i32, Device>,
        edge: EdgeMode,
    ) -> Self {
        let texels = (deviation * scale.factor()).max(0.0);
        let reach = 3.0 * texels;
        let spacing = (reach / Self::MAX_TAPS).max(1.0);
        let taps = (reach / spacing).ceil().clamp(1.0, Self::MAX_TAPS);
        let direction = axis.direction();
        Self {
            extents: self::extents(extents[0], extents[1]),
            kernel: [direction[0], direction[1], texels, taps],
            sampling: [scale.factor(), scale.factor(), spacing, 0.0],
            valid: valid(written, scale, origins[0]),
            origins: self::origins(origins[0], origins[1]),
            edge: self::edge(edge),
        }
    }
}

/// Returns the texels that hold the device pixels `written` in a target at `scale` whose first
/// texel holds device pixel `origin`, as left, top, right and bottom.
fn valid(written: Rect<i32, Device>, scale: TargetScale, origin: Point<i32, Device>) -> [f32; 4] {
    let texels = scale.texels_at(written, origin);
    [
        texels.left() as f32,
        texels.top() as f32,
        texels.right() as f32,
        texels.bottom() as f32,
    ]
}

fn edge(edge: EdgeMode) -> [f32; 4] {
    match edge {
        EdgeMode::Transparent => [1.0, 0.0, 0.0, 0.0],
        EdgeMode::Duplicate => [0.0; 4],
    }
}

fn origins(source: Point<i32, Device>, destination: Point<i32, Device>) -> [f32; 4] {
    [
        source.x as f32,
        source.y as f32,
        destination.x as f32,
        destination.y as f32,
    ]
}

fn extents(source: Size<i32, Device>, destination: Size<i32, Device>) -> [f32; 4] {
    [
        source.width.max(1) as f32,
        source.height.max(1) as f32,
        destination.width.max(1) as f32,
        destination.height.max(1) as f32,
    ]
}
