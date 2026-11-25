//! The [`Renderer`] implementation, and the frame statistics it reports.

use std::any::Any;
use std::time::Duration;

use iceblink::damage::DamageSet;
use iceblink::geom::Size;
use iceblink::{
    BufferAge, Frame, FrameOutcome, FrameStats, MemoryReport, Presenter, RenderCapabilities,
    RenderTarget, Renderer, SkipReason, SubpixelOrder, TargetPoolReport, VectorStatus,
};

use crate::frame::shift::ShiftScratch;
use crate::renderer::compose::Composition;
use crate::renderer::{WgpuRenderer, outcome};

/// The blend modes the renderer draws exactly: `NORMAL` alone.
const BLEND_MODES: u32 = 1;

impl Renderer for WgpuRenderer {
    fn capabilities(&self) -> RenderCapabilities {
        let device = self.gpu.capabilities();
        let kept = !self.buffer_age();
        RenderCapabilities {
            max_texture_size: device.max_texture_size,
            subpixel_text: device.subpixel_text,
            custom_shaders: device.custom_shaders,
            vector_compute: device.vector_compute,
            can_shift: kept,
            resident_chunks: true,
            blend_modes: BLEND_MODES,
            kept_rasters: self.rasters_kept
                && (self.vectors.is_some() || self.vector_factory.is_some()),
        }
    }

    fn configure(&mut self, target: RenderTarget) {
        // A change of scale or opacity changes what every composed pixel means. A change of size
        // keeps the picture, which the next frame carries over if the target was reallocated.
        if let Some(held) = self.target {
            self.full_damage_next |= held.scale != target.scale || held.opaque != target.opaque;
        }
        // A frame held for another target is never presented to this one.
        if self.target != Some(target) {
            self.held = false;
            self.reconfigured = true;
        }
        self.target = Some(target);
        self.resize(Size::new(target.size[0] as i32, target.size[1] as i32));
        // Built here, so the first frame with paths does not pay for it.
        self.build_vectors(target.size);
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
        self.latency.mark("draw.in");
        self.refreshes = (frame.clock.present_ns, frame.clock.interval_ns);
        if self.gpu.loss().is_lost() {
            return match self.recover() {
                Ok(()) => FrameOutcome::Recovered,
                Err(failure) => {
                    tracing::error!(%failure, "the device was lost and could not be rebuilt");
                    FrameOutcome::Skipped(SkipReason::DeviceUnavailable)
                }
            };
        }

        let mut encoder = self.frame_encoder();
        let uploaded = self.apply_work(&mut encoder, frame);

        // A presenter whose images cannot hold the target, such as a supplied set of another
        // extent, receives nothing.
        if self.target.is_none() || !self.presenter.is_configured() {
            self.submit_frame(encoder);
            return FrameOutcome::Skipped(SkipReason::Unconfigured);
        }
        // A frame built for a kept target is culled to its damage, so it cannot be redrawn whole.
        // It keeps its damage and is built again for a target of unknown age.
        let promised = self.promised_kept.replace(false);
        if promised && self.full_damage_next && !frame.damage.draw.is_full() {
            self.submit_frame(encoder);
            return FrameOutcome::Skipped(SkipReason::Unacquired);
        }
        let full = self.full_damage_next;
        let everything = DamageSet::full();
        let draw = if full {
            &everything
        } else {
            &frame.damage.draw
        };
        if draw.is_empty() && frame.shifts.is_empty() && !self.kept.carrying() {
            self.submit_frame(encoder);
            // A composed frame that was never shown on the display is presented now.
            if self.unpresented.is_some() && !self.buffer_age() {
                self.latency.mark("draw.unpresented");
                return self.gated_present(
                    FrameStats {
                        bytes_uploaded: uploaded,
                        ..FrameStats::default()
                    },
                    None,
                );
            }
            self.latency.mark("draw.undamaged");
            return FrameOutcome::Skipped(SkipReason::Undamaged);
        }
        let image = if self.buffer_age() {
            match self.presenter.acquired() {
                Some(image) => Some(image),
                None => {
                    self.submit_frame(encoder);
                    return FrameOutcome::Skipped(SkipReason::Unacquired);
                }
            }
        } else {
            None
        };
        self.full_damage_next = false;

        let composed = self.compose(encoder, frame, draw, full, image);
        let present = if full {
            DamageSet::full()
        } else {
            frame.damage.present
        };
        let stats = self.frame_stats(frame, &composed, uploaded);
        self.pipelines.borrow().persist();

        match image {
            Some(image) => {
                self.submit_frame(composed.encoder);
                let result = self.presenter.present(image, &present);
                self.finish_present(image, result, &present);
                outcome::of_present(result, stats)
            }
            None => {
                self.presenter.composed(&present);
                let mut pending = self.unpresented.take().unwrap_or_default();
                pending.absorb_set(&present);
                self.unpresented = Some(pending);
                self.gated_present(stats, Some(composed.encoder))
            }
        }
    }

