//! The presenter contract of a supplied set.

use std::any::Any;
use std::sync::Arc;

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BufferAge, GpuUnavailable, PresentCapabilities, PresentFeedback, PresentMode,
    PresentResult, STALE_DAMAGE, SkipReason,
};

use super::Supplied;
use crate::gpu::device::Gpu;
use crate::gpu::formats::Formats;
use crate::present::WgpuPresenter;

impl Presenter for Supplied {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: PresentMode::KeptTarget,
            partial_present: false,
            feedback: false,
        }
    }

    // The textures cannot be reallocated, so an output of another extent fails with
    // `Unconfigured` until the output is configured back or another set is supplied.
    fn configure(&mut self, size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        let wanted = [size[0].max(1), size[1].max(1)];
        let diverged = wanted != self.size;
        if diverged && !self.diverged {
            tracing::warn!(
                ?wanted,
                supplied = ?self.size,
                "the output was configured at an extent the supplied textures do not have, so \
                 frames stop until the extents agree"
            );
        }
        self.diverged = diverged;
        if diverged {
            Err(SkipReason::Unconfigured)
        } else {
            Ok(())
        }
    }

    fn acquire(&mut self) -> Acquired {
        if self.diverged {
            return Acquired::Skipped(SkipReason::Unconfigured);
        }
        let image = *self.acquired.get_or_insert(self.selected);
        Acquired::Image {
            image,
            age: BufferAge::Kept,
        }
    }

    fn acquired(&self) -> Option<u32> {
        self.acquired
    }

    fn release(&mut self, image: u32) {
        if self.acquired == Some(image) {
            self.acquired = None;
        }
    }

    fn composed(&mut self, damage: &DamageSet) {
        self.debts.composed(damage);
        if damage.is_full() {
            self.uncopied.set_full();
        } else {
            for rect in damage.rects() {
                self.uncopied.absorb(*rect);
            }
        }
    }

    fn debt(&self, image: u32) -> &DamageSet<STALE_DAMAGE> {
        if self.consumed {
            &self.uncopied
        } else {
            self.debts.debt(image)
        }
    }

    fn present(&mut self, image: u32, _damage: &DamageSet) -> PresentResult {
        if self.acquired != Some(image) {
            return PresentResult::Skipped(SkipReason::Unacquired);
        }
        self.acquired = None;
        self.debts.paid(image);
        self.uncopied.clear();
        PresentResult::Presented
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn feedback(&mut self, _out: &mut Vec<PresentFeedback>) {}

    fn refresh_ns(&self) -> Option<u64> {
        self.refresh
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl WgpuPresenter for Supplied {
    fn formats(&self) -> Formats {
        self.formats
    }

    fn extent(&self) -> [u32; 2] {
        self.size
    }

    fn image_texture(&self, image: u32) -> Option<&wgpu::Texture> {
        self.textures.get(image as usize)
    }

    fn image_view(&self, image: u32) -> Option<&wgpu::TextureView> {
        self.views.get(image as usize)
    }

    fn keeps_contents(&self) -> bool {
        true
    }

    fn is_configured(&self) -> bool {
        !self.diverged
    }

    fn rebuild(&mut self, _gpu: &Arc<Gpu>) -> Result<(), GpuUnavailable> {
        Err(GpuUnavailable::default().rejected(
            "the supplied textures",
            "the textures belong to whoever supplied them, which supplies a set on the new device",
        ))
    }
}
