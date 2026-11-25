//! What a renderer's device supports.

/// What a renderer's device supports.
///
/// The capabilities decide what a frame holds, and every frame request includes the current ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderCapabilities {
    /// The largest texture extent the device creates, in texels.
    pub max_texture_size: u32,
    /// Whether subpixel sprites can be drawn, which needs dual-source blending.
    pub subpixel_text: bool,
    /// Whether application shaders can be drawn.
    pub custom_shaders: bool,
    /// Whether a compute path rasteriser is available.
    pub vector_compute: bool,
    /// Whether shifts can be applied to the kept target.
    pub can_shift: bool,
    /// Whether lowered chunks are kept on the device across frames.
    pub resident_chunks: bool,
    /// The blend modes drawn exactly, as bit `1 << mode` for each `BlendMode` value.
    pub blend_modes: u32,
    /// Whether the renderer keeps vector pass rasters by `PassKey` across frames.
    ///
    /// The compositor issues pass keys only to such a renderer, and plans its passes uncut to the
    /// draw damage.
    pub kept_rasters: bool,
}

impl RenderCapabilities {
    /// A device with no optional feature, a texture limit of 4096 texels and only
    /// `BlendMode::NORMAL` drawn exactly.
    pub const MINIMAL: Self = Self {
        max_texture_size: 4096,
        subpixel_text: false,
        custom_shaders: false,
        vector_compute: false,
        can_shift: false,
        resident_chunks: false,
        blend_modes: 1,
        kept_rasters: false,
    };
}
