//! Applying a frame's work and composing its damage.

use iceblink::Frame;
use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Rect, Size};

use crate::frame::build::{ComposedTarget, FramePlan, PlanBuilder, PlanResources};
use crate::frame::damage;
use crate::frame::pass::Recorded;
use crate::frame::plan::{PlanInputs, draw_rects, plan_segments};
use crate::renderer::WgpuRenderer;
use crate::vector::ResourcedPlan;

impl WgpuRenderer {
    /// Applies the frame's page and shader work, chunk notes and table deltas, and returns the
    /// bytes uploaded.
    pub(super) fn apply_work(&mut self, encoder: &mut wgpu::CommandEncoder, frame: &Frame) -> u64 {
        self.counters.frames += 1;
        self.buffers.begin_frame();
        if let Some(timer) = &mut self.timer {
            timer.begin(&self.gpu);
        }

        self.latency.mark("r.work");
        let mut uploaded = self.pages.apply(
            &self.gpu,
            &mut self.buffers.uploader,
            encoder,
            &frame.resources.pages,
        );
        self.effects.apply(
            &self.gpu,
            self.pipelines.borrow_mut().effects_mut(),
            &frame.resources.shaders,
        );
        self.buffers.tables.apply(&frame.tables);
        self.counters.table_slots_prepared += self.buffers.tables.prepared();
        self.buffers.masks.update(
            &self.gpu,
            encoder,
            &mut self.buffers.tables,
            &self.pages,
            &frame.resources.pages,
        );
        self.buffers.resolve_chunks(&self.gpu, encoder, frame);
        uploaded += self.buffers.upload_tables(&self.gpu, encoder);
        uploaded += self.buffers.upload_orders(&self.gpu, encoder);

        let allocations = self.buffers.uploader.allocations();
        self.counters.upload_allocations += allocations - self.buffers.allocations_counted;
        self.buffers.allocations_counted = allocations;
        uploaded
    }

    /// Applies the frame's shifts and redraws `draw` into the composed target, the acquired
    /// `image` in buffer-age mode.
    ///
    /// `full` redraws the whole target. Shifts apply only to the kept target.
    pub(super) fn compose(
        &mut self,
        mut encoder: wgpu::CommandEncoder,
        frame: &Frame,
        draw: &DamageSet,
        full: bool,
        image: Option<u32>,
    ) -> Composition {
        let used = match self.composed(image) {
            Some(composed) => composed.used,
            None => self.kept.used(),
        };
        self.buffers.stage_effect_params();

        self.latency.mark("r.vectors");
        let vectors = self.rasterise_vectors(frame);

        self.latency.mark("r.plan");
        let rects = draw_rects(frame, draw, used);
        let redrawn: u64 = rects.iter().map(|rect| damage::area(*rect)).sum();
        let plan = self.plan(
            frame,
            rects,
            full || draw.is_full(),
            image,
            used,
            vectors.as_ref(),
        );
        self.count(&plan);

        self.composed_rects.clear();
        self.composed_rects.extend_from_slice(&plan.rects);
        if image.is_none() {
            self.carry_and_shift(&mut encoder, frame);
        }
        damage::rows_of(&self.composed_rects, &mut self.composed_rows);
        let uploaded = self.buffers.upload_blocks(&self.gpu, &mut encoder);
        // Every staged write is unmapped before the first submission, a vector round's included.
        self.buffers.uploader.finish();

        self.latency.mark("r.record");
        let recorded = self.record_runs(&mut encoder, frame, &plan, vectors.as_ref(), image);
        self.groups.release_all();
        self.reconfigured = false;

        Composition {
            encoder,
            recorded,
            redrawn,
            uploaded,
            culled: plan.culled,
        }
    }

