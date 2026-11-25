//! Acquiring a swap chain image and submitting the barrier that takes it.

use ash::vk;

use super::{ACQUIRE_TIMEOUT, Chain, FENCE_TIMEOUT};
use crate::present::acquisition::Acquisition;

impl Chain {
    /// Acquires the image the next frame is copied into, and submits the barrier that takes it.
    ///
    /// Returns the answer, with the image's slot and texture when an image was acquired.
    pub(crate) fn acquire(&mut self) -> (Acquisition, Option<(u32, wgpu::Texture)>) {
        if self.swapchain == vk::SwapchainKHR::null() {
            return (Acquisition::Outdated, None);
        }
        let ring = self.next;
        if !self.settle(ring) {
            return (Acquisition::Timeout, None);
        }

        let semaphore = self.acquires[ring].semaphore;
        let held = self.timing.as_ref().map(|timing| timing.hold());
        // SAFETY: the swap chain is live and held against the present waiter, and the semaphore
        // is unsignalled with no pending operation, since the fence settled above says its last
        // wait finished.
        let acquired = unsafe {
            self.swapchain_fns.acquire_next_image(
                self.swapchain,
                ACQUIRE_TIMEOUT,
                semaphore,
                vk::Fence::null(),
            )
        };
        drop(held);

        let (index, suboptimal) = match acquired {
            Ok(acquired) => acquired,
            Err(error) => return (self.classify(error), None),
        };
        let Some(image) = self.images.get_mut(index as usize) else {
            return (Acquisition::Validation, None);
        };
        let command = if image.used { image.again } else { image.first };
        image.used = true;
        let texture = image.texture.clone();

        if let Err(error) = self.submit_acquire_barrier(ring, command) {
            tracing::warn!(%error, "the barrier that takes a swap chain image was refused");
            return (self.classify(error), None);
        }
        self.next = (ring + 1) % self.acquires.len();

        let answer = if suboptimal {
            Acquisition::Suboptimal
        } else {
            Acquisition::Success
        };
        (answer, Some((index, texture)))
    }

    /// Submits the acquire barrier `command`, which waits on the semaphore of the ring entry at
    /// `ring` and signals its fence.
    ///
    /// Fails with the driver's error.
    fn submit_acquire_barrier(
        &mut self,
        ring: usize,
        command: vk::CommandBuffer,
    ) -> Result<(), vk::Result> {
        // The barrier orders against everything submitted after it, so the frame's copy, submitted
        // next, waits for the semaphore through it.
        let entry = &mut self.acquires[ring];
        let waits = [entry.semaphore];
        let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let commands = [command];
        let submit = vk::SubmitInfo::default()
            .wait_semaphores(&waits)
            .wait_dst_stage_mask(&stages)
            .command_buffers(&commands);
        // SAFETY: the command buffer was recorded for simultaneous use and names an image of this
        // swap chain. The fence is unsignalled and not pending. The queue is used only from the
        // thread that draws, which is this one.
        unsafe {
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit), entry.fence)
        }?;
        entry.pending = true;
        Ok(())
    }

    /// Waits until the ring entry at `ring` may be used again, and reports whether it finished in
    /// time.
    pub(super) fn settle(&mut self, ring: usize) -> bool {
        let entry = &mut self.acquires[ring];
        if !entry.pending {
            return true;
        }
        let fences = [entry.fence];
        // SAFETY: the fence belongs to this device and was handed to a submission.
        let waited = unsafe { self.device.wait_for_fences(&fences, true, FENCE_TIMEOUT) };
        if waited.is_err() {
            tracing::warn!("a swap chain barrier did not finish in time");
            return false;
        }
        // SAFETY: the fence is signalled and no pending submission names it.
        if unsafe { self.device.reset_fences(&fences) }.is_err() {
            return false;
        }
        entry.pending = false;
        true
    }
}
