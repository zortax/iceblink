//! The textures bound to clients' external texture ids.

use std::collections::HashMap;

use iceblink::{ClientId, FrameExternal, TextureId};

/// A bound texture's view, and whether its texels are premultiplied.
#[derive(Debug)]
struct Bound {
    view: wgpu::TextureView,
    premultiplied: bool,
}

/// The textures external quads draw, by client and texture id.
#[derive(Debug, Default)]
pub(crate) struct Externals {
    bound: HashMap<(ClientId, TextureId), Bound>,
}

impl Externals {
    /// Binds `texture` to texture `id` of `client`, replacing an earlier binding.
    pub(crate) fn bind(
        &mut self,
        client: ClientId,
        id: TextureId,
        texture: &wgpu::Texture,
        premultiplied: bool,
    ) {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bound.insert(
            (client, id),
            Bound {
                view,
                premultiplied,
            },
        );
    }

    /// Removes the binding of texture `id` of `client`.
    pub(crate) fn unbind(&mut self, client: ClientId, id: TextureId) {
        self.bound.remove(&(client, id));
    }

    /// Reports whether the texture of `external` is premultiplied, or `None` when none is bound.
    pub(crate) fn premultiplied(&self, external: &FrameExternal) -> Option<bool> {
        self.bound
            .get(&(external.client, external.texture))
            .map(|bound| bound.premultiplied)
    }

    /// Returns the view of the texture of `external`, or `None` when none is bound.
    pub(crate) fn view(&self, external: &FrameExternal) -> Option<&wgpu::TextureView> {
        self.bound
            .get(&(external.client, external.texture))
            .map(|bound| &bound.view)
    }
}
