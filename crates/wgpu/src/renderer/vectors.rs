//! Rasterising a frame's vector passes through the path rasteriser.

use iceblink::Frame;

use crate::gpu::timing::GpuTimer;
use crate::renderer::WgpuRenderer;
use crate::vector::{ResourcedPlan, VectorError, VectorFrame};

impl WgpuRenderer {
    /// Builds the path rasteriser for a target of `size`, if a factory is set and none is built.
    pub(crate) fn build_vectors(&mut self, size: [u32; 2]) {
        if self.vectors.is_none()
            && let Some(factory) = &self.vector_factory
        {
            let raster = factory(&self.gpu, size);
            self.vector_backend = Some(raster.backend());
            self.vectors = Some(self.keeping(raster));
        }
    }

    /// Resources the frame's vector plan and rasterises its first round.
    ///
    /// A plan whose rasteriser runs out of capacity keeps the passes it prepared, and the
    /// shortfall panics when shortfalls are fatal. The first frame with passes and no rasteriser
    /// logs a warning.
    ///
    /// Returns `None` when the frame has no passes, no rasteriser is set, the rasteriser plans
    /// nothing, or the first round fails and keeps no pass.
    pub(crate) fn rasterise_vectors(&mut self, frame: &Frame) -> Option<ResourcedPlan> {
        if frame.vector_plan.is_empty() {
            return None;
        }
        self.build_vectors(frame.size);
        let fatal = self.vector_shortfall_is_fatal;
        let Some(raster) = self.vectors.as_deref_mut() else {
            if !self.vectors_missed {
                self.vectors_missed = true;
                tracing::warn!(
                    passes = frame.vector_plan.passes.len(),
                    "a frame has vector passes and the renderer has no path rasteriser, so paths \
                     and glyphs on the path route draw nothing"
                );
            }
            return None;
        };
        let mut plan = raster.plan(&frame.vector_plan);
        if plan.is_empty() {
            return None;
        }
        // A composite names its pass by index, so a plan that dropped one would draw every later
        // composite from the wrong pass.
        debug_assert_eq!(plan.passes.len(), frame.vector_plan.passes.len());
        match self.prepare_round(frame, &plan, 0) {
            Ok(()) => Some(plan),
            Err(VectorError::OutOfCapacity { detail, prepared }) => {
                self.counters.vector_frames_dropped += 1;
                assert!(
                    !fatal,
                    "the vector rasteriser ran out of capacity and {prepared} of {} passes were \
                     drawn: {detail}",
                    plan.passes.len()
                );
                tracing::warn!(
                    prepared,
                    planned = plan.passes.len(),
                    %detail,
                    "the vector rasteriser ran out of capacity, so this frame draws the passes \
                     that fit"
                );
                plan.passes.truncate(prepared);
                (!plan.is_empty()).then_some(plan)
            }
            Err(failure) => {
                tracing::warn!(%failure, "the vector rasteriser could not prepare this frame");
                self.counters.vector_frames_dropped += 1;
                None
            }
        }
    }

    /// Clears the scratch targets of `round` and rasterises its passes.
    fn prepare_round(
        &mut self,
        frame: &Frame,
        plan: &ResourcedPlan,
        round: usize,
    ) -> Result<(), VectorError> {
        let Some(raster) = self.vectors.as_deref_mut() else {
            return Ok(());
        };

        let span = self
            .timer
            .as_mut()
            .and_then(|timer| timer.open_vectors(&self.gpu));
        // Mandatory before anything reads, or a reused scratch would composite the last round.
        raster.clear_targets(plan, round);
        let tables = &self.buffers.tables;
        let mut vector_frame = VectorFrame {
            plan,
            paths: &frame.paths,
            clips: tables.clips(),
            paints: tables.paints(),
            spaces: tables.spaces(),
            round,
            device: self.gpu.device(),
            queue: self.gpu.queue(),
        };
        let prepared = raster.prepare(&mut vector_frame);
        if let Some(span) = &span {
            GpuTimer::close_vectors(&self.gpu, span);
        }
        prepared
    }

    /// Rasterises a later round, and reports whether it succeeded.
    ///
    /// A shortfall panics when shortfalls are fatal.
    #[must_use]
    pub(crate) fn rasterise_round(
        &mut self,
        frame: &Frame,
        plan: &ResourcedPlan,
        round: usize,
    ) -> bool {
        let Err(failure) = self.prepare_round(frame, plan, round) else {
            return true;
        };
        assert!(
            !(self.vector_shortfall_is_fatal
                && matches!(failure, VectorError::OutOfCapacity { .. })),
            "the vector rasteriser ran out of capacity in round {round}: {failure}"
        );
        tracing::warn!(
            %failure,
            round,
            "the vector rasteriser could not prepare a round, so its passes draw nothing"
        );
        false
    }
}
