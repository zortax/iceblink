//! What an open device supports.

/// What a device supports, probed once when it is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceCapabilities {
    /// The largest texture side, in texels.
    pub max_texture_size: u32,
    /// Whether dual-source blending is available for subpixel sprites.
    pub subpixel_text: bool,
    /// Whether application shaders can be drawn.
    pub custom_shaders: bool,
    /// Whether a compute path rasteriser can run.
    pub vector_compute: bool,
    /// Whether a surface texture can be viewed in a format other than its own.
    pub mutable_texture_formats: bool,
}

/// Returns what `device` on `adapter` supports, and logs whether it draws subpixel text.
pub(super) fn probe(adapter: &wgpu::Adapter, device: &wgpu::Device) -> DeviceCapabilities {
    let downlevel = adapter.get_downlevel_capabilities();
    let features = device.features();
    let limits = device.limits();
    let subpixel_text = features.contains(wgpu::Features::DUAL_SOURCE_BLENDING);
    tracing::info!(
        "the device {} text with one coverage value per display subpixel",
        if subpixel_text {
            "can draw"
        } else {
            "cannot draw"
        }
    );
    DeviceCapabilities {
        subpixel_text,
        vector_compute: downlevel
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
            && downlevel
                .flags
                .contains(wgpu::DownlevelFlags::SHADER_F16_IN_F32)
            && adapter
                .get_texture_format_features(wgpu::TextureFormat::Rgba8Unorm)
                .allowed_usages
                .contains(wgpu::TextureUsages::STORAGE_BINDING),
        mutable_texture_formats: downlevel
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS),
        // An application shader draws through a third bind group.
        custom_shaders: limits.max_bind_groups >= 3,
        max_texture_size: limits.max_texture_dimension_2d,
    }
}
