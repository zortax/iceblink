//! The per-frame instances of vector-pass composites.

use crate::buffer::instances::InstanceTable;
use crate::gpu::device::Gpu;
use crate::pipeline::vector::VectorInstance;
use crate::upload::UploadBelt;

/// The composite instances of a frame's vector passes, staged and uploaded into an instance table.
#[derive(Debug)]
pub(crate) struct VectorInstances {
    staged: Vec<VectorInstance>,
    table: InstanceTable,
}

impl VectorInstances {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        Self {
            staged: Vec::new(),
            table: InstanceTable::new(gpu, "iceblink.vector_instances"),
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.staged.clear();
    }

    /// Appends `instances`, and returns the first and how many there are.
    pub(crate) fn stage(
        &mut self,
        instances: impl IntoIterator<Item = VectorInstance>,
    ) -> (u32, u32) {
        let first = self.staged.len() as u32;
        self.staged.extend(instances);
        (first, self.staged.len() as u32 - first)
    }

    /// Records the upload of the staged instances, and returns the bytes uploaded.
    pub(crate) fn upload_with(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
    ) -> u64 {
        self.table.upload(gpu, belt, encoder, &self.staged)
    }

    pub(crate) fn binding(&self) -> wgpu::BindingResource<'_> {
        self.table.binding()
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.table.capacity()
    }

    pub(crate) fn generation(&self) -> u64 {
        self.table.generation()
    }

    /// Frees the staged instances, shrinks the table, and returns the bytes freed.
    pub(crate) fn shrink(&mut self, gpu: &Gpu) -> u64 {
        self.staged.clear();
        self.staged.shrink_to_fit();
        self.table.shrink(gpu)
    }
}
