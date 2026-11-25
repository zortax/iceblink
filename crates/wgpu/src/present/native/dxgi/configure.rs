//! Making or resizing the swap chain for a configuration.

use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_MWA_NO_ALT_ENTER, DXGI_MWA_NO_WINDOW_CHANGES, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING, DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGISwapChain3,
};
use windows::core::Interface;

use super::Chain;
use super::format::dxgi_format;
use crate::present::FRAME_LATENCY;
use crate::present::native::ChainConfig;

impl Chain {
    /// Makes the swap chain, or resizes it, for `config`, and returns how many buffers it has.
    ///
    /// Fails with the reason when the window needs a composition swap chain, the format has no
    /// flip-model format, or a call fails.
    pub(crate) fn configure(&mut self, config: &ChainConfig) -> Result<usize, String> {
        if config.alpha_mode != wgpu::CompositeAlphaMode::Opaque {
            return Err("a translucent window needs a composition swap chain".to_owned());
        }
        if !config.view_formats.is_empty() {
            return Err("a flip-model swap chain is not viewed in another format here".to_owned());
        }
        let format = dxgi_format(config.format)
            .ok_or_else(|| format!("{:?} has no flip-model format", config.format))?;
        let [width, height] = [config.size[0].max(1), config.size[1].max(1)];
        self.mode = config.present_mode;

        // Nothing in flight may still use a buffer this gives back.
        self.idle();
        self.buffers.clear();

        let swapchain = match &self.swapchain {
            Some(swapchain) => {
                // SAFETY: the queue is idle and every reference to a buffer was dropped above.
                unsafe {
                    swapchain.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, self.flags)
                }
                .map_err(|error| format!("resizing the swap chain: {error}"))?;
                swapchain.clone()
            }
            None => self.create(width, height, format)?,
        };
        self.swapchain = Some(swapchain.clone());

        // SAFETY: the swap chain is live.
        let count = unsafe { swapchain.GetDesc1() }
            .map_err(|error| format!("reading the swap chain: {error}"))?
            .BufferCount;
        for index in 0..count {
            // SAFETY: `index` is below the swap chain's own buffer count.
            let resource: ID3D12Resource = unsafe { swapchain.GetBuffer(index) }
                .map_err(|error| format!("a swap chain buffer: {error}"))?;
            let buffer = self.wrap(resource, config.format, width, height)?;
            self.buffers.push(buffer);
        }
        self.extent = [width, height];
        Ok(self.buffers.len())
    }

    /// Makes the swap chain for buffers of `width` by `height` in `format`.
    ///
    /// Fails with the reason when a call fails.
    fn create(
        &mut self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<IDXGISwapChain3, String> {
        let mut flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;
        // Set at creation whatever the mode, because a resize cannot change it.
        if self.tearing {
            flags |= DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING;
        }
        let description = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: format,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: FRAME_LATENCY + 1,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: flags.0 as u32,
        };
        // SAFETY: the queue is the device's own, the window handle is live for as long as the
        // surface wgpu made for it, and the description is complete.
        let swapchain = unsafe {
            self.factory
                .CreateSwapChainForHwnd(&self.queue, self.hwnd, &description, None, None)
        }
        .map_err(|error| format!("creating the swap chain: {error}"))?;
        let swapchain: IDXGISwapChain3 = swapchain
            .cast()
            .map_err(|error| format!("the swap chain has no third interface: {error}"))?;
        // SAFETY: the window is the one the swap chain was made for. Alt+Enter is left to the
        // application, as wgpu leaves it.
        unsafe {
            self.factory.MakeWindowAssociation(
                self.hwnd,
                DXGI_MWA_NO_WINDOW_CHANGES | DXGI_MWA_NO_ALT_ENTER,
            )
        }
        .map_err(|error| format!("associating the window: {error}"))?;
        // SAFETY: the swap chain was made with the waitable object flag, and the latency is in the
        // range the call allows.
        unsafe { swapchain.SetMaximumFrameLatency(FRAME_LATENCY) }
            .map_err(|error| format!("setting the frame latency: {error}"))?;
        // SAFETY: as above.
        self.waitable = Some(unsafe { swapchain.GetFrameLatencyWaitableObject() });
        self.flags = flags;
        Ok(swapchain)
    }
}
