//! Recording a planned frame into an encoder.

mod bound;
mod copy;
mod draw;
mod targets;

use iceblink::geom::{Device, Point, Rect, Size};

use crate::effect::EffectKey;
use crate::frame::build::FramePlan;
use crate::frame::segment::{PassLoad, PlannedPass, Segment};
use crate::frame::target::{Composed, TargetRef};
use crate::gpu::device::Gpu;
use crate::gpu::timing::{GpuTimer, PassQueries, TimedWork};
use crate::pages::Pages;
use crate::pipeline::Pipelines;
use crate::renderer::buffers::FrameBuffers;
use crate::target::group_pool::GroupPool;
use crate::target::marks::MarksScratch;
use crate::vector::VectorRaster;

use self::bound::BoundPass;
use self::draw::Draw;

/// What recording issued, and what it planned but could not issue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Recorded {
    pub(crate) draw_calls: u32,
    /// Planned draws the device could not issue.
    pub(crate) dropped: u32,
}

impl core::ops::AddAssign for Recorded {
    fn add_assign(&mut self, other: Self) {
        self.draw_calls += other.draw_calls;
        self.dropped += other.dropped;
    }
}

/// Records planned segments, with everything their draws bind.
pub(crate) struct Recorder<'frame> {
    pub(crate) gpu: &'frame Gpu,
    pub(crate) pipelines: &'frame mut Pipelines,
    pub(crate) buffers: &'frame FrameBuffers,
    pub(crate) pages: &'frame Pages,
    pub(crate) pool: &'frame GroupPool,
    pub(crate) composed: Composed<'frame>,
    /// The filtering sampler textured draws read through.
    pub(crate) sampler: &'frame wgpu::Sampler,
    /// The view of an external quad's texture, if it is bound.
    pub(crate) externals: &'frame dyn Fn(u32) -> Option<&'frame wgpu::TextureView>,
    /// The compiled shader in a frame shader slot.
    pub(crate) effects: &'frame dyn Fn(u32) -> Option<EffectKey>,
    pub(crate) vectors: Option<&'frame dyn VectorRaster>,
    pub(crate) marks: &'frame MarksScratch,
    pub(crate) mark_extent: Size<i32, Device>,
    pub(crate) timer: Option<&'frame mut GpuTimer>,
}

impl<'frame> Recorder<'frame> {
    /// Records `segments` of `plan` into `encoder`, and returns what was issued.
    ///
    /// Consecutive passes over one target that keep its texels share a render pass.
    pub(crate) fn record(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        plan: &FramePlan,
        segments: &[Segment],
    ) -> Recorded {
        let mut recorded = Recorded::default();
        let mut index = 0;
        while index < segments.len() {
            match &segments[index] {
                Segment::Encoder(op) => {
                    self.run(encoder, *op);
                    index += 1;
                }
                Segment::Pass(first) => {
                    let end = run_end(segments, index, first);
                    let run = segments[index..end]
                        .iter()
                        .filter_map(|segment| match segment {
                            Segment::Pass(pass) => Some(pass),
                            Segment::Encoder(_) | Segment::Round(_) => None,
                        });
                    self.record_pass(encoder, plan, run, &mut recorded);
                    index = end;
                }
                Segment::Round(_) => {}
            }
        }
        if recorded.dropped > 0 {
            tracing::debug!(
                dropped = recorded.dropped,
                "planned draws the device could not issue"
            );
        }
        recorded
    }

    /// Records the passes of `run`, which share a target, into one render pass.
    fn record_pass<'a>(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        plan: &FramePlan,
        run: impl Iterator<Item = &'a PlannedPass> + Clone,
        recorded: &mut Recorded,
    ) {
        let Some(planned) = run.clone().next() else {
            return;
        };
        // A target that is gone draws nothing.
        if self.view(planned.target).is_none() {
            return;
        }
        let Some(format) = self.format(planned.target) else {
            return;
        };
        let extent = self.extent(planned.target);
        // Every scissor of the run is empty, so the whole run draws nothing anywhere.
        if run
            .clone()
            .all(|held| scaled(held.scissor, held.target, extent).is_empty())
        {
            return;
        }
        // Taken only for a pass that opens, because a query nothing writes would resolve to noise.
        let queries = self
            .timer
            .as_deref_mut()
            .and_then(|timer| timer.pass(timed_work(planned.target)));
        let Some(view) = self.view(planned.target) else {
            return;
        };
        let mut pass = BoundPass::new(open(encoder, view, planned.load, queries.as_ref(), extent));

        let tables = self
            .buffers
            .frame_bind_group(self.gpu, self.pipelines.layouts());
        for planned in run {
            let scissor = scaled(planned.scissor, planned.target, extent);
            if scissor.is_empty() {
                continue;
            }
            pass.set_scissor_rect(
                scissor.origin.x.max(0) as u32,
                scissor.origin.y.max(0) as u32,
                scissor.size.width.max(0) as u32,
                scissor.size.height.max(0) as u32,
            );
            let context = Draw {
                planned,
                tables: tables.as_ref(),
                format,
            };
            for draw in plan.draws_of(planned) {
                let issued = self.issue(&mut pass, context, draw);
                if issued {
                    recorded.draw_calls += 1;
                } else {
                    recorded.dropped += 1;
                }
            }
        }
    }
}

/// Opens a render pass over `view`, with a viewport of `extent`.
fn open<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: PassLoad,
    queries: Option<&PassQueries>,
    extent: Size<i32, Device>,
) -> wgpu::RenderPass<'encoder> {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("iceblink.pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: match load {
                    PassLoad::Keep => wgpu::LoadOp::Load,
                    PassLoad::Discard => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                },
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: queries.map(PassQueries::render),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_viewport(
        0.0,
        0.0,
        extent.width.max(1) as f32,
        extent.height.max(1) as f32,
        0.0,
        1.0,
    );
    pass
}

/// Returns the end of the run of passes that starts with `first` at `index` and shares its render
/// pass.
fn run_end(segments: &[Segment], index: usize, first: &PlannedPass) -> usize {
    let mut end = index + 1;
    while let Some(Segment::Pass(next)) = segments.get(end) {
        if !shares_a_pass(first, next) {
            break;
        }
        end += 1;
    }
    end
}

/// Returns the work a pass over `target` is timed as.
fn timed_work(target: TargetRef) -> TimedWork {
    match target {
        TargetRef::Pool(_) => TimedWork::Groups,
        TargetRef::Composed | TargetRef::MarksPage(_) => TimedWork::Draws,
    }
}

/// Returns `rect` in the texels of `target`, cut to `extent`.
fn scaled(
    rect: Rect<i32, Device>,
    target: TargetRef,
    extent: Size<i32, Device>,
) -> Rect<i32, Device> {
    target
        .scale()
        .texels_at(rect, target.origin())
        .intersection(Rect::new(Point::new(0, 0), extent))
        .unwrap_or(Rect::ZERO)
}

/// Reports whether `next` can continue the render pass of `first`.
fn shares_a_pass(first: &PlannedPass, next: &PlannedPass) -> bool {
    first.target == next.target
        && first.globals == next.globals
        && matches!(next.load, PassLoad::Keep)
}
