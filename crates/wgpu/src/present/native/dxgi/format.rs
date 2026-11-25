//! The formats and present flags a flip-model swap chain is made with.

use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
    DXGI_FORMAT_R10G10B10A2_UNORM,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_FEATURE_PRESENT_ALLOW_TEARING, IDXGIFactory2, IDXGIFactory5,
};
use windows::core::Interface;

/// Reports whether presents may tear on this system.
pub(super) fn allows_tearing(factory: &IDXGIFactory2) -> bool {
    let Ok(factory) = factory.cast::<IDXGIFactory5>() else {
        return false;
    };
    let mut allowed = windows::core::BOOL(0);
    // SAFETY: the data is a `BOOL`, as the feature asks, and its size is stated.
    let asked = unsafe {
        factory.CheckFeatureSupport(
            DXGI_FEATURE_PRESENT_ALLOW_TEARING,
            (&raw mut allowed).cast(),
            size_of::<windows::core::BOOL>() as u32,
        )
    };
    asked.is_ok() && allowed.as_bool()
}

/// Returns the flip-model format of `format`, or `None` for a format no flip-model swap chain is
/// made in.
pub(super) fn dxgi_format(format: wgpu::TextureFormat) -> Option<DXGI_FORMAT> {
    Some(match format {
        wgpu::TextureFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        wgpu::TextureFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        wgpu::TextureFormat::Rgb10a2Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
        _ => return None,
    })
}
