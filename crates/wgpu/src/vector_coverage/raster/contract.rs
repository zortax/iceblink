//! The [`VectorRaster`] implementation.

use iceblink::{MemoryReport, VectorBackend, VectorPlan};

use crate::upload::Refused;
use crate::vector::{
    Layering, ResourcedPlan, VectorError, VectorFrame, VectorRaster, VectorTarget,
};
use crate::vector_coverage::raster::{CoverageRaster, Rasterised};

impl VectorRaster for CoverageRaster {
    fn backend(&self) -> VectorBackend {
        VectorBackend::Raster
    }

    fn flat_gradients(&self) -> bool {
        true
    }

    fn plan(&mut self, plan: &VectorPlan) -> ResourcedPlan {
        if plan.is_empty() {
            return ResourcedPlan::default();
        }
        // A frame plans once and may prepare a round more than once, so the frame starts here.
        self.last = Rasterised::default();
        self.outlines.begin_frame();
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
        self.clear_tables();
        if frame.is_empty() {
            return Ok(());
        }

        let round = frame.passes();
        let overflow = {
            let _stage = tracing::debug_span!("coverage.collect").entered();
            self.collect_round(frame, round.clone())
        };
        self.group(frame.plan, round);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("iceblink.vector.coverage"),
            });
        {
            let _stage = tracing::debug_span!(
                "coverage.upload",
                items = self.items.len(),
                segments = self.segments.len(),
                bands = self.bands.len(),
                runs = self.runs.len()
            )
            .entered();
            self.upload(&mut encoder).map_err(|Refused(error)| {
                self.uploader.finish();
                VectorError::Device {
                    detail: error.to_string(),
                }
            })?;
        }
        {
            let _stage = tracing::debug_span!("coverage.record").entered();
            self.record(&mut encoder)?;
        }
        self.queue.submit([encoder.finish()]);
        self.uploader.recall();

        if self.last.unclippable > 0 {
            tracing::warn!(
                paths = self.last.unclippable,
                "paths left undrawn because a residual clip has no outline"
            );
        }
        match overflow {
            None => Ok(()),
            Some(prepared) => Err(self.out_of_capacity(prepared)),
        }
    }

    fn view(&self, target: VectorTarget) -> Option<&wgpu::TextureView> {
        self.scratch.straight(u32::try_from(target.0).ok()?)
    }

    fn texture(&self, target: VectorTarget) -> Option<(&wgpu::Texture, u32)> {
        let texture = self
            .scratch
            .straight_texture(u32::try_from(target.0).ok()?)?;
        Some((texture, 0))
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport {
            vector: self.scratch.bytes()
                + self.buffers.items.capacity()
                + self.buffers.segments.capacity()
                + self.buffers.runs.capacity()
                + self.buffers.bands.capacity()
                + self.uploader.bytes(),
            ..MemoryReport::default()
        }
    }

    fn release_idle_resources(&mut self) -> u64 {
        let mut freed = self.scratch.release();
        freed += self.buffers.items.shrink(&self.device);
        freed += self.buffers.segments.shrink(&self.device);
        freed += self.buffers.runs.shrink(&self.device);
        freed += self.buffers.bands.shrink(&self.device);
        freed += self.uploader.release_idle();
        for held in [&mut self.segments, &mut self.flattened] {
            held.clear();
            held.shrink_to_fit();
        }
        self.items = Vec::new();
        self.runs = Vec::new();
        self.tally = Vec::new();
        self.bands = Vec::new();
        freed
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        out("coverage.clip_layers", u64::from(self.last.clip_layers));
        out("coverage.unclippable", u64::from(self.last.unclippable));
        out("coverage.unpaintable", u64::from(self.last.unpaintable));
        out("coverage.segments", u64::from(self.last.segments));
        out("coverage.scratch_layers", u64::from(self.scratch.layers()));
        out("coverage.scratch_bytes", self.scratch.bytes());
        out("coverage.bytes_uploaded", self.uploader.uploaded());
        out("coverage.upload_chunks", self.uploader.allocations());
    }
}

impl CoverageRaster {
    /// Empties the tables and pass spans collected for the last round.
    fn clear_tables(&mut self) {
        self.segments.clear();
        self.bands.clear();
        self.runs.clear();
        self.items.clear();
        self.spans.clear();
    }

    /// Returns the error of a round whose pass `prepared` did not fit.
    fn out_of_capacity(&self, prepared: usize) -> VectorError {
        VectorError::OutOfCapacity {
            detail: format!(
                "pass {prepared} does not fit the tables of a texture {} texels tall",
                self.device.limits().max_texture_dimension_2d
            ),
            prepared,
        }
    }
}
