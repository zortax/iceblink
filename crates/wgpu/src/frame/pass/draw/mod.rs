//! Issuing one planned draw into a pass, with everything it binds.

mod batch;
mod external;
mod filter;
mod marks;
mod shaded;
mod textured;
mod vector;

use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::segment::{PlannedDraw, PlannedPass};
use crate::pipeline::kind::PipelineKind;

use self::marks::MarkDraw;

/// What every draw of one scissor of a pass shares.
#[derive(Clone, Copy)]
pub(in crate::frame::pass) struct Draw<'a> {
    /// The pass the draws belong to.
    pub(in crate::frame::pass) planned: &'a PlannedPass,
    /// The frame tables, if they could be bound.
    pub(in crate::frame::pass) tables: Option<&'a wgpu::BindGroup>,
    /// The format of the pass's target.
    pub(in crate::frame::pass) format: wgpu::TextureFormat,
}

impl<'frame> Recorder<'frame> {
    /// Issues `draw`, and reports whether it was drawn.
    pub(in crate::frame::pass) fn issue<'pass>(
        &mut self,
        pass: &mut BoundPass<'pass>,
        context: Draw<'_>,
        draw: &PlannedDraw,
    ) -> bool
    where
        'frame: 'pass,
    {
        match draw {
            PlannedDraw::Clear => self.clear(pass, context),
            PlannedDraw::Batch(batch) => self.batch(pass, context, batch),
            PlannedDraw::Blur {
                source,
                params,
                downsample,
            } => {
                let kind = if *downsample {
                    PipelineKind::BlurDownsample
                } else {
                    PipelineKind::BlurAxis
                };
                self.textured(pass, context, kind, *source, *params, false)
            }
            PlannedDraw::Effect {
                source,
                shader,
                params,
                block,
            } => self.effect_filter(pass, context, *source, *shader, [*params, *block]),
            PlannedDraw::Composite { source, params } => self.textured(
                pass,
                context,
                PipelineKind::Composite,
                *source,
                *params,
                true,
            ),
            PlannedDraw::Vector {
                target,
                first,
                count,
            } => self.vector(pass, context, *target, *first..*first + *count),
            PlannedDraw::Marks {
                kind,
                coverage,
                position,
                block,
                instances,
                page,
            } => {
                let mark = MarkDraw {
                    kind: *kind,
                    coverage: *coverage,
                    position: *position,
                    block: *block,
                    instances: *instances,
                    page: *page,
                };
                self.marks_draw(pass, context, &mark)
            }
            PlannedDraw::MarksComposite { block } => self.marks_composite(pass, context, *block),
            PlannedDraw::External { index, params } => {
                self.external(pass, context, *index, *params)
            }
        }
    }

    /// Clears the scissor to transparent.
    fn clear(&mut self, pass: &mut BoundPass<'_>, context: Draw<'_>) -> bool {
        let kind = PipelineKind::DamageClear;
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        pass.draw(0..4, 0..1);
        true
    }
}
