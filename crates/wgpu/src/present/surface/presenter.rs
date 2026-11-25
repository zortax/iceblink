//! The presenter contract of a window surface.

use std::any::Any;
use std::sync::Arc;

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BufferAge, GpuUnavailable, PresentCapabilities, PresentFeedback, PresentMode,
    PresentResult, STALE_DAMAGE, SkipReason,
};

use super::{SurfacePresenter, cropped_extent};
use crate::gpu::device::Gpu;
use crate::gpu::formats::Formats;
use crate::present::WgpuPresenter;
use crate::present::native::NativeSwapchain;

/// The debt of every image of wgpu's swap chain.
static FULL: DamageSet<STALE_DAMAGE> = DamageSet::full();

impl Presenter for SurfacePresenter {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: PresentMode::KeptTarget,
            partial_present: self
                .native
                .as_ref()
                .is_some_and(|native| native.partial_present()),
            feedback: self.native.as_ref().is_some_and(NativeSwapchain::timed),
        }
    }

    fn configure(&mut self, size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        let largest = self.gpu.capabilities().max_texture_size;
        let used = [size[0].clamp(1, largest), size[1].clamp(1, largest)];
        self.used = used;
        if let Some(native) = &mut self.native {
            native.set_frame(used);
        }
        let mut size = used;
        if self.crop {
            let current = self
                .configured
                .then_some([self.config.width, self.config.height]);
            size = cropped_extent(size, current, largest);
        }
        // Configuring again waits for the device to go idle, so an unchanged extent keeps the
        // configuration.
        if self.configured && !self.reconfigure && size == [self.config.width, self.config.height] {
            self.pending = None;
            return Ok(());
        }
        self.pending = Some(size);
        Ok(())
    }

    fn acquire(&mut self) -> Acquired {
        if let Some((image, ..)) = &self.acquired {
            return Acquired::Image {
                image: *image,
                age: BufferAge::Unknown,
            };
        }
        self.acquire_image()
    }

    fn acquired(&self) -> Option<u32> {
        match &self.native {
            Some(native) => native.acquired(),
            None => self.acquired.as_ref().map(|(image, ..)| *image),
        }
    }

    fn release(&mut self, image: u32) {
        if let Some(native) = &mut self.native {
            native.release(image);
        } else if self.acquired() == Some(image) {
            self.acquired = None;
        }
    }

    fn composed(&mut self, damage: &DamageSet) {
        if let Some(native) = &mut self.native {
            native.composed(damage);
        }
    }

    fn debt(&self, image: u32) -> &DamageSet<STALE_DAMAGE> {
        match &self.native {
            Some(native) => native.debt(image),
            None => &FULL,
        }
    }

    fn present(&mut self, image: u32, damage: &DamageSet) -> PresentResult {
        if let Some(native) = &mut self.native {
            let Some(answer) = native.present(image, damage) else {
                return PresentResult::Skipped(SkipReason::Unacquired);
            };
            if answer.presents() {
                if answer.reconfigures() {
                    self.reconfigure = true;
                }
                return PresentResult::Presented;
            }
            self.note_failure(answer);
            return PresentResult::Skipped(answer.skip());
        }
        match self.acquired.take() {
            Some((held, texture, _)) if held == image => {
                self.gpu.queue().present(texture);
                PresentResult::Presented
            }
            other => {
                self.acquired = other;
                PresentResult::Skipped(SkipReason::Unacquired)
            }
        }
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn feedback(&mut self, out: &mut Vec<PresentFeedback>) {
        let Some(native) = &mut self.native else {
            return;
        };
        native.presented(&mut self.presented);
        let refresh_ns = native.refresh_ns().unwrap_or(0);
        out.extend(
            self.presented
                .drain(..)
                .map(|(present, presented_ns)| PresentFeedback {
                    image: 0,
                    present,
                    presented_ns,
                    refresh_ns,
                }),
        );
    }

    fn refresh_ns(&self) -> Option<u64> {
        self.native.as_ref().and_then(NativeSwapchain::refresh_ns)
    }

    fn display_depth(&self) -> u32 {
        self.native
            .as_ref()
            .map_or(1, NativeSwapchain::display_depth)
    }

    fn presents(&self) -> u64 {
        self.native.as_ref().map_or(0, NativeSwapchain::presents)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl WgpuPresenter for SurfacePresenter {
    fn formats(&self) -> Formats {
        self.formats
    }

    fn display_full(&self) -> bool {
        self.native
            .as_ref()
            .is_some_and(NativeSwapchain::display_full)
    }

    fn follow_refreshes(&mut self, refresh_ns: u64, interval_ns: u64) {
        if let Some(native) = &self.native {
            native.follow_refreshes(refresh_ns, interval_ns);
        }
    }

    fn extent(&self) -> [u32; 2] {
        if let Some(pending) = self.pending {
            return pending;
        }
        match &self.native {
            Some(native) => native.extent(),
            None => [self.config.width, self.config.height],
        }
    }

    fn image_texture(&self, image: u32) -> Option<&wgpu::Texture> {
        if let Some(native) = &self.native {
            return native.texture(image);
        }
        match &self.acquired {
            Some((held, texture, _)) if *held == image => Some(&texture.texture),
            _ => None,
        }
    }

    fn image_view(&self, image: u32) -> Option<&wgpu::TextureView> {
        if let Some(native) = &self.native {
            return native.view(image);
        }
        match &self.acquired {
            Some((held, _, view)) if *held == image => Some(view),
            _ => None,
        }
    }

    fn keeps_contents(&self) -> bool {
        self.native.is_some()
    }

    fn check_image(&self, image: u32, composed: &wgpu::Texture, size: [u32; 2]) {
        if let Some(native) = &self.native {
            native.verify(image, composed, size);
        }
    }

    fn rebuild(&mut self, _gpu: &Arc<Gpu>) -> Result<(), GpuUnavailable> {
        Err(GpuUnavailable::default().rejected(
            "the window surface",
            "a surface is created again by whatever owns the window",
        ))
    }
}
