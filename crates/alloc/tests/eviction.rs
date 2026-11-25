//! Eviction by generation and down to the soft limit.

mod common;

use common::atlas::{atlas, fill_generation, mono, small, square};
use iceblink_alloc::atlas::{Atlas, AtlasError, AtlasLimits};

#[test]
fn eviction_removes_exactly_the_oldest_generation() {
    let (mut atlas, _sink) = atlas();
    fill_generation(&mut atlas, 0..3, 4);
    fill_generation(&mut atlas, 3..5, 4);
    atlas.begin_frame();

    let mut removed = Vec::new();
    let first = atlas.evict_least_recently_used_into(&mut removed);
    assert_eq!(first.tiles, 3);
    assert_eq!(first.texels, 3 * 16);
    assert_eq!(first.generation, Some(1));
    removed.sort();
    assert_eq!(removed, vec![mono(0), mono(1), mono(2)]);
    assert!((3..5).all(|handle| atlas.contains(mono(handle))));

    let second = atlas.evict_least_recently_used();
    assert_eq!(second.tiles, 2);
    assert!(atlas.is_empty());

    assert!(atlas.evict_least_recently_used().is_empty());
}

#[test]
fn eviction_spares_held_entries_and_entries_used_this_generation() {
    let (mut atlas, _sink) = atlas();
    fill_generation(&mut atlas, 0..3, 4);
    atlas.retain(mono(0));

    atlas.begin_frame();
    assert!(atlas.get(mono(1)).is_some());
    assert!(atlas.used_this_frame(mono(1)));
    assert!(!atlas.used_this_frame(mono(2)));

    let freed = atlas.evict_all_unused();
    assert_eq!(freed.tiles, 1);
    assert!(atlas.contains(mono(0)));
    assert!(atlas.contains(mono(1)));
    assert!(!atlas.contains(mono(2)));
}

#[test]
fn a_lookup_moves_an_entry_into_the_current_generation() {
    let (mut atlas, _sink) = atlas();
    fill_generation(&mut atlas, 1..2, 4);
    fill_generation(&mut atlas, 2..3, 4);
    assert!(atlas.get(mono(1)).is_some());

    atlas.begin_frame();
    let freed = atlas.evict_least_recently_used();
    assert_eq!(freed.tiles, 2);
}

#[test]
fn a_full_pool_is_out_of_space_until_eviction_frees_room() {
    let mut atlas = Atlas::new(AtlasLimits {
        max_textures_per_pool: 1,
        ..small()
    });

    let mut stored = 0_u64;
    let error = loop {
        match atlas.get_or_insert(mono(stored), square(16), || vec![0; 256]) {
            Ok(_) => stored += 1,
            Err(error) => break error,
        }
        assert!(
            stored < 1_000,
            "a 64x64 texture cannot hold this many tiles"
        );
    };
    assert!(matches!(error, AtlasError::OutOfSpace { .. }));

    atlas.begin_frame();
    let freed = atlas.evict_all_unused();
    assert_eq!(freed.tiles as u64, stored);
    assert!(
        atlas
            .get_or_insert(mono(stored), square(16), || vec![0; 256])
            .is_ok()
    );
}

#[test]
fn without_a_soft_limit_nothing_is_evicted_to_meet_it() {
    let mut atlas = Atlas::new(small());
    fill_generation(&mut atlas, 0..64, 16);
    let resident = atlas.resident_bytes();
    assert!(resident > 0);

    atlas.begin_frame();
    assert!(atlas.evict_to_soft_limit().is_empty());
    assert_eq!(atlas.resident_bytes(), resident);
}

#[test]
fn an_atlas_over_its_soft_limit_returns_below_it() {
    let one_texture = 64 * 64;
    let mut atlas = Atlas::new(small());
    atlas.set_soft_bytes(Some(2 * one_texture));
    assert_eq!(atlas.limits().soft_bytes, Some(2 * one_texture));

    // Each generation fills a texture of its own.
    for round in 0..4_u64 {
        fill_generation(&mut atlas, round * 16..round * 16 + 16, 16);
    }
    assert!(atlas.resident_bytes() > 2 * one_texture);

    atlas.begin_frame();
    let mut removed = Vec::new();
    let freed = atlas.evict_to_soft_limit_into(&mut removed);
    assert!(!freed.is_empty());
    assert_eq!(removed.len(), freed.tiles);
    assert!(atlas.resident_bytes() <= 2 * one_texture);
}

#[test]
fn the_soft_limit_never_evicts_held_or_used_entries() {
    let mut atlas = Atlas::new(small().with_soft_bytes(0));
    fill_generation(&mut atlas, 0..8, 16);
    atlas.retain(mono(0));
    atlas.begin_frame();
    assert!(atlas.get(mono(1)).is_some());

    let freed = atlas.evict_to_soft_limit();
    assert!(!freed.is_empty());
    assert!(atlas.contains(mono(0)));
    assert!(atlas.contains(mono(1)));
    assert!(atlas.resident_bytes() > 0);
}
