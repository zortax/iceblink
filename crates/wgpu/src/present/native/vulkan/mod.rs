//! A Vulkan swap chain the renderer creates, acquires from and presents to.
//!
//! Each image is wrapped once as a `wgpu::Texture`, which wgpu tracks as a colour target. Two
//! barriers per frame keep that true: the acquire barrier moves the image from the present layout
//! to the colour attachment layout before wgpu records into it, and the release barrier moves it
//! back after the frame. An image keeps its texels from one present to the next, and loses them
//! when the swap chain is made again.
//!
//! The renderer submits every frame from one thread, and every submission here is made from it,
//! so the queue is never used from two threads at once.

mod acquire;
mod barrier;
mod configure;
mod format;
mod present;
mod timing;
mod wrap;

use std::sync::Arc;

use ash::vk;

use crate::gpu::device::Gpu;

/// How long an acquisition waits for an image, in nanoseconds.
// As long as wgpu's own acquisition waits.
const ACQUIRE_TIMEOUT: u64 = 1_000_000_000;

/// How long a fence of the acquire ring is waited on before the device is taken to be stuck, in
/// nanoseconds.
const FENCE_TIMEOUT: u64 = 2_000_000_000;

/// One image of the swap chain, with the barriers that move it between frames and presents.
struct Image {
    texture: wgpu::Texture,
    /// The barrier from `UNDEFINED`, for the image's first frame.
    first: vk::CommandBuffer,
    /// The barrier from `PRESENT_SRC_KHR`, for every later frame.
    again: vk::CommandBuffer,
    /// The barrier to `PRESENT_SRC_KHR`, after each frame.
    release: vk::CommandBuffer,
    /// The semaphore the release barrier signals and the present waits on.
    rendered: vk::Semaphore,
    /// Whether a frame was drawn into the image since the swap chain was made.
    used: bool,
}

/// One entry of the acquire ring.
// The ring is one longer than the image count. An entry is used again only once its fence
// signals that the barrier waiting on its semaphore finished, which also bounds how far the
// processor runs ahead of the device.
struct Acquire {
    /// The semaphore `vkAcquireNextImageKHR` signals and the acquire barrier waits on.
    semaphore: vk::Semaphore,
    /// The fence the acquire barrier signals.
    fence: vk::Fence,
    /// Whether a submission signals the fence and nothing waited on it since.
    pending: bool,
}

/// A Vulkan swap chain.
pub(crate) struct Chain {
    // Keeps the device alive for as long as anything here exists on it.
    gpu: Arc<Gpu>,
    device: ash::Device,
    swapchain_fns: ash::khr::swapchain::Device,
    surface_fns: ash::khr::surface::Instance,
    physical: vk::PhysicalDevice,
    queue: vk::Queue,
    // Owned by wgpu's `Surface`, which outlives the chain.
    surface: vk::SurfaceKHR,
    /// Whether the device has `VK_KHR_incremental_present`.
    incremental: bool,
    /// Whether the device has `VK_KHR_swapchain_mutable_format`.
    mutable_format: bool,
    pool: vk::CommandPool,
    /// The swap chain, null before the first configuration.
    swapchain: vk::SwapchainKHR,
    images: Vec<Image>,
    /// The acquire ring.
    acquires: Vec<Acquire>,
    /// The entry of the acquire ring the next acquisition uses.
    next: usize,
    extent: [u32; 2],
    /// Whether the images can be copied out of.
    readable: bool,
    /// The timing of presents, when the device has present ids and present waits.
    timing: Option<timing::PresentTiming>,
    /// The id of the last present to the current swap chain, counted from 1.
    present_id: u64,
    /// The timed presents of the swap chains before this one.
    presents_before: u64,
    /// The functions that read the display's refresh interval, where the device has them.
    display_timing: Option<ash::google::display_timing::Device>,
    /// The refresh interval of the display the swap chain presents to, where known.
    refresh_ns: Option<u64>,
}

impl std::fmt::Debug for Chain {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("vulkan::Chain")
            .field("images", &self.images.len())
            .field("extent", &self.extent)
            .field("incremental", &self.incremental)
            .finish_non_exhaustive()
    }
}

