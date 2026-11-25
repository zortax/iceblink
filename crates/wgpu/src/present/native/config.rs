//! The configuration a native chain is built for.

/// The configuration of a native chain, which mirrors a wgpu surface configuration.
#[derive(Clone, Debug)]
#[cfg_attr(
    not(vulkan_hal),
    expect(dead_code, reason = "no backend of this target has a native chain")
)]
pub(crate) struct ChainConfig {
    /// The extent of the images, in texels.
    pub(crate) size: [u32; 2],
    /// The format of the images.
    pub(crate) format: wgpu::TextureFormat,
    /// The other formats an image is viewed in.
    pub(crate) view_formats: Vec<wgpu::TextureFormat>,
    /// The presentation mode.
    pub(crate) present_mode: wgpu::PresentMode,
    /// How the display server composites the images' alpha.
    pub(crate) alpha_mode: wgpu::CompositeAlphaMode,
}

impl ChainConfig {
    /// Returns the configuration of a chain that stands in for `config`.
    pub(crate) fn of(config: &wgpu::SurfaceConfiguration) -> Self {
        Self {
            size: [config.width, config.height],
            format: config.format,
            view_formats: config.view_formats.clone(),
            present_mode: config.present_mode,
            alpha_mode: config.alpha_mode,
        }
    }
}
