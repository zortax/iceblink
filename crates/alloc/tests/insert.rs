//! Insertion, deferred uploads and refused insertions.

mod common;

use std::sync::Arc;

use common::atlas::{atlas, mono, square};
use iceblink::geom::Size;
use iceblink_alloc::atlas::{AtlasError, AtlasKey, TextureFormat, TextureKind};

#[test]
fn a_miss_allocates_and_a_hit_does_not_build() {
    let (mut atlas, _sink) = atlas();
    let first = atlas
        .get_or_insert(mono(1), square(8), || vec![1; 64])
        .unwrap();
    let second = atlas
        .get_or_insert(mono(1), square(8), || -> Vec<u8> {
            panic!("build must not run on a hit")
        })
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(atlas.len(), 1);
    assert_eq!(atlas.report().pending_uploads, 1);
}

#[test]
fn uploads_wait_for_the_flush() {
    let (mut atlas, mut sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(4), || vec![9; 16])
        .unwrap();
    assert_eq!(sink.bytes_written(), 0);

    assert_eq!(atlas.flush_uploads(&mut sink).unwrap(), 16);
    assert_eq!(sink.writes(), 1);
    assert_eq!(atlas.report().pending_uploads, 0);
}

#[test]
fn a_shared_payload_is_queued_without_a_copy() {
    let (mut atlas, mut sink) = atlas();
    let texels = Arc::new(vec![9_u8; 16]);
    atlas
        .get_or_insert(mono(1), square(4), || Arc::clone(&texels))
        .unwrap();
    assert_eq!(Arc::strong_count(&texels), 2);

    atlas.flush_uploads(&mut sink).unwrap();
    assert_eq!(Arc::strong_count(&texels), 1);
}

#[test]
fn the_bytes_land_in_the_tile_rectangle() {
    let (mut atlas, mut sink) = atlas();
    let tile = atlas
        .get_or_insert(mono(1), square(2), || vec![0x5a; 4])
        .unwrap();
    atlas.flush_uploads(&mut sink).unwrap();

    let x = tile.bounds.origin.x;
    let y = tile.bounds.origin.y;
    assert_eq!(sink.texel(tile.texture, x, y), Some(&[0x5a][..]));
}

#[test]
fn removing_before_the_flush_discards_the_upload() {
    let (mut atlas, mut sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(4), || vec![9; 16])
        .unwrap();
    assert!(atlas.remove(mono(1)));

    assert_eq!(atlas.report().pending_uploads, 0);
    assert_eq!(atlas.report().pending_bytes, 0);
    assert_eq!(atlas.flush_uploads(&mut sink).unwrap(), 0);
}

#[test]
fn pools_are_separate_and_keep_their_formats() {
    let (mut atlas, mut sink) = atlas();
    let mono_tile = atlas
        .get_or_insert(mono(1), square(4), || vec![0; 16])
        .unwrap();
    let color_tile = atlas
        .get_or_insert(AtlasKey::new(1, TextureKind::Color), square(4), || {
            vec![0; 64]
        })
        .unwrap();
    atlas.flush_uploads(&mut sink).unwrap();

    assert_eq!(atlas.len(), 2);
    assert_eq!(atlas.report().textures, 2);
    assert_eq!(mono_tile.texture.format(), TextureFormat::R8Unorm);
    assert_eq!(color_tile.texture.format(), TextureFormat::Rgba8Unorm);
    assert_eq!(
        sink.texel(color_tile.texture, 0, 0).map(<[u8]>::len),
        Some(4)
    );
}

#[test]
fn a_standalone_tile_covers_its_own_texture_and_frees_it_on_removal() {
    let (mut atlas, mut sink) = atlas();
    let key = AtlasKey::new(9, TextureKind::Image);
    let levels = || {
        vec![
            vec![1_u8; 8 * 4 * 4],
            vec![2_u8; 4 * 2 * 4],
            vec![3_u8; 2 * 4],
            vec![4_u8; 4],
        ]
    };

    let tile = atlas
        .insert_standalone(key, Size::new(8, 4), levels)
        .unwrap();
    assert_eq!(tile.bounds.size, Size::new(8, 4));
    assert_eq!(atlas.report().pending_uploads, 4);

    atlas.flush_uploads(&mut sink).unwrap();
    assert_eq!(sink.size_of(tile.texture), Some(Size::new(8, 4)));

    let again = atlas.insert_standalone(key, Size::new(8, 4), || -> Vec<Vec<u8>> {
        panic!("build must not run on a hit")
    });
    assert_eq!(again.unwrap(), tile);

    assert!(atlas.remove(key));
    assert!(atlas.is_fully_reclaimed());
    assert_eq!(atlas.report().textures, 0);
}

#[test]
fn a_standalone_level_with_the_wrong_byte_count_is_refused() {
    let (mut atlas, _sink) = atlas();
    let key = AtlasKey::new(9, TextureKind::Image);
    // Level one is 4x2 and needs 32 bytes.
    let refused = atlas.insert_standalone(key, Size::new(8, 4), || {
        vec![vec![1_u8; 8 * 4 * 4], vec![2_u8; 3]]
    });
    assert!(matches!(refused, Err(AtlasError::WrongByteCount { .. })));
    assert!(!atlas.contains(key));
    assert_eq!(atlas.refusals(), 1);
}

#[test]
fn a_tile_larger_than_any_texture_is_too_large() {
    let (mut atlas, _sink) = atlas();
    let error = atlas
        .get_or_insert(mono(1), square(8192), Vec::new)
        .unwrap_err();
    assert!(matches!(error, AtlasError::TooLarge { .. }));
    assert_eq!(atlas.refusals(), 1);
}

#[test]
fn an_empty_tile_is_out_of_space() {
    let (mut atlas, _sink) = atlas();
    let error = atlas
        .get_or_insert(mono(1), Size::new(0, 4), Vec::new)
        .unwrap_err();
    assert!(matches!(error, AtlasError::OutOfSpace { .. }));
}

#[test]
fn a_wrong_byte_count_frees_the_allocated_space() {
    let (mut atlas, _sink) = atlas();
    let error = atlas
        .get_or_insert(mono(1), square(4), || vec![0; 3])
        .unwrap_err();
    assert!(matches!(error, AtlasError::WrongByteCount { .. }));
    assert!(atlas.is_fully_reclaimed());
    assert!(atlas.is_empty());
    assert_eq!(atlas.refusals(), 1);
}

#[test]
fn hits_count_found_lookups_and_insertions() {
    let (mut atlas, _sink) = atlas();
    assert_eq!(atlas.hits(), 0);

    atlas
        .get_or_insert(mono(1), square(8), || vec![1; 64])
        .unwrap();
    assert_eq!(atlas.hits(), 1);

    assert!(atlas.get(mono(1)).is_some());
    assert!(atlas.get(mono(1)).is_some());
    assert_eq!(atlas.hits(), 3);

    assert!(atlas.get(mono(99)).is_none());
    assert_eq!(atlas.hits(), 3);
}
