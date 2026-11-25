//! Creating and writing images, external textures and shaders.

use std::sync::Arc;

use iceblink::{
    CommitError, ImageDesc, ImageFormat, ImageId, ImageWrite, RejectReason, ShaderDesc, ShaderId,
    TextureDesc, TextureId,
};

use crate::commit::apply::Apply;
use crate::commit::apply::journaled::create;
use crate::scene::record::{ImageRecord, Links, ShaderRecord, TextureRecord};

/// The most bytes a shader's own parameters may take.
const PARAMS_LIMIT: u32 = 64;

impl Apply<'_> {
    pub(crate) fn create_image(&mut self, id: ImageId, desc: ImageDesc) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.images, id.slot(), id.generation())?;
        self.require(desc.format.is_valid(), RejectReason::InvalidItem)?;

        let record = ImageRecord {
            desc,
            links: Links::default(),
            masks: Vec::new(),
        };
        create(
            self.command,
            &mut self.scene.images,
            &mut self.journal.images,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn write_image(
        &mut self,
        id: ImageId,
        write: ImageWrite,
    ) -> Result<(), CommitError> {
        let Some(record) = self.scene.images.get(id.slot(), id.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        self.require(fits(record.desc, &write), RejectReason::InvalidWrite)?;

        self.journal.deferred.image_writes.push((id, write));
        self.counters.images_written += 1;
        Ok(())
    }

    pub(crate) fn create_texture(
        &mut self,
        id: TextureId,
        desc: TextureDesc,
    ) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.textures, id.slot(), id.generation())?;

        // The compositor holds only the slot, and a renderer's companion binds the texture.
        let _ = desc;
        let record = TextureRecord {
            id,
            links: Links::default(),
        };
        create(
            self.command,
            &mut self.scene.textures,
            &mut self.journal.textures,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn texture_frame(&mut self, id: TextureId) -> Result<(), CommitError> {
        self.require(self.live_texture(id), RejectReason::StaleId)?;
        self.journal.deferred.texture_frames.push(id);
        Ok(())
    }

    pub(crate) fn create_shader(
        &mut self,
        id: ShaderId,
        desc: ShaderDesc,
    ) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.shaders, id.slot(), id.generation())?;
        let valid =
            desc.mode.is_valid() && desc.reads.is_valid() && desc.params_size <= PARAMS_LIMIT;
        self.require(valid, RejectReason::InvalidShader)?;

        let record = ShaderRecord {
            desc: Arc::new(desc),
            links: Links::default(),
        };
        create(
            self.command,
            &mut self.scene.shaders,
            &mut self.journal.shaders,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }
}

/// Returns how many bytes one texel of `format` takes.
fn bytes_per_texel(format: ImageFormat) -> usize {
    if format == ImageFormat::A8 { 1 } else { 4 }
}

/// Reports whether `write` lies inside an image of `desc` and its bytes hold every row.
fn fits(desc: ImageDesc, write: &ImageWrite) -> bool {
    let rect = write.rect;
    if rect.origin.x < 0 || rect.origin.y < 0 || rect.size.width < 0 || rect.size.height < 0 {
        return false;
    }

    let right = rect.origin.x as u64 + rect.size.width as u64;
    let bottom = rect.origin.y as u64 + rect.size.height as u64;
    if right > u64::from(desc.width) || bottom > u64::from(desc.height) {
        return false;
    }
    if rect.size.width == 0 || rect.size.height == 0 {
        return true;
    }

    let row = rect.size.width as u64 * bytes_per_texel(desc.format) as u64;
    let stride = u64::from(write.stride);
    let needed = write.offset as u64 + (rect.size.height as u64 - 1) * stride + row;
    stride >= row && needed <= write.bytes.len() as u64
}
