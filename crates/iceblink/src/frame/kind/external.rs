//! External textures in a frame.

use crate::id::{ClientId, TextureId};
use crate::item::ExternalQuad;

/// A quad of an external texture.
///
/// The renderer's backend companion resolves the texture from `client` and `texture`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameExternal {
    /// The quad, with frame clip and space slots and the client's texture slot.
    pub quad: ExternalQuad,
    /// The client that owns the texture.
    pub client: ClientId,
    /// The texture the quad shows, as the client's id.
    pub texture: TextureId,
}
