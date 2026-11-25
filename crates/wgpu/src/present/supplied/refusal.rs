//! The checks a set of supplied textures passes before frames are copied into it.

use super::Supplied;

impl Supplied {
    /// Returns why `textures` cannot be presented into as one set, or `None` when they can.
    ///
    /// A set is refused when it is empty, or when a texture:
    ///
    /// 1. lacks `RENDER_ATTACHMENT` usage,
    /// 2. is not two-dimensional,
    /// 3. has more than one mip level, sample or layer,
    /// 4. differs from the first texture in format or extent.
    ///
    /// ```
    /// use iceblink_wgpu::Supplied;
    ///
    /// assert!(Supplied::unusable(&[]).is_some());
    /// ```
    pub fn unusable(textures: &[wgpu::Texture]) -> Option<String> {
        let Some(first) = textures.first() else {
            return Some("the set is empty, so it states no format and no extent".to_owned());
        };
        textures.iter().enumerate().find_map(|(slot, texture)| {
            refused(slot, texture).or_else(|| disagrees(slot, texture, first))
        })
    }
}

/// Returns why a frame cannot be copied into `texture`, or `None` when it can.
fn refused(slot: usize, texture: &wgpu::Texture) -> Option<String> {
    if !texture
        .usage()
        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        return Some(format!(
            "texture {slot} is {:?}, and a frame is copied into it as a RENDER_ATTACHMENT",
            texture.usage()
        ));
    }
    if texture.dimension() != wgpu::TextureDimension::D2 {
        return Some(format!(
            "texture {slot} is {:?}, and a frame is copied into a two-dimensional texture",
            texture.dimension()
        ));
    }
    let counts = [
        ("mip levels", texture.mip_level_count()),
        ("samples", texture.sample_count()),
        ("layers", texture.depth_or_array_layers()),
    ];
    counts
        .into_iter()
        .find(|(_, count)| *count != 1)
        .map(|(what, count)| format!("texture {slot} has {count} {what}, and a frame fills one"))
}

/// Returns how `texture` differs from `first` in format or extent, or `None` when it agrees.
fn disagrees(slot: usize, texture: &wgpu::Texture, first: &wgpu::Texture) -> Option<String> {
    if texture.format() != first.format() {
        return Some(format!(
            "texture {slot} is {:?} and the first is {:?}, and one set has one format",
            texture.format(),
            first.format()
        ));
    }
    if texture.size() != first.size() {
        return Some(format!(
            "texture {slot} is {}x{} and the first is {}x{}, and one set has one extent",
            texture.width(),
            texture.height(),
            first.width(),
            first.height()
        ));
    }
    None
}