    /// Plans the draws of `rects` within `used`, the whole of it when `full` is set.
    fn plan(
        &mut self,
        frame: &Frame,
        rects: Vec<Rect<i32, Device>>,
        full: bool,
        image: Option<u32>,
        used: Rect<i32, Device>,
        vectors: Option<&ResourcedPlan>,
    ) -> FramePlan {
        self.marks_scratch.ensure(
            &self.gpu,
            Size::new(
                frame.mark_plan.extent[0] as i32,
                frame.mark_plan.extent[1] as i32,
            ),
            frame.mark_plan.pages,
        );

        let chunks = &self.buffers.chunks;
        let marks = |position: usize| chunks.marks_item(frame, position);
        let cells =
            |position: usize| chunks.mark_payload(position)[crate::buffer::chunks::CELLS].start;
        let externals = &self.externals;
        let external = |index: u32| {
            frame
                .externals
                .get(index as usize)
                .and_then(|held| externals.premultiplied(held))
        };
        let inputs = PlanInputs {
            frame,
            marks: &marks,
            cells: &cells,
            externals: &external,
            vectors,
            used,
        };

        let format = match image {
            Some(_) => self.presenter.formats().present_attachment(),
            None => self.kept.format(),
        };
        let allocated = self
            .composed(image)
            .map_or(used.size, |composed| composed.allocated());
        let resources = PlanResources {
            pool: &mut self.groups,
            globals: &mut self.buffers.globals,
            blocks: &mut self.buffers.blocks,
            vectors: &mut self.buffers.vectors,
            mark_draws: &mut self.buffers.mark_draws,
            effect_offsets: &self.buffers.effect_offsets,
            effects: &self.effects,
        };
        let composed = ComposedTarget {
            region: used.size,
            format,
            extent: allocated,
        };
        let builder = PlanBuilder::new(
            &self.gpu,
            resources,
            self.subpixel_order,
            (frame.shader_clock, frame.scale),
            composed,
        );
        plan_segments(builder, &inputs, rects, full)
    }

    /// Adds what `plan` deferred, left unisolated or blended unsupported to the counters.
    fn count(&mut self, plan: &FramePlan) {
        self.counters.deferred += u64::from(plan.deferred);
        self.counters.unisolated += u64::from(plan.unisolated);
        if plan.unsupported_blends > 0 {
            self.counters.unsupported_blends += u64::from(plan.unsupported_blends);
            tracing::debug!(
                groups = plan.unsupported_blends,
                "groups blending by a mode other than normal composite as normal"
            );
        }
    }

    /// Records the carry-over into a reallocated kept target and the frame's shifts, and adds the
    /// rectangles they write to the composed rectangles.
    fn carry_and_shift(&mut self, encoder: &mut wgpu::CommandEncoder, frame: &Frame) {
        // A target reallocated by a resize first receives the picture of the one it replaced.
        if let Some(carried) = self.kept.carry(encoder) {
            self.latency.note("r.carry", || format!("{carried:?}"));
            self.composed_rects.push(self.kept.used());
        }

        // Before anything the frame draws: the passes redraw what the move exposes.
        for shift in &frame.shifts {
            if let Some(moved_to) = shift.destination() {
                self.composed_rects.push(moved_to);
            }
            crate::frame::shift::apply(
                &self.gpu,
                encoder,
                self.kept.texture(),
                &mut self.shift_scratch,
                *shift,
            );
        }
    }

    /// Records the segments of `plan`, rasterising each later vector round before the segments
    /// that composite it, and returns what was recorded.
    ///
    /// The work recorded so far is submitted before each later round.
    fn record_runs(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        plan: &FramePlan,
        vectors: Option<&ResourcedPlan>,
        image: Option<u32>,
    ) -> Recorded {
        let mut recorded = Recorded::default();
        let mut dropped = false;
        for (round, segments) in plan.runs() {
            if let (Some(round), Some(vectors)) = (round, vectors) {
                let finished = core::mem::replace(encoder, self.frame_encoder());
                self.gpu.queue().submit([finished.finish()]);
                dropped |= !self.rasterise_round(frame, vectors, round);
            }
            recorded += self.record(encoder, plan, segments, image, frame);
        }
        if dropped {
            self.counters.vector_frames_dropped += 1;
        }
        recorded
    }
}

/// The encoder and counts of a composed frame.
pub(super) struct Composition {
    pub(super) encoder: wgpu::CommandEncoder,
    pub(super) recorded: Recorded,
    /// The pixels of the rectangles redrawn.
    pub(super) redrawn: u64,
    /// The bytes of the plan's staged blocks uploaded.
    pub(super) uploaded: u64,
    pub(super) culled: u32,
}
