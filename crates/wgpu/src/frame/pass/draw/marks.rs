//! Mark prims and the composite of their coverage bins.

use crate::buffer::chunks::MARKS_LANE;
use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;
use crate::pipeline::kind::PipelineKind;
use crate::pipeline::marks::MarkKind;

/// A draw of one kind of mark prims from one payload.
pub(super) struct MarkDraw {
    pub(super) kind: MarkKind,
    /// Whether the prims add to coverage bins instead of painting.
    pub(super) coverage: bool,
    /// The drawn chunk position of the payload.
    pub(super) position: usize,
    /// The offset of the item's mark draw block.
    pub(super) block: u32,
    /// The prims drawn, counted from the payload start.
    pub(super) instances: [u32; 2],
    /// The atlas page of the payload's shape cells.
    pub(super) page: Option<u32>,
}

impl Recorder<'_> {
    /// Draws mark prims, and reports whether they were drawn.
    pub(super) fn marks_draw(
        &mut self,
        pass: &mut BoundPass<'_>,
        context: Draw<'_>,
        mark: &MarkDraw,
    ) -> bool {
        let payload = self.buffers.chunks.mark_payload(mark.position)[mark.kind.lane()].clone();
        // The instances count from the payload start and never leave the payload. The whole
        // payload stays bound, so a segment at the start of the part still reads its neighbours.
        let start = payload
            .start
            .saturating_add(mark.instances[0])
            .min(payload.end);
        let range = start
            ..payload
                .start
                .saturating_add(mark.instances[1])
                .min(payload.end);
        if range.is_empty() {
            return false;
        }
        let Some(tables) = context.tables else {
            return false;
        };
        let layouts = self.pipelines.layouts();
        let Some(instances) = self
            .buffers
            .instance_bind_group(self.gpu, layouts, MARKS_LANE)
        else {
            return false;
        };
        let Some(payload) = self.buffers.mark_bind_group(self.gpu, layouts, mark.kind) else {
            return false;
        };
        // Placements whose page is gone draw nothing rather than another page's texels.
        let atlas = match mark.page {
            None => None,
            Some(page) => match self.pages.bind_group(page) {
                Some(bind_group) => Some(bind_group),
                None => return false,
            },
        };
        let kind = PipelineKind::marks(mark.kind, mark.coverage);
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        pass.set_bind_group(0, tables, &[context.planned.globals]);
        pass.set_bind_group(1, &instances, &[]);
        pass.set_bind_group(2, &payload, &[mark.block]);
        if let Some(bind_group) = atlas {
            pass.set_bind_group(3, bind_group, &[]);
        }
        pass.draw(0..4, range);
        true
    }

    /// Composites the coverage bins of marks with block offset `block`, and reports whether it
    /// was drawn.
    pub(super) fn marks_composite(
        &mut self,
        pass: &mut BoundPass<'_>,
        context: Draw<'_>,
        block: u32,
    ) -> bool {
        let Some(view) = self.marks.array() else {
            return false;
        };
        let Some(tables) = context.tables else {
            return false;
        };
        let layouts = self.pipelines.layouts();
        let Some(instances) = self
            .buffers
            .instance_bind_group(self.gpu, layouts, MARKS_LANE)
        else {
            return false;
        };
        let Some(bins) = self.buffers.mark_composite_bind_group(
            self.gpu,
            layouts,
            view,
            self.marks.generation(),
        ) else {
            return false;
        };
        let kind = PipelineKind::MarksComposite;
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        pass.set_bind_group(0, tables, &[context.planned.globals]);
        pass.set_bind_group(1, &instances, &[]);
        pass.set_bind_group(2, &bins, &[block]);
        pass.draw(0..4, 0..1);
        true
    }
}
