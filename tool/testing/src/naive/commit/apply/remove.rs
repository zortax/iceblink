//! Removing nodes, slots and resources.

use iceblink::{AnyId, RejectReason};

use crate::naive::commit::draft::Draft;

/// Removes the object `id` refers to, which must be live.
///
/// Whether anything still refers to the object is checked once the transaction is applied.
pub(super) fn remove(draft: &mut Draft<'_>, id: AnyId, index: u32) -> Result<(), RejectReason> {
    let client = &mut draft.client;
    let removed = match id {
        AnyId::Transform(id) => client.transforms.remove(id).is_some(),
        AnyId::Scroll(id) => client.scrolls.remove(id).is_some(),
        AnyId::Clip(id) => client.clips.remove(id).is_some(),
        AnyId::Effect(id) => client.effects.remove(id).is_some(),
        AnyId::Order(id) => client.orders.remove(id).is_some(),
        AnyId::Paint(id) => client.paints.remove(id).is_some(),
        AnyId::Params(id) => client.params.remove(id).is_some(),
        AnyId::Image(id) => client.images.remove(id).is_some(),
        AnyId::Texture(id) => client.textures.remove(id).is_some(),
        AnyId::Shader(id) => client.shaders.remove(id).is_some(),
        // No animation ever starts, so every animation id names one that ended.
        AnyId::Animation(_) => return Ok(()),
    };

    if !removed {
        return Err(RejectReason::StaleId);
    }

    draft.removed.push((id, index));
    Ok(())
}
