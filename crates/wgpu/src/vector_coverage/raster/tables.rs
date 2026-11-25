//! The copies of the four tables the coverage shader reads.

use crate::upload::Refused;
use crate::vector_coverage::raster::CoverageRaster;

impl CoverageRaster {
    /// Records the copies of the four tables.
    pub(super) fn upload(&mut self, encoder: &mut wgpu::CommandEncoder) -> Result<(), Refused> {
        self.uploader.begin_frame();
        let device = &self.device;
        let uploader = &mut self.uploader;
        self.buffers
            .items
            .upload(device, uploader, encoder, &self.items)?;
        self.buffers
            .segments
            .upload(device, uploader, encoder, &self.segments)?;
        self.buffers
            .runs
            .upload(device, uploader, encoder, &self.runs)?;
        self.buffers
            .bands
            .upload(device, uploader, encoder, &self.bands)?;
        // Before the submission that reads them.
        self.uploader.finish();
        Ok(())
    }
}