impl Chain {
    /// Returns a chain for `surface`, or `None` where the device or the surface cannot have one.
    pub(crate) fn new(gpu: &Arc<Gpu>, surface: &wgpu::Surface<'static>) -> Option<Self> {
        // SAFETY: `as_hal` asks that the surface is not destroyed while the guard lives. The guard
        // is read through and dropped here, and the surface outlives this chain.
        let raw_surface = unsafe {
            surface
                .as_hal::<wgpu::hal::api::Vulkan>()
                .and_then(|hal| hal.raw_native_handle())
        }?;
        // SAFETY: as above, for the device. The handles read out are owned by the device, which
        // the `Arc<Gpu>` held here keeps alive.
        let hal = unsafe { gpu.device().as_hal::<wgpu::hal::api::Vulkan>() }?;
        let device = hal.raw_device().clone();
        let instance = hal.shared_instance();
        let swapchain_fns = ash::khr::swapchain::Device::new(instance.raw_instance(), &device);
        let surface_fns =
            ash::khr::surface::Instance::new(instance.entry(), instance.raw_instance());
        let enabled = hal.enabled_device_extensions();
        let incremental = enabled.contains(&ash::khr::incremental_present::NAME);
        let mutable_format = enabled.contains(&ash::khr::swapchain_mutable_format::NAME);
        let display_timing = enabled
            .contains(&ash::google::display_timing::NAME)
            .then(|| ash::google::display_timing::Device::new(instance.raw_instance(), &device));
        let timing = crate::gpu::extensions::PRESENT_TIMING
            .iter()
            .all(|name| enabled.contains(name))
            .then(|| ash::khr::present_wait::Device::new(instance.raw_instance(), &device))
            .and_then(timing::PresentTiming::new);
        let physical = hal.raw_physical_device();
        let family = hal.queue_family_index();
        let queue = hal.raw_queue();

        // SAFETY: the physical device and the surface are live, and the family is one of the
        // physical device's own.
        let supported = unsafe {
            surface_fns.get_physical_device_surface_support(physical, family, raw_surface)
        };
        if !matches!(supported, Ok(true)) {
            return None;
        }

        let info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: the family is the one the device's queue was created from.
        let pool = unsafe { device.create_command_pool(&info, None) }.ok()?;
        tracing::info!(
            incremental,
            timed = timing.is_some(),
            "a window presents through a swap chain the renderer made"
        );
        Some(Self {
            gpu: Arc::clone(gpu),
            device,
            swapchain_fns,
            surface_fns,
            physical,
            queue,
            surface: raw_surface,
            incremental,
            mutable_format,
            pool,
            swapchain: vk::SwapchainKHR::null(),
            images: Vec::new(),
            acquires: Vec::new(),
            next: 0,
            extent: [0, 0],
            readable: false,
            timing,
            present_id: 0,
            presents_before: 0,
            display_timing,
            refresh_ns: None,
        })
    }

    /// Returns the extent of the images, in texels.
    pub(crate) fn extent(&self) -> [u32; 2] {
        self.extent
    }

    /// Reports whether the images can be copied out of.
    pub(crate) fn readable(&self) -> bool {
        self.readable
    }

    /// Moves the numbers of presents and the times they were shown on the display into `out`, on
    /// the frame clock, oldest first.
    pub(crate) fn presented(&mut self, out: &mut Vec<(u64, u64)>) {
        if let Some(timing) = &self.timing {
            timing.drain(out);
        }
    }

    /// Returns the number of the newest timed present, which counts the timed presents of every
    /// swap chain of the chain, or zero when presents are not timed.
    pub(crate) fn presents(&self) -> u64 {
        self.presents_before + self.present_id
    }

    /// Returns the refresh interval of the display the swap chain presents to, where known.
    pub(crate) fn refresh_ns(&self) -> Option<u64> {
        self.refresh_ns
    }

    /// Reports whether presents are timed.
    pub(crate) fn timed(&self) -> bool {
        self.timing.is_some()
    }

    /// Takes the refreshes frames are planned on: one at `refresh_ns` and the others every
    /// `interval_ns` from it, on the frame clock.
    pub(crate) fn follow_refreshes(&self, refresh_ns: u64, interval_ns: u64) {
        if let Some(timing) = &self.timing {
            timing.follow(refresh_ns, interval_ns);
        }
    }

    /// Returns how many presents the display holds at once, one while presents are not timed.
    pub(crate) fn display_depth(&self) -> u32 {
        self.timing.as_ref().map_or(1, timing::PresentTiming::depth)
    }

    /// Reports whether as many timed presents wait for the display as it holds at once.
    pub(crate) fn display_full(&self) -> bool {
        self.timing
            .as_ref()
            .is_some_and(timing::PresentTiming::full)
    }

    /// Reports whether a present passes the rectangles that changed to the display server.
    pub(crate) fn incremental(&self) -> bool {
        self.incremental
    }

    /// Waits until the device finished everything, so nothing in flight uses an image.
    fn idle(&mut self) {
        if let Err(error) = self.gpu.device().poll(wgpu::PollType::wait_indefinitely()) {
            tracing::warn!(%error, "waiting for the device before a swap chain was replaced");
        }
        // SAFETY: the device is live. Waiting for it to go idle has no other precondition.
        if let Err(error) = unsafe { self.device.device_wait_idle() } {
            tracing::warn!(%error, "waiting for the device before a swap chain was replaced");
        }
        for ring in 0..self.acquires.len() {
            self.settle(ring);
        }
    }

    /// Frees every image's texture, barriers and semaphore.
    ///
    /// The device must be idle.
    fn release_images(&mut self) {
        for image in self.images.drain(..) {
            let commands = [image.first, image.again, image.release];
            // SAFETY: the device is idle, so none of these is pending, and they belong to this
            // chain's pool and device.
            unsafe {
                self.device.free_command_buffers(self.pool, &commands);
                self.device.destroy_semaphore(image.rendered, None);
            }
            drop(image.texture);
        }
    }
}

impl Drop for Chain {
    fn drop(&mut self) {
        // The waiter stops before the swap chain it waits on is destroyed.
        self.timing = None;
        self.idle();
        self.release_images();
        for acquire in self.acquires.drain(..) {
            // SAFETY: the device is idle, so neither is in use.
            unsafe {
                self.device.destroy_semaphore(acquire.semaphore, None);
                self.device.destroy_fence(acquire.fence, None);
            }
        }
        // SAFETY: the device is idle and every image was released, so nothing uses the swap
        // chain or the pool. The surface is wgpu's and is left alone.
        unsafe {
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_fns.destroy_swapchain(self.swapchain, None);
            }
            self.device.destroy_command_pool(self.pool, None);
        }
    }
}
