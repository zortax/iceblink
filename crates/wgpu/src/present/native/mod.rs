//! Swap chains the renderer creates and presents through itself.
//!
//! The images of a native chain keep their texels between presents, so the copy into an image
//! covers only its debt. A present lists the rectangles that changed, so the display server
//! recomposites only those.
//!
//! A native chain is made on a Vulkan device and on a DX12 device with an opaque window, when
//! [`SwapChain::Native`] is chosen, and on a Vulkan device that times its presents when
//! [`SwapChain::Auto`] is chosen. Where a chain cannot be made, the window presents through wgpu's
//! swap chain.

mod backend;
mod config;
// The swap chain APIs are reached below wgpu, which needs `unsafe`, with each block's invariants
// stated.
#[cfg(windows)]
#[allow(unsafe_code)]
mod dxgi;
mod swapchain;
mod verify;
#[cfg(vulkan_hal)]
#[allow(unsafe_code)]
mod vulkan;
mod window;

pub(crate) use self::config::ChainConfig;
pub(crate) use self::swapchain::NativeSwapchain;
pub(crate) use self::window::NativeWindow;

use crate::gpu::device::Gpu;

/// The environment variable that chooses who owns a window's swap chain, over the builder's
/// choice.
///
/// `native` chooses [`SwapChain::Native`], `auto` [`SwapChain::Auto`], and any other value
/// [`SwapChain::Wgpu`]. Without the variable, the builder's choice applies.
pub const PRESENTER_VARIABLE: &str = "ICEBLINK_PRESENTER";

/// Who owns a window's swap chain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwapChain {
    /// wgpu's swap chain, which presents whole images.
    Wgpu,
    /// A native chain where the backend allows one, which presents only the damaged rectangles,
    /// and wgpu's swap chain elsewhere.
    Native,
    /// A native Vulkan chain where the device reports when presents are shown on the display, and
    /// wgpu's swap chain elsewhere.
    ///
    /// The device reports it when it has `VK_KHR_present_id` and `VK_KHR_present_wait` with
    /// their features, and [`TIMING_VARIABLE`] does not turn the timing off.
    #[default]
    Auto,
}

impl SwapChain {
    /// Returns the swap chain [`PRESENTER_VARIABLE`] chooses, or `self` without the variable.
    pub(crate) fn overridden(self) -> Self {
        match std::env::var_os(PRESENTER_VARIABLE) {
            Some(value) if value == "native" => Self::Native,
            Some(value) if value == "auto" => Self::Auto,
            Some(_) => Self::Wgpu,
            None => self,
        }
    }

    /// Reports whether a device opened on `adapter` asks for the extensions of native chains.
    pub(crate) fn opens_native(self, adapter: &wgpu::Adapter) -> bool {
        match self {
            Self::Wgpu => false,
            Self::Native => true,
            Self::Auto => {
                times_presents() && crate::gpu::extensions::present_timing_granted(adapter)
            }
        }
    }

    /// Reports whether a window on `gpu` presents through a native chain where one can be made.
    pub(crate) fn makes_native(self, gpu: &Gpu) -> bool {
        match self {
            Self::Wgpu => false,
            Self::Native => true,
            Self::Auto => gpu.times_presents(),
        }
    }
}

/// The environment variable that checks every presented native image against the kept target.
///
/// `1` reads each image back after the copy and logs the first texel that differs. It waits for
/// the device on every frame.
pub const VERIFY_VARIABLE: &str = "ICEBLINK_PRESENT_VERIFY";

/// The environment variable that turns off the timing of native presents.
///
/// `0` opens devices without present ids and present waits, so native chains report no feedback
/// and [`SwapChain::Auto`] chooses wgpu's swap chain. Without the variable, a device that has
/// them times its native presents.
pub const TIMING_VARIABLE: &str = "ICEBLINK_PRESENT_TIMING";

/// Reports whether native chains time their presents.
pub(crate) fn times_presents() -> bool {
    std::env::var(TIMING_VARIABLE).map_or(true, |value| value != "0")
}

/// Reports whether the environment asks for every presented native image to be checked.
pub(crate) fn verifies() -> bool {
    std::env::var(VERIFY_VARIABLE).is_ok_and(|value| value == "1")
}
