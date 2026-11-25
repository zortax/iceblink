//! A DXGI swap chain the renderer creates, acquires from and presents to.
//!
//! The swap chain uses the sequential flip model, whose back buffers keep their texels, and
//! presents with `Present1`, which passes the rectangles that changed. Each buffer is wrapped once
//! as a `wgpu::Texture`, which wgpu tracks as a colour target. Two command lists per buffer,
//! recorded once, move it to the present state after each frame and back to the render target
//! state before the next frame drawn into it.
//!
//! A chain is made for a window with an `HWND`, an opaque alpha mode and an unencoded format.
//! The swap chain presents on the queue wgpu submits frames on, so a present is ordered after
//! every frame submitted before it.

mod acquire;
mod buffer;
mod configure;
mod format;
mod present;

use std::sync::Arc;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, WAIT_OBJECT_0};
use windows::Win32::Graphics::Direct3D12::{
    D3D12_FENCE_FLAG_NONE, ID3D12CommandQueue, ID3D12Device, ID3D12Fence,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_SWAP_CHAIN_FLAG, IDXGIFactory2,
    IDXGISwapChain3,
};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObjectEx};

use self::buffer::Buffer;
use crate::gpu::device::Gpu;
use crate::present::native::NativeWindow;

/// How long an acquisition waits for a buffer, in milliseconds.
// As long as wgpu's own acquisition waits.
const ACQUIRE_TIMEOUT_MS: u32 = 1_000;

/// A DXGI swap chain.
pub(crate) struct Chain {
    // Keeps the device alive for as long as anything here exists on it.
    gpu: Arc<Gpu>,
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    factory: IDXGIFactory2,
    hwnd: HWND,
    /// Whether presents may tear on this system.
    tearing: bool,
    /// The swap chain, `None` before the first configuration.
    swapchain: Option<IDXGISwapChain3>,
    /// The event signalled when the swap chain takes another frame.
    waitable: Option<HANDLE>,
    /// The flags the swap chain was made with.
    // Every resize repeats them.
    flags: DXGI_SWAP_CHAIN_FLAG,
    buffers: Vec<Buffer>,
    /// The fence that `idle` signals on the queue.
    fence: ID3D12Fence,
    /// The last value `idle` signalled `fence` with.
    fenced: u64,
    /// The event `idle` waits on for `fence`.
    event: HANDLE,
    mode: wgpu::PresentMode,
    extent: [u32; 2],
}

// SAFETY: the window handle and the two event handles are kernel handles, which any thread of the
// process may use. The DXGI and D3D12 objects are free-threaded. The renderer configures a surface
// on a worker thread, and nothing here is used from two threads at once.
unsafe impl Send for Chain {}

impl std::fmt::Debug for Chain {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("dxgi::Chain")
            .field("buffers", &self.buffers.len())
            .field("extent", &self.extent)
            .finish_non_exhaustive()
    }
}

impl Chain {
    /// Returns a chain for `window`, or `None` where the device or the window cannot have one.
    pub(crate) fn new(gpu: &Arc<Gpu>, window: NativeWindow) -> Option<Self> {
        let hwnd = HWND(window.hwnd? as *mut core::ffi::c_void);
        // SAFETY: `as_hal` asks that the device is not destroyed while the guard lives. The guard
        // is read through and dropped here, and the COM references cloned out of it keep what
        // they name alive.
        let (device, queue) = unsafe {
            let hal = gpu.device().as_hal::<wgpu::hal::api::Dx12>()?;
            (hal.raw_device().clone(), hal.raw_queue().clone())
        };
        // SAFETY: no flags, and the interface asked for is one a factory implements.
        let factory: IDXGIFactory2 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }.ok()?;
        let tearing = format::allows_tearing(&factory);
        // SAFETY: the device is live.
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }.ok()?;
        // SAFETY: an unnamed auto-reset event with default security.
        let event = unsafe { CreateEventW(None, false, false, None) }.ok()?;
        tracing::info!(
            tearing,
            "a window presents through a swap chain the renderer made"
        );
        Some(Self {
            gpu: Arc::clone(gpu),
            device,
            queue,
            factory,
            hwnd,
            tearing,
            swapchain: None,
            waitable: None,
            flags: DXGI_SWAP_CHAIN_FLAG(0),
            buffers: Vec::new(),
            fence,
            fenced: 0,
            event,
            mode: wgpu::PresentMode::Fifo,
            extent: [0, 0],
        })
    }

    /// Returns the extent of the buffers, in texels.
    pub(crate) fn extent(&self) -> [u32; 2] {
        self.extent
    }

    /// Waits until the queue finished everything submitted to it.
    fn idle(&mut self) {
        if let Err(error) = self.gpu.device().poll(wgpu::PollType::wait_indefinitely()) {
            tracing::warn!(%error, "waiting for the device before a swap chain was changed");
        }
        self.fenced += 1;
        // SAFETY: the fence and the event belong to this chain, and the queue is live.
        let waited = unsafe {
            self.queue
                .Signal(&self.fence, self.fenced)
                .and_then(|()| self.fence.SetEventOnCompletion(self.fenced, self.event))
                .map(|()| WaitForSingleObjectEx(self.event, INFINITE, false))
        };
        if !matches!(waited, Ok(event) if event == WAIT_OBJECT_0) {
            tracing::warn!("waiting for the queue before a swap chain was changed failed");
        }
    }
}

impl Drop for Chain {
    fn drop(&mut self) {
        self.idle();
        self.buffers.clear();
        self.swapchain = None;
        // SAFETY: both handles were opened by this chain and nothing waits on them now.
        unsafe {
            if let Some(waitable) = self.waitable.take() {
                drop(CloseHandle(waitable));
            }
            drop(CloseHandle(self.event));
        }
    }
}
