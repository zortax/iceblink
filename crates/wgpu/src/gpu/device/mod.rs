//! Opening a device, or adopting one the application opened.

mod capabilities;
mod request;

use std::ffi::CStr;
use std::sync::Arc;

use crate::gpu::adapter;
use crate::gpu::extensions;
use crate::gpu::loss::DeviceLoss;
use crate::present::native::SwapChain;

pub use self::capabilities::DeviceCapabilities;

use self::capabilities::probe;
use self::request::{descriptor, with_presenter};

/// An open graphics device: the instance, adapter, device and queue, and what the device
/// supports.
#[derive(Debug)]
pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    capabilities: DeviceCapabilities,
    extensions: Vec<&'static CStr>,
    loss: Arc<DeviceLoss>,
    adopted: bool,
}

impl Gpu {
    /// Opens a device on `adapter`, with the Vulkan device extensions `extensions` where the
    /// adapter has them.
    ///
    /// The device can make the swap chains of [`SwapChain::Auto`], or those
    /// [`PRESENTER_VARIABLE`](crate::present::native::PRESENTER_VARIABLE) chooses.
    ///
    /// Fails with the device request's message when the adapter refuses the device.
    pub fn open(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        extensions: &[&'static CStr],
    ) -> Result<Self, String> {
        Self::open_for(instance, adapter, extensions, SwapChain::Auto.overridden())
    }

    /// Opens a device as [`Gpu::open`] does, able to make the swap chains of `swap_chain`.
    pub(crate) fn open_for(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        extensions: &[&'static CStr],
        swap_chain: SwapChain,
    ) -> Result<Self, String> {
        let descriptor = descriptor(&adapter);
        let extensions = with_presenter(&adapter, extensions, swap_chain);
        let (device, queue, extensions) = match extensions::open(&adapter, &extensions, &descriptor)
        {
            Some(opened) => opened,
            None => {
                let (device, queue) =
                    futures::executor::block_on(adapter.request_device(&descriptor))
                        .map_err(|error| error.to_string())?;
                (device, queue, Vec::new())
            }
        };
        Ok(Self::opened(instance, adapter, device, queue, extensions))
    }

    /// Opens a device on `adapter` without blocking.
    ///
    /// Dual-source blending is left off unless `subpixel_text` asks for it.
    ///
    /// Fails with the device request's message when the adapter refuses the device.
    pub async fn open_async(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        subpixel_text: bool,
    ) -> Result<Self, String> {
        let mut descriptor = descriptor(&adapter);
        if !subpixel_text {
            descriptor
                .required_features
                .remove(wgpu::Features::DUAL_SOURCE_BLENDING);
        }
        let (device, queue) = adapter
            .request_device(&descriptor)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self::opened(instance, adapter, device, queue, Vec::new()))
    }

    /// Returns the device opened on `adapter`, with its loss reported into a new [`DeviceLoss`].
    fn opened(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        extensions: Vec<&'static CStr>,
    ) -> Self {
        let loss = Arc::new(DeviceLoss::new());
        let watcher = Arc::clone(&loss);
        device.set_device_lost_callback(move |reason, message| watcher.report(reason, &message));

        let capabilities = probe(&adapter, &device);
        Self {
            instance,
            adapter,
            device,
            queue,
            capabilities,
            extensions,
            loss,
            adopted: false,
        }
    }

    /// Wraps a device the application already opened, so the renderer draws on it.
    ///
    /// The device's loss is not watched. [`DeviceLoss::observer`] returns a callback that reports
    /// it.
    pub fn from_existing(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Arc<Self> {
        let capabilities = probe(&adapter, &device);
        Arc::new(Self {
            instance,
            adapter,
            device,
            queue,
            capabilities,
            extensions: Vec::new(),
            loss: Arc::new(DeviceLoss::new()),
            adopted: true,
        })
    }

    /// Reports whether the device came from [`Gpu::from_existing`].
    pub fn is_adopted(&self) -> bool {
        self.adopted
    }

    /// Returns the instance the adapter came from.
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    /// Returns the adapter the device was opened on.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Returns the device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Returns the device's queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Returns what the device supports.
    pub fn capabilities(&self) -> DeviceCapabilities {
        self.capabilities
    }

    /// Returns the Vulkan device extensions the device was opened with.
    pub fn vulkan_extensions(&self) -> &[&'static CStr] {
        &self.extensions
    }

    /// Reports whether the device was opened with the extensions that time native presents.
    pub(crate) fn times_presents(&self) -> bool {
        #[cfg(vulkan_hal)]
        {
            extensions::PRESENT_TIMING
                .iter()
                .all(|name| self.extensions.contains(name))
        }
        #[cfg(not(vulkan_hal))]
        {
            false
        }
    }

    /// Returns the device's loss state.
    pub fn loss(&self) -> &Arc<DeviceLoss> {
        &self.loss
    }

    /// Returns the bytes an upload wrote, or zero after reporting a refused staging mapping as a
    /// device loss.
    pub(crate) fn uploaded(&self, written: Result<u64, crate::upload::Refused>) -> u64 {
        written.unwrap_or_else(|crate::upload::Refused(error)| {
            // One report for each loss: every later upload of the frame is refused the same way.
            if !self.loss.is_lost() {
                self.loss
                    .report(wgpu::DeviceLostReason::Unknown, &error.to_string());
            }
            0
        })
    }

    /// Returns a description of the adapter for logs.
    pub fn describe(&self) -> String {
        adapter::describe(&self.adapter.get_info())
    }

    /// Runs the callbacks of work the device finished, without waiting.
    pub fn reclaim(&self) {
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Waits until the device finished all submitted work.
    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}
