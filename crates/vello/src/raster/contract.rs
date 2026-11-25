//! The [`VectorRaster`] implementation of [`VelloRaster`].

use iceblink::{MemoryReport, VectorBackend, VectorPlan};
use iceblink_wgpu::vector::{
    Layering, ResourcedPlan, VectorError, VectorFrame, VectorRaster, VectorTarget,
};

use super::{Encoded, VelloRaster};

impl VectorRaster for VelloRaster {
    fn backend(&self) -> VectorBackend {
        VectorBackend::Compute
    }

    fn plan(&mut self, plan: &VectorPlan) -> ResourcedPlan {
        // A frame without passes runs no rasterisation at all.
        if plan.is_empty() {
            return ResourcedPlan::default();
        }

        // A frame plans once and may prepare a round more than once, so the frame starts here.
        self.encodings.end_frame();
        self.last = Encoded::default();
        self.passes = 0;

        self.regions.clear();
        self.regions
            .extend(plan.passes.iter().map(|planned| planned.region));
        let layering = Layering::of(&self.regions, self.capacity);
        self.depth = layering.layers();
        self.rounds = layering.rounds();

        let (width, height) = layering.extent();
        self.scratch
            .ensure(&self.device, width, height, layering.layers());
        ResourcedPlan::placed(plan, &layering)
    }

    fn clear_targets(&mut self, plan: &ResourcedPlan, round: usize) {
        plan.targets_of(round, &mut self.targets);
        self.scratch.clear(&self.device, &self.queue, &self.targets);
    }

    fn prepare(&mut self, frame: &mut VectorFrame<'_>) -> Result<(), VectorError> {
        self.prepare_round(frame)
    }

    fn view(&self, target: VectorTarget) -> Option<&wgpu::TextureView> {
        self.scratch.view(u32::try_from(target.0).ok()?)
    }

    fn texture(&self, target: VectorTarget) -> Option<(&wgpu::Texture, u32)> {
        self.scratch.texture(u32::try_from(target.0).ok()?)
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport {
            fixed: self.shared.fixed_bytes(),
            vector: self.scratch.bytes(),
            ..MemoryReport::default()
        }
    }

    fn release_idle_resources(&mut self) -> u64 {
        self.scratch.release()
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        let (held, (hits, misses)) = self.cache();
        out("vello.renders", self.renders);
        out("vello.passes", u64::from(self.passes));
        out("vello.encode_hits", hits);
        out("vello.encode_misses", misses);
        out("vello.encodings", held as u64);
        out("vello.clip_layers", u64::from(self.last.clip_layers));
        out("vello.unclippable", u64::from(self.last.unclippable));
        out("vello.unpaintable", u64::from(self.last.unpaintable));
        out("vello.scratch_layers", u64::from(self.scratch.layers()));
        out("vello.scratch_bytes", self.scratch.bytes());
    }
}
