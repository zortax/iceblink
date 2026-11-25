//! The Vulkan formats of the swap chain formats the renderer draws into.

use ash::vk;

/// Returns the Vulkan format of `format`, or `None` for a format no swap chain here is made in.
pub(super) fn vk_format(format: wgpu::TextureFormat) -> Option<vk::Format> {
    Some(match format {
        wgpu::TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
        wgpu::TextureFormat::Bgra8UnormSrgb => vk::Format::B8G8R8A8_SRGB,
        wgpu::TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        wgpu::TextureFormat::Rgba8UnormSrgb => vk::Format::R8G8B8A8_SRGB,
        wgpu::TextureFormat::Rgb10a2Unorm => vk::Format::A2B10G10R10_UNORM_PACK32,
        wgpu::TextureFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::vk_format;

    #[test]
    fn the_eight_bit_formats_have_vulkan_formats() {
        for format in [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            assert!(vk_format(format).is_some(), "{format:?}");
        }
    }
}
