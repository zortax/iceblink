//! The textures, views, formats and extents of the targets a frame draws into.

use iceblink::geom::{Device, Size};

use crate::frame::pass::Recorder;
use crate::frame::target::TargetRef;
use crate::pipeline::kind::PipelineKind;

impl Recorder<'_> {
    pub(super) fn view(&self, target: TargetRef) -> Option<&wgpu::TextureView> {
        match target {
            TargetRef::Composed => Some(self.composed.view),
            TargetRef::Pool(slot) => Some(self.pool.view(slot)),
            TargetRef::MarksPage(page) => self.marks.page(page),
        }
    }

    pub(super) fn texture(&self, target: TargetRef) -> Option<&wgpu::Texture> {
        match target {
            TargetRef::Composed => Some(self.composed.texture),
            TargetRef::Pool(slot) => Some(self.pool.texture(slot)),
            TargetRef::MarksPage(_) => None,
        }
    }

    pub(super) fn format(&self, target: TargetRef) -> Option<wgpu::TextureFormat> {
        match target {
            TargetRef::Composed => Some(self.composed.format),
            TargetRef::Pool(slot) => Some(slot.format()),
            TargetRef::MarksPage(_) => Some(PipelineKind::COVERAGE_FORMAT),
        }
    }

    /// Returns the extent of `target` a pass draws over.
    pub(super) fn extent(&self, target: TargetRef) -> Size<i32, Device> {
        match target {
            TargetRef::Composed => self.composed.used.size,
            TargetRef::Pool(slot) => slot.extent(),
            TargetRef::MarksPage(_) => self.mark_extent,
        }
    }
}
