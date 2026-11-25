//! Images, external textures, shaders and damage declarations.

use std::sync::Arc;

use iceblink::{
    ImageDesc, ImageId, ImageWrite, OutputId, RejectReason, ShaderDesc, ShaderId, TextureDesc,
    TextureId,
};

use crate::naive::commit::check::{creatable, live};
use crate::naive::commit::draft::Draft;
use crate::naive::scene::StoredImage;

pub(super) fn create_image(
    draft: &mut Draft<'_>,
    id: ImageId,
    desc: ImageDesc,
) -> Result<(), RejectReason> {
    creatable(&draft.client.images, id)?;
    if !desc.format.is_valid() {
        return Err(RejectReason::InvalidItem);
    }

    let version = draft.next_version();
    draft
        .client
        .images
        .create(id, StoredImage::new(desc, version))
}

pub(super) fn write_image(
    draft: &mut Draft<'_>,
    id: ImageId,
    write: &ImageWrite,
) -> Result<(), RejectReason> {
    live(&draft.client.images, id)?;
    let version = draft.next_version();
    let image = draft.client.images.get_mut(id).expect("checked above");
    if !image.accepts(write) {
        return Err(RejectReason::InvalidWrite);
    }

    image.write(write, version);
    draft.counters.images_written += 1;
    Ok(())
}

pub(super) fn create_texture(
    draft: &mut Draft<'_>,
    id: TextureId,
    desc: TextureDesc,
) -> Result<(), RejectReason> {
    draft.client.textures.create(id, (id, desc))
}

pub(super) fn texture_frame(draft: &mut Draft<'_>, id: TextureId) -> Result<(), RejectReason> {
    live(&draft.client.textures, id)
}

pub(super) fn create_shader(
    draft: &mut Draft<'_>,
    id: ShaderId,
    desc: ShaderDesc,
) -> Result<(), RejectReason> {
    creatable(&draft.client.shaders, id)?;
    if !desc.mode.is_valid() || !desc.reads.is_valid() || desc.params_size > 64 {
        return Err(RejectReason::InvalidShader);
    }

    draft.client.shaders.create(id, Arc::new(desc))
}

/// Fails with `UnknownOutput` unless `output` exists.
///
/// The declared damage itself is not used.
pub(super) fn declare_damage(draft: &mut Draft<'_>, output: OutputId) -> Result<(), RejectReason> {
    if draft.outputs.contains(&output) {
        Ok(())
    } else {
        Err(RejectReason::UnknownOutput)
    }
}
