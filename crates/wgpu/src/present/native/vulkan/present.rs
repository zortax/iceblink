//! Releasing an image and presenting it with the rectangles that changed.

use ash::vk;
use iceblink::geom::{Device, Rect};

use super::Chain;
use crate::present::acquisition::Acquisition;

impl Chain {
    /// Releases the image at `slot` and presents it.
    ///
    /// `rects` are the rectangles that changed since the previous present, inside the image, and
    /// `None` means the whole image.
    pub(crate) fn present(
        &mut self,
        slot: u32,
        rects: Option<&[Rect<i32, Device>]>,
    ) -> Acquisition {
        let Some(image) = self.images.get(slot as usize) else {
            return Acquisition::Validation;
        };
        let (release, rendered) = (image.release, image.rendered);

        if let Err(error) = self.submit_release(release, rendered) {
            tracing::warn!(%error, "the barrier that releases a swap chain image was refused");
            return self.classify(error);
        }

        match self.queue_present(slot, rendered, rects.unwrap_or_default()) {
            Ok(false) => Acquisition::Success,
            Ok(true) => Acquisition::Suboptimal,
            Err(error) => self.classify(error),
        }
    }

    /// Submits the release barrier `release`, which signals `rendered`.
    ///
    /// Fails with the driver's error.
    fn submit_release(
        &self,
        release: vk::CommandBuffer,
        rendered: vk::Semaphore,
    ) -> Result<(), vk::Result> {
        let signals = [rendered];
        let commands = [release];
        let submit = vk::SubmitInfo::default()
            .command_buffers(&commands)
            .signal_semaphores(&signals);
        // SAFETY: the command buffer was recorded for simultaneous use and names an image of this
        // swap chain. The semaphore is unsignalled, since the present that last waited on it took
        // it.
        unsafe {
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit), vk::Fence::null())
        }
    }

    /// Presents the released image at `slot` once `rendered` signals, with `rects` as the
    /// rectangles that changed, and reports whether the swap chain is suboptimal.
    ///
    /// An empty `rects` covers the whole image. A timed present is numbered with the next present
    /// id.
    ///
    /// Fails with the driver's error.
    fn queue_present(
        &mut self,
        slot: u32,
        rendered: vk::Semaphore,
        rects: &[Rect<i32, Device>],
    ) -> Result<bool, vk::Result> {
        let layers = rect_layers(rects);
        let region = [vk::PresentRegionKHR::default().rectangles(&layers)];
        let mut regions = vk::PresentRegionsKHR::default().regions(&region);
        let waits = [rendered];
        let swapchains = [self.swapchain];
        let indices = [slot];
        let mut info = vk::PresentInfoKHR::default()
            .wait_semaphores(&waits)
            .swapchains(&swapchains)
            .image_indices(&indices);
        if self.incremental && !layers.is_empty() {
            info = info.push_next(&mut regions);
        }
        let ids = [self.present_id + 1];
        let mut identified = vk::PresentIdKHR::default().present_ids(&ids);
        if self.timing.is_some() {
            info = info.push_next(&mut identified);
        }

        let held = self.timing.as_ref().map(|timing| timing.hold());
        // SAFETY: the image was acquired from this swap chain and released, the semaphore is
        // signalled by that release, and the rectangles lie inside the image. The queue is used
        // only from this thread, and the swap chain is held against the present waiter.
        let presented = unsafe { self.swapchain_fns.queue_present(self.queue, &info) };
        drop(held);

        if let Some(timing) = &self.timing
            && presented.is_ok()
        {
            self.present_id += 1;
            timing.submitted(self.present_id);
        }
        presented
    }

    /// Returns the answer for a Vulkan error, and reports a lost device to the device's loss.
    pub(super) fn classify(&mut self, error: vk::Result) -> Acquisition {
        match error {
            vk::Result::TIMEOUT | vk::Result::NOT_READY => Acquisition::Timeout,
            vk::Result::ERROR_OUT_OF_DATE_KHR => Acquisition::Outdated,
            vk::Result::ERROR_SURFACE_LOST_KHR => Acquisition::Lost,
            vk::Result::ERROR_DEVICE_LOST => {
                self.gpu.loss().report(
                    wgpu::DeviceLostReason::Unknown,
                    "the device was lost while presenting",
                );
                Acquisition::Validation
            }
            other => {
                tracing::warn!(error = %other, "a swap chain call failed");
                Acquisition::Validation
            }
        }
    }
}

/// Returns `rects` as rectangles of the image's one layer.
fn rect_layers(rects: &[Rect<i32, Device>]) -> Vec<vk::RectLayerKHR> {
    rects
        .iter()
        .map(|rect| vk::RectLayerKHR {
            offset: vk::Offset2D {
                x: rect.origin.x,
                y: rect.origin.y,
            },
            extent: vk::Extent2D {
                width: rect.size.width.max(0) as u32,
                height: rect.size.height.max(0) as u32,
            },
            layer: 0,
        })
        .collect()
}
