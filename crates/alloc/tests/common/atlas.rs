//! Builders shared by the atlas tests.

use iceblink::geom::{Device, Size};
use iceblink_alloc::atlas::{Atlas, AtlasKey, AtlasLimits, MemorySink, TextureKind};

/// A key of the mono pool.
pub(crate) fn mono(handle: u64) -> AtlasKey {
    AtlasKey::new(handle, TextureKind::Mono)
}

/// A square tile size.
pub(crate) fn square(side: i32) -> Size<i32, Device> {
    Size::new(side, side)
}

/// An atlas with default limits, and a sink for it.
pub(crate) fn atlas() -> (Atlas, MemorySink) {
    (Atlas::new(AtlasLimits::default()), MemorySink::new())
}

/// Limits of 64-texel textures, so a few tiles fill a texture.
pub(crate) fn small() -> AtlasLimits {
    AtlasLimits {
        texture_size: 64,
        max_texture_size: 64,
        max_textures_per_pool: 16,
        soft_bytes: None,
    }
}

/// Starts a generation and inserts a square mono tile of `side` for every handle.
pub(crate) fn fill_generation(atlas: &mut Atlas, handles: std::ops::Range<u64>, side: i32) {
    atlas.begin_frame();
    for handle in handles {
        let texels = (side * side) as usize;
        atlas
            .get_or_insert(mono(handle), square(side), || vec![0; texels])
            .unwrap_or_else(|error| panic!("handle {handle} could not allocate: {error}"));
    }
}

/// Reports whether every pair of cached tiles in one texture is disjoint.
pub(crate) fn no_tile_overlaps(atlas: &Atlas) -> bool {
    let tiles: Vec<_> = atlas.tiles().collect();
    tiles.iter().enumerate().all(|(index, left)| {
        tiles[index + 1..]
            .iter()
            .all(|right| left.texture != right.texture || !left.bounds.intersects(right.bounds))
    })
}
