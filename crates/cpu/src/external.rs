//! Images bound to clients' external texture slots, which external quads draw.

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::{ClientId, FrameExternal, TextureId};

use crate::math::Floor as _;
use crate::math::vec::mix4;
use crate::target::from_byte;

/// An image the renderer did not draw, such as a video frame, with eight-bit RGBA texels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalImage {
    width: u32,
    height: u32,
    texels: Vec<[u8; 4]>,
    premultiplied: bool,
}

impl ExternalImage {
    /// Returns an image of `width` by `height` over `rgba`, four bytes per texel row by row.
    ///
    /// The colour channels are already scaled by alpha when `premultiplied` is set, and straight
    /// otherwise.
    ///
    /// # Panics
    ///
    /// Panics if `rgba` does not hold four bytes for each texel.
    pub fn new(width: u32, height: u32, rgba: &[u8], premultiplied: bool) -> Self {
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "{width}x{height} texels need four bytes each"
        );
        Self {
            width,
            height,
            texels: rgba.as_chunks::<4>().0.to_vec(),
            premultiplied,
        }
    }

    /// Returns the image sampled bilinearly at `uv`, from zero to one across it, premultiplied.
    pub(crate) fn sample(&self, uv: [f32; 2]) -> [f32; 4] {
        if self.width == 0 || self.height == 0 {
            return [0.0; 4];
        }
        let load = |x: i32, y: i32| {
            let x = x.clamp(0, self.width as i32 - 1) as usize;
            let y = y.clamp(0, self.height as i32 - 1) as usize;
            self.texels[y * self.width as usize + x].map(from_byte)
        };
        let at = [
            uv[0] * self.width as f32 - 0.5,
            uv[1] * self.height as f32 - 0.5,
        ];
        let base = [at[0].floored(), at[1].floored()];
        let weight = [at[0] - base[0], at[1] - base[1]];
        let [x, y] = [base[0] as i32, base[1] as i32];
        let low = mix4(load(x, y), load(x + 1, y), weight[0]);
        let high = mix4(load(x, y + 1), load(x + 1, y + 1), weight[0]);
        let texel = mix4(low, high, weight[1]);
        if self.premultiplied {
            return texel;
        }
        [
            texel[0] * texel[3],
            texel[1] * texel[3],
            texel[2] * texel[3],
            texel[3],
        ]
    }
}

/// The images external quads draw, by client and texture id.
#[derive(Debug, Default)]
pub(crate) struct Externals {
    bound: HashMap<(ClientId, TextureId), Arc<ExternalImage>>,
}

impl Externals {
    /// Binds `image` to the slot `id` of `client`, replacing what was bound.
    pub(crate) fn bind(&mut self, client: ClientId, id: TextureId, image: Arc<ExternalImage>) {
        self.bound.insert((client, id), image);
    }

    /// Removes what is bound to the slot `id` of `client`.
    pub(crate) fn unbind(&mut self, client: ClientId, id: TextureId) {
        self.bound.remove(&(client, id));
    }

    /// Returns the image bound to the slot `external` draws, if any.
    pub(crate) fn get(&self, external: &FrameExternal) -> Option<&ExternalImage> {
        self.bound
            .get(&(external.client, external.texture))
            .map(Arc::as_ref)
    }
}
