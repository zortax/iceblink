//! The `Renderer` contract.

use iceblink::damage::DamageSet;
use iceblink::{
    BufferAge, Frame, FrameOutcome, FrameStats, MemoryReport, RenderCapabilities, RenderTarget,
    SkipReason, SubpixelOrder, TargetPoolReport, VectorBackend, VectorStatus,
};
use iceblink::{Presenter, Renderer};

use crate::renderer::present::{area_of, of_present};
use crate::renderer::{CpuRenderer, MAX_TEXTURE_SIZE};

/// The blend modes the renderer draws exactly: all seventeen.
const BLEND_MODES: u32 = (1 << 17) - 1;

impl Renderer for CpuRenderer {
    fn capabilities(&self) -> RenderCapabilities {
        let kept = !self.buffer_age();
        RenderCapabilities {
            max_texture_size: MAX_TEXTURE_SIZE,
            subpixel_text: true,
            custom_shaders: false,
            vector_compute: false,
            can_shift: kept,
            resident_chunks: self.resident_chunks,
            blend_modes: BLEND_MODES,
            kept_rasters: self.vectors.keeping,
        }
    }

    fn configure(&mut self, target: RenderTarget) {
        // A change of scale or opacity changes what every composed pixel means. A change of size
        // keeps the picture.
        if let Some(held) = self.target {
            self.full_damage_next |= held.scale != target.scale || held.opaque != target.opaque;
        }
        // A frame held for another target is never presented to this one.
        if self.target != Some(target) {
            self.held = false;
            self.reconfigured = true;
        }
        self.target = Some(target);
        let size = [target.size[0].max(1), target.size[1].max(1)];
        let _ = self.presenter.configure(size, target.opaque);
        if self.buffer_age() {
            self.kept.resize(1, 1);
        } else {
            self.kept.resize(size[0], size[1]);
        }
    }

    fn target(&self) -> Option<RenderTarget> {
        self.target
    }

    fn target_age(&self) -> BufferAge {
        let kept = !self.buffer_age() && !self.full_damage_next;
        self.promised_kept.set(kept);
        if kept {
            BufferAge::Kept
        } else {
            BufferAge::Unknown
        }
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        let uploaded = self.apply_work(frame);
        if self.target.is_none() || !self.presenter.is_configured() {
            return FrameOutcome::Skipped(SkipReason::Unconfigured);
        }
        // A frame built for a kept target is culled to its damage, so it cannot be redrawn whole.
        let promised = self.promised_kept.replace(false);
        if promised && self.full_damage_next && !frame.damage.draw.is_full() {
            return FrameOutcome::Skipped(SkipReason::Unacquired);
        }
        let full = self.full_damage_next;
        let everything = DamageSet::full();
        let draw = if full {
            &everything
        } else {
            &frame.damage.draw
        };
        if draw.is_empty() && frame.shifts.is_empty() {
            // A composed frame that never reached the display is presented now.
            if self.unpresented.is_some() && !self.buffer_age() {
                return self.present_composed(FrameStats {
                    bytes_uploaded: uploaded,
                    ..FrameStats::default()
                });
            }
            return FrameOutcome::Skipped(SkipReason::Undamaged);
        }
        let image = if self.buffer_age() {
            match self.presenter.acquired() {
                Some(image) => Some(image),
                None => return FrameOutcome::Skipped(SkipReason::Unacquired),
            }
        } else {
            None
        };
        self.full_damage_next = false;

        let composed = self.compose(frame, draw, full, image);
        self.reconfigured = false;
        let present = if full {
            DamageSet::full()
        } else {
            frame.damage.present
        };
        let stats = FrameStats {
            draw_calls: composed.counts.draws,
            batches_culled: composed.counts.culled,
            vector_passes: frame.vector_plan.passes.len() as u32,
            bytes_uploaded: uploaded,
            chunks_resident: self.resident.len() as u32,
            damage_px: composed.redrawn,
            ..FrameStats::default()
        };

        match image {
            Some(image) => {
                let extent = self.presenter.extent();
                let result = self.presenter.present(image, &present);
                self.finish_present(result);
                of_present(
                    result,
                    FrameStats {
                        present_damage_px: area_of(&present, extent),
                        ..stats
                    },
                )
            }
            None => {
                self.presenter.composed(&present);
                let mut pending = self.unpresented.take().unwrap_or_default();
                pending.absorb_set(&present);
                self.unpresented = Some(pending);
                let outcome = self.present_composed(stats);
                self.held = outcome == FrameOutcome::Skipped(SkipReason::Held);
                outcome
            }
        }
    }

    fn present_held(&mut self) -> FrameOutcome {
        let held = std::mem::take(&mut self.held);
        // A frame composed for an earlier target is not presented to this one.
        if !held && (self.unpresented.is_none() || self.reconfigured) {
            return FrameOutcome::Skipped(SkipReason::Undamaged);
        }
        if self.full_damage_next || self.target.is_none() || !self.presenter.is_configured() {
            return FrameOutcome::Skipped(SkipReason::Unacquired);
        }
        let outcome = self.present_composed(FrameStats::default());
        self.held = outcome == FrameOutcome::Skipped(SkipReason::Held);
        outcome
    }

    fn acquire_block(&self) -> std::time::Duration {
        self.acquire_block
    }

    fn set_subpixel_order(&mut self, order: SubpixelOrder) {
        self.subpixel_order = order;
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        Some(self.presenter.as_mut())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport {
            fixed: 0,
            targets: self.kept.bytes() + self.pooled(),
            atlas: self.pages.bytes(),
            chunks: 0,
            vector: self.vectors.bytes(),
        }
    }

    fn target_pool(&self) -> TargetPoolReport {
        TargetPoolReport {
            resident: self.pooled(),
            lent: 0,
            leases: self.workers.iter().map(|worker| worker.pool.leases()).sum(),
        }
    }

    fn release_cached_targets(&mut self) -> u64 {
        self.workers
            .iter_mut()
            .map(|worker| worker.pool.release_unused())
            .sum()
    }

    fn release_idle_resources(&mut self) -> u64 {
        self.release_cached_targets() + self.vectors.release()
    }

    fn vector_status(&self) -> VectorStatus {
        VectorStatus {
            backend: Some(VectorBackend::Cpu),
            name: "vello_cpu",
            initialized: true,
        }
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        out("frames", self.frames);
        out("workers", self.workers.len() as u64);
        self.vectors.extra_stats(out);
    }

    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn core::any::Any> {
        Some(self)
    }
}

impl CpuRenderer {
    fn pooled(&self) -> u64 {
        self.workers.iter().map(|worker| worker.pool.bytes()).sum()
    }
}
