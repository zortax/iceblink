//! The native chain of whichever platform API the device runs on.

use std::sync::Arc;

use iceblink::geom::{Device, Rect};

use crate::gpu::device::Gpu;
use crate::present::acquisition::Acquisition;
use crate::present::native::{ChainConfig, NativeWindow};

/// A native chain on one of the backends that can have one.
#[derive(Debug)]
pub(crate) enum Backend {
    /// A Vulkan swap chain.
    #[cfg(vulkan_hal)]
    Vulkan(Box<crate::present::native::vulkan::Chain>),
    /// A DXGI swap chain.
    #[cfg(windows)]
    Dxgi(Box<crate::present::native::dxgi::Chain>),
    /// Nothing, on a target where no backend has a native chain.
    #[cfg(not(vulkan_hal))]
    #[expect(dead_code, reason = "a value of this variant cannot exist")]
    Never(core::convert::Infallible),
}

#[cfg_attr(
    not(vulkan_hal),
    expect(
        unused_variables,
        reason = "no backend of this target has a native chain"
    )
)]
impl Backend {
    /// Returns a chain for `surface` on `gpu`, or `None` where the backend or the surface cannot
    /// have one.
    ///
    /// No swap chain exists until the first [`Backend::configure`].
    pub(crate) fn new(
        gpu: &Arc<Gpu>,
        surface: &wgpu::Surface<'static>,
        window: NativeWindow,
    ) -> Option<Self> {
        match gpu.adapter().get_info().backend {
            #[cfg(vulkan_hal)]
            wgpu::Backend::Vulkan => {
                let _ = window;
                crate::present::native::vulkan::Chain::new(gpu, surface)
                    .map(|chain| Self::Vulkan(Box::new(chain)))
            }
            #[cfg(windows)]
            wgpu::Backend::Dx12 => crate::present::native::dxgi::Chain::new(gpu, window)
                .map(|chain| Self::Dxgi(Box::new(chain))),
            _ => None,
        }
    }

    /// Makes the swap chain again for `config`, and returns how many images it has.
    ///
    /// Every image's contents are lost.
    ///
    /// Fails with the reason the chain cannot be made, and the window then presents through
    /// wgpu's swap chain.
    pub(crate) fn configure(&mut self, config: &ChainConfig) -> Result<usize, String> {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.configure(config),
            #[cfg(windows)]
            Self::Dxgi(chain) => chain.configure(config),
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Acquires the next image, and returns the answer with the image's slot and texture when the
    /// answer presents.
    pub(crate) fn acquire(&mut self) -> (Acquisition, Option<(u32, wgpu::Texture)>) {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.acquire(),
            #[cfg(windows)]
            Self::Dxgi(chain) => chain.acquire(),
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Presents the image at `slot`.
    ///
    /// `rects` are the rectangles that changed since the previous present, and `None` stands for
    /// the whole image.
    pub(crate) fn present(
        &mut self,
        slot: u32,
        rects: Option<&[Rect<i32, Device>]>,
    ) -> Acquisition {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.present(slot, rects),
            #[cfg(windows)]
            Self::Dxgi(chain) => chain.present(slot, rects),
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Returns the extent of the images, in texels.
    pub(crate) fn extent(&self) -> [u32; 2] {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.extent(),
            #[cfg(windows)]
            Self::Dxgi(chain) => chain.extent(),
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Reports whether the images can be copied out of.
    pub(crate) fn readable(&self) -> bool {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.readable(),
            #[cfg(windows)]
            Self::Dxgi(_) => true,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Reports whether the chain reports when its presents were shown on the display.
    pub(crate) fn timed(&self) -> bool {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.timed(),
            #[cfg(windows)]
            Self::Dxgi(_) => false,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Returns the refresh interval of the display the chain presents to, where known.
    pub(crate) fn refresh_ns(&self) -> Option<u64> {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.refresh_ns(),
            #[cfg(windows)]
            Self::Dxgi(_) => None,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Returns the number of the newest present.
    pub(crate) fn presents(&self) -> u64 {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.presents(),
            #[cfg(windows)]
            Self::Dxgi(_) => 0,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Takes the refreshes frames are planned on: one at `refresh_ns` and the others every
    /// `interval_ns` from it, on the frame clock.
    pub(crate) fn follow_refreshes(&self, refresh_ns: u64, interval_ns: u64) {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.follow_refreshes(refresh_ns, interval_ns),
            #[cfg(windows)]
            Self::Dxgi(_) => {}
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Returns how many presents the display holds at once.
    pub(crate) fn display_depth(&self) -> u32 {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.display_depth(),
            #[cfg(windows)]
            Self::Dxgi(_) => 1,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Reports whether as many timed presents wait for the display as it holds at once.
    pub(crate) fn display_full(&self) -> bool {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.display_full(),
            #[cfg(windows)]
            Self::Dxgi(_) => false,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Moves the numbers of presents and the times they were shown on the display into `out`, on
    /// the frame clock, oldest first.
    pub(crate) fn presented(&mut self, out: &mut Vec<(u64, u64)>) {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.presented(out),
            #[cfg(windows)]
            Self::Dxgi(_) => {}
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }

    /// Reports whether a present passes the changed rectangles to the display server.
    pub(crate) fn partial(&self) -> bool {
        match self {
            #[cfg(vulkan_hal)]
            Self::Vulkan(chain) => chain.incremental(),
            #[cfg(windows)]
            Self::Dxgi(_) => true,
            #[cfg(not(vulkan_hal))]
            Self::Never(never) => match *never {},
        }
    }
}
