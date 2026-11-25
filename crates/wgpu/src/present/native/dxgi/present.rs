//! Releasing a buffer and presenting it with the rectangles that changed.

use iceblink::geom::{Device, Rect};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D12::ID3D12CommandList;
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET, DXGI_PRESENT, DXGI_PRESENT_ALLOW_TEARING,
    DXGI_PRESENT_PARAMETERS,
};
use windows::core::Interface;

use super::Chain;
use crate::present::acquisition::Acquisition;

impl Chain {
    /// Releases the buffer at `slot` and presents it.
    ///
    /// `rects` are the rectangles that changed since the previous present, inside the buffer, and
    /// `None` means the whole buffer.
    pub(crate) fn present(
        &mut self,
        slot: u32,
        rects: Option<&[Rect<i32, Device>]>,
    ) -> Acquisition {
        let (Some(swapchain), Some(buffer)) = (&self.swapchain, self.buffers.get(slot as usize))
        else {
            return Acquisition::Validation;
        };
        let Ok(list) = buffer.release.cast::<ID3D12CommandList>() else {
            return Acquisition::Validation;
        };
        // SAFETY: the list is closed and names the buffer the frame was just copied into.
        unsafe { self.queue.ExecuteCommandLists(&[Some(list)]) };

        let mut dirty: Vec<RECT> = rects
            .unwrap_or_default()
            .iter()
            .map(|rect| RECT {
                left: rect.origin.x,
                top: rect.origin.y,
                right: rect.origin.x + rect.size.width,
                bottom: rect.origin.y + rect.size.height,
            })
            .collect();
        let parameters = DXGI_PRESENT_PARAMETERS {
            DirtyRectsCount: dirty.len() as u32,
            pDirtyRects: if dirty.is_empty() {
                core::ptr::null_mut()
            } else {
                dirty.as_mut_ptr()
            },
            pScrollRect: core::ptr::null_mut(),
            pScrollOffset: core::ptr::null_mut(),
        };
        // Tearing only for `Immediate`, and only where the system allows it, as wgpu presents.
        let (interval, flags) = match self.mode {
            wgpu::PresentMode::Immediate if self.tearing => (0, DXGI_PRESENT_ALLOW_TEARING),
            wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox => (0, DXGI_PRESENT(0)),
            _ => (1, DXGI_PRESENT(0)),
        };
        // SAFETY: the parameters point at `dirty`, which outlives the call, and every rectangle
        // lies inside the buffer.
        let result = unsafe { swapchain.Present1(interval, flags, &parameters) };
        if result == DXGI_ERROR_DEVICE_REMOVED || result == DXGI_ERROR_DEVICE_RESET {
            self.gpu.loss().report(
                wgpu::DeviceLostReason::Unknown,
                "the device was removed while presenting",
            );
            return Acquisition::Validation;
        }
        if result.is_err() {
            tracing::warn!(%result, "a present was refused");
            return Acquisition::Validation;
        }
        Acquisition::Success
    }
}
