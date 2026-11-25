//! A device and the pipelines built on it.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use crate::gpu::device::Gpu;
use crate::gpu::formats::Formats;
use crate::pipeline::Pipelines;
use crate::pipeline::kind::PipelineKind;

/// A device and the pipelines built on it.
pub(crate) struct DeviceState {
    pub(crate) gpu: Arc<Gpu>,
    pub(crate) pipelines: Rc<RefCell<Pipelines>>,
}

impl DeviceState {
    /// Returns `gpu` with its pipelines, cached in `cache` when given.
    pub(crate) fn new(gpu: Arc<Gpu>, cache: Option<&Path>) -> Rc<Self> {
        let pipelines = Rc::new(RefCell::new(Pipelines::new(&gpu, cache)));
        Rc::new(Self { gpu, pipelines })
    }

    /// Builds every pipeline kind for the format it draws into.
    ///
    /// Blits draw into the present attachment of `formats`, kinds that add coverage into coverage
    /// targets, and every other kind into the scene format.
    pub(crate) fn prepare(&self, formats: Formats) {
        let wanted = PipelineKind::ALL.map(|kind| {
            let format = match kind {
                PipelineKind::Blit | PipelineKind::BlitUndoSrgb => formats.present_attachment(),
                _ if kind.adds_coverage() => PipelineKind::COVERAGE_FORMAT,
                _ => formats.scene,
            };
            (kind, format)
        });
        self.pipelines.borrow_mut().prepare(&self.gpu, &wanted);
    }
}
