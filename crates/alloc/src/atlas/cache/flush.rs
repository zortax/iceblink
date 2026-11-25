//! Performing the queued texture operations and uploads.

use crate::atlas::cache::Atlas;
use crate::atlas::error::AtlasError;
use crate::atlas::sink::TextureSink;

impl Atlas {
    /// Performs the queued texture creations and destructions, then the queued uploads, against
    /// `sink`.
    ///
    /// Texture operations run in the order they were queued, before any upload. Uploads are
    /// written between [`TextureSink::begin_uploads`] and [`TextureSink::finish_uploads`].
    ///
    /// Returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// [`AtlasError::Sink`] if `sink` refuses a creation, the start of the uploads or a write. The
    /// refused operation and everything after it stay queued.
    pub fn flush_uploads(&mut self, sink: &mut impl TextureSink) -> Result<u64, AtlasError> {
        self.textures.replay(sink)?;
        sink.begin_uploads()?;

        let mut written = 0;
        let mut flushed = 0;
        let mut failure = None;
        for upload in &self.pending {
            match sink.write_texture_mip(
                upload.texture,
                upload.mip,
                upload.bounds,
                upload.texture.format(),
                upload.bytes.as_slice(),
            ) {
                Ok(()) => {
                    written += upload.bytes.len() as u64;
                    flushed += 1;
                }
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }

        // The accepted writes are submitted even when a later one failed.
        sink.finish_uploads();
        self.pending.drain(..flushed);
        self.pending_bytes -= written;
        match failure {
            Some(error) => Err(error.into()),
            None => Ok(written),
        }
    }
}
