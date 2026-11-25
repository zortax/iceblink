//! Waiting for the swap chain to take a frame, and taking the buffer it goes into.

use windows::Win32::Foundation::WAIT_TIMEOUT;
use windows::Win32::Graphics::Direct3D12::ID3D12CommandList;
use windows::Win32::System::Threading::WaitForSingleObjectEx;
use windows::core::Interface;

use super::{ACQUIRE_TIMEOUT_MS, Chain};
use crate::present::acquisition::Acquisition;

impl Chain {
    /// Waits until the swap chain takes another frame, and returns the answer with the buffer's
    /// slot and texture when one was taken.
    pub(crate) fn acquire(&mut self) -> (Acquisition, Option<(u32, wgpu::Texture)>) {
        let Some(swapchain) = &self.swapchain else {
            return (Acquisition::Outdated, None);
        };
        if let Some(waitable) = self.waitable {
            // SAFETY: the handle is the swap chain's own and stays open while it lives.
            let waited = unsafe { WaitForSingleObjectEx(waitable, ACQUIRE_TIMEOUT_MS, true) };
            if waited == WAIT_TIMEOUT {
                return (Acquisition::Timeout, None);
            }
        }
        // SAFETY: the swap chain is live.
        let slot = unsafe { swapchain.GetCurrentBackBufferIndex() };
        let Some(buffer) = self.buffers.get_mut(slot as usize) else {
            return (Acquisition::Validation, None);
        };
        if buffer.used {
            let Ok(list) = buffer.again.cast::<ID3D12CommandList>() else {
                return (Acquisition::Validation, None);
            };
            // SAFETY: the list is closed and names a buffer of this swap chain, which the frame
            // before last released to the present state.
            unsafe { self.queue.ExecuteCommandLists(&[Some(list)]) };
        }
        buffer.used = true;
        (Acquisition::Success, Some((slot, buffer.texture.clone())))
    }
}
