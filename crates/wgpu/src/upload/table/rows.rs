//! The layout of table textures.

/// How many texels wide every table texture is.
///
/// Shaders that read a table divide by the same number.
pub const TEXELS_WIDE: u32 = 256;

/// How many bytes one `rgba32uint` texel holds.
pub const TEXEL: usize = 16;

/// How many bytes one row of a table holds.
pub const ROW: usize = TEXELS_WIDE as usize * TEXEL;

/// Returns the most rows a table can have on `device`.
pub fn row_limit(device: &wgpu::Device) -> u32 {
    device.limits().max_texture_dimension_2d
}

/// Allocates a table texture of `rows` rows, and returns it with its view.
pub fn allocate_rows(
    device: &wgpu::Device,
    label: &'static str,
    rows: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: TEXELS_WIDE,
            height: rows,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