    fn present_held(&mut self) -> FrameOutcome {
        let held = std::mem::take(&mut self.held);
        // A frame composed for an earlier target is not presented to this one.
        if !held && (self.unpresented.is_none() || self.reconfigured) {
            return FrameOutcome::Skipped(SkipReason::Undamaged);
        }
        // A reallocated kept target holds its picture only once the next frame carries it over.
        if self.full_damage_next
            || self.target.is_none()
            || !self.presenter.is_configured()
            || self.gpu.loss().is_lost()
            || self.kept.carrying()
        {
            return FrameOutcome::Skipped(SkipReason::Unacquired);
        }
        // A held frame waits like a fresh one.
        if !self.may_present() {
            self.held = true;
            return FrameOutcome::Skipped(SkipReason::Held);
        }
        self.present_composed(FrameStats::default(), None)
    }

    fn acquire_block(&self) -> Duration {
        self.acquire_block
    }

    fn set_subpixel_order(&mut self, order: SubpixelOrder) {
        self.subpixel_order = order;
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        Some(self.presenter.as_mut())
    }

    fn memory(&self) -> MemoryReport {
        let own = MemoryReport {
            fixed: 0,
            targets: self.kept.bytes()
                + self.groups.bytes()
                + self.marks_scratch.bytes()
                + self.shift_scratch_bytes(),
            atlas: self.pages.bytes() + self.buffers.masks.bytes(),
            chunks: self.buffers.bytes(),
            vector: 0,
        };
        match &self.vectors {
            Some(raster) => own.plus(raster.memory()),
            None => own,
        }
    }

    fn target_pool(&self) -> TargetPoolReport {
        TargetPoolReport {
            resident: self.groups.bytes() + self.shift_scratch_bytes(),
            lent: self.groups.lent_bytes(),
            leases: self.groups.leases(),
        }
    }

    fn release_cached_targets(&mut self) -> u64 {
        let scratch = self.shift_scratch_bytes();
        self.shift_scratch = None;
        // A kept bind group holds its view, so a target is freed only once those are dropped too.
        self.buffers.forget_view_binds();
        self.groups.release_unused() + scratch + self.marks_scratch.release()
    }

    fn release_idle_resources(&mut self) -> u64 {
        // Staging comes back through mapping callbacks, which run only when the device is polled.
        self.gpu.wait();
        let mut freed = self.release_cached_targets();
        freed += self.buffers.release_idle(&self.gpu);
        if let Some(vectors) = self.vectors.as_mut() {
            freed += vectors.release_idle_resources();
        }
        // The device frees dropped resources once it is polled.
        self.gpu.reclaim();
        freed
    }

    fn vector_status(&self) -> VectorStatus {
        let backend = match self.vectors.as_deref() {
            Some(raster) => Some(raster.backend()),
            None => self
                .vector_backend
                .filter(|_| self.vector_factory.is_some()),
        };
        VectorStatus {
            backend,
            name: self.vector_name,
            initialized: self.vectors.is_some(),
        }
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        let counters = &self.counters;
        out("frames", counters.frames);
        out("vector_frames_dropped", counters.vector_frames_dropped);
        out("composites_deferred", counters.deferred);
        out("groups_unisolated", counters.unisolated);
        out("blends_unsupported", counters.unsupported_blends);
        out("table_slots_prepared", counters.table_slots_prepared);
        out("upload_allocations", counters.upload_allocations);
        out("upload_staging_bytes", self.buffers.uploader.bytes());
        out(
            "upload_staging_warm_bytes",
            self.buffers.uploader.warm_bytes(),
        );
        out("page_writes", self.pages.writes());
        out("group_leases", self.groups.leases());
        out("group_targets_degraded", u64::from(self.groups.degraded()));
        out(
            "effects_compiled",
            self.pipelines.borrow().effects().len() as u64,
        );
        if let Some(vectors) = &self.vectors {
            vectors.extra_stats(out);
        }
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl WgpuRenderer {
    /// Returns the statistics of a composed frame, which uploaded `uploaded` bytes of work before
    /// it was composed.
    ///
    /// The chunk bytes uploaded since the last call count toward the frame.
    fn frame_stats(&mut self, frame: &Frame, composed: &Composition, uploaded: u64) -> FrameStats {
        let passes = frame.vector_plan.passes.len() as u32;
        let chunk_bytes = self.buffers.chunks.take_uploaded();
        FrameStats {
            draw_calls: composed.recorded.draw_calls,
            batches_culled: composed.culled,
            vector_passes: passes,
            vector_passes_without_raster: if self.vectors.is_none() { passes } else { 0 },
            bytes_uploaded: uploaded + composed.uploaded + chunk_bytes,
            chunk_bytes_uploaded: chunk_bytes,
            chunks_resident: self.buffers.chunks.resident() as u32,
            damage_px: composed.redrawn,
            gpu_time_ns: self
                .gpu_timing()
                .map(|timing| (timing.total_ms() * 1.0e6) as u64),
            ..FrameStats::default()
        }
    }

    /// Returns the bytes the shift scratch holds.
    fn shift_scratch_bytes(&self) -> u64 {
        self.shift_scratch.as_ref().map_or(0, ShiftScratch::bytes)
    }
}
