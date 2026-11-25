//! The GPU records of the frame tables, as the shaders read them.

use bytemuck::{Pod, Zeroable};

/// What `GpuClip::space` holds when no link is kept in a space of its own.
pub(crate) const NO_SPACE: u32 = u32::MAX;

/// A box no point lies outside of.
pub(crate) const UNBOUNDED: [f32; 4] = [-1.0e9, -1.0e9, 2.0e9, 2.0e9];

/// One rounded-rectangle test of a clip.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuRounded {
    pub(crate) rect: [f32; 4],
    pub(crate) radii: [f32; 8],
    pub(crate) shapes: [f32; 4],
}

/// One clip: a device box, up to two rounded tests, a kept space and a range of general links.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuClip {
    /// The device box.
    pub(crate) aabb: [f32; 4],
    pub(crate) first: GpuRounded,
    pub(crate) second: GpuRounded,
    /// The box in the coordinates of the kept space.
    pub(crate) kept_aabb: [f32; 4],
    /// How many of the two rounded tests apply.
    pub(crate) count: u32,
    /// The kept space, or [`NO_SPACE`].
    pub(crate) space: u32,
    /// One bit per rounded test that is in the kept space.
    pub(crate) kept_tests: u32,
    /// The index of the first general link plus one, or zero without general links.
    pub(crate) links: u32,
}

impl Default for GpuClip {
    fn default() -> Self {
        Self {
            aabb: UNBOUNDED,
            first: GpuRounded::default(),
            second: GpuRounded::default(),
            kept_aabb: UNBOUNDED,
            count: 0,
            space: NO_SPACE,
            kept_tests: 0,
            links: 0,
        }
    }
}

/// A general link, evaluated in its own space with the space's current matrix.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuClipLink {
    /// One of the [`link`] kinds.
    pub(crate) kind: u32,
    pub(crate) space: u32,
    /// 1 for a mask read as luminance, and 0 for one read as alpha.
    pub(crate) mode: u32,
    /// On a clip's first link, how many links the clip has.
    pub(crate) count: u32,
    pub(crate) rect: [f32; 4],
    pub(crate) radii: [f32; 8],
    /// The exponents of a rounded link, or the mask's cell in its mask texture.
    pub(crate) extra: [f32; 4],
}

/// The kinds of [`GpuClipLink`].
pub(crate) mod link {
    pub(crate) const ROUNDED: u32 = 0;
    pub(crate) const ALPHA_MASK: u32 = 1;
    pub(crate) const COLOR_MASK: u32 = 2;
}

/// One paint.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuPaint {
    pub(crate) kind: u32,
    /// The gradient shape: 0 for linear, 1 for radial and 2 for conic.
    pub(crate) gradient: u32,
    /// The stop encoding.
    pub(crate) space: u32,
    /// 1 when the gradient's ramp repeats.
    pub(crate) flags: u32,
    /// The start and end, the centre and radii, or the centre and angle of the gradient.
    pub(crate) geometry: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) stop_start: u32,
    pub(crate) stop_count: u32,
    pub(crate) pad0: u32,
    pub(crate) pad1: u32,
}

/// One gradient stop, or one side colour of a sides paint.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuStop {
    pub(crate) color: [f32; 4],
    pub(crate) offset: f32,
    pub(crate) pad: [f32; 3],
}

/// One space: the world matrix, the inverse of its plane homography, and flags.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuSpatial {
    pub(crate) matrix: [[f32; 4]; 4],
    pub(crate) inverse: [[f32; 4]; 3],
    /// 1 in the first lane when the matrix only translates the plane.
    pub(crate) flags: [u32; 4],
}
