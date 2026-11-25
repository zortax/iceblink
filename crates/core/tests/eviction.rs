//! Pins, eviction, trimming and forgetting.

mod common;

use common::content::{FakeRaster, cache_config, drain, glyph, input, lower, moved, run, solid};
use iceblink::{Glyph, GlyphFlags, PageOp};
use iceblink_core::content::{ContentCache, ContentConfig, Lowering};

/// Returns a content config of pages 32 texels square, one per pool.
fn tiny() -> ContentConfig {
    ContentConfig {
        page_size: 32,
        max_page_size: 32,
        pages_per_pool: 1,
    }
}

/// Lowers glyphs `ids` of one run and returns the lowering.
fn lower_ids(cache: &mut ContentCache, ids: std::ops::Range<u32>) -> Lowering {
    let run = run(16.0, solid(0), GlyphFlags::default());
    let glyphs: Vec<Glyph> = ids.map(|id| glyph(id, 0.0, 0.0)).collect();
    lower(cache, &input(&run, &glyphs), &moved(0.0, 0.0))
}

#[test]
fn trimming_evicts_only_unpinned_tiles_from_before_the_current_frame() {
    let raster = FakeRaster::shared();
    let mut cache = cache_config(&raster, tiny());
    let pinned = lower_ids(&mut cache, 1..4);
    cache.pin(&pinned.tiles);
    cache.begin_frame();
    let loose = lower_ids(&mut cache, 10..13);
    cache.begin_frame();
    let current = lower_ids(&mut cache, 20..23);

    let freed = cache.trim(0);

    assert!(cache.all_resident(&pinned.tiles));
    assert!(cache.all_resident(&current.tiles));
    assert!(loose.tiles.iter().all(|&key| !cache.is_resident(key)));
    assert_eq!(freed, 0, "the page still holds pinned tiles");
    assert_eq!(cache.evictions(), 3);
    assert!(cache.memory().atlas_pinned_bytes > 0);
    assert!(cache.memory().atlas_pinned_bytes < cache.memory().atlas_bytes);
}

#[test]
fn a_page_emptied_by_eviction_is_destroyed_and_its_bytes_are_returned() {
    let raster = FakeRaster::shared();
    let mut cache = cache_config(&raster, tiny());
    let lowering = lower_ids(&mut cache, 1..4);
    cache.pin(&lowering.tiles);
    drain(&mut cache);
    cache.begin_frame();
    assert_eq!(cache.trim(0), 0);

    cache.unpin(&lowering.tiles);
    let freed = cache.trim(0);

    assert_eq!(freed, 32 * 32);
    assert_eq!(cache.memory().atlas_bytes, 0);
    assert!(matches!(drain(&mut cache)[..], [PageOp::Destroy { .. }]));
}

#[test]
fn a_full_pool_evicts_the_oldest_unpinned_tiles_to_place_new_ones() {
    let raster = FakeRaster::shared();
    let capacity = {
        let mut cache = cache_config(&raster, tiny());
        lower_ids(&mut cache, 33..433).items.len() as u32
    };
    assert!(
        capacity > 4 && capacity < 400,
        "the page holds {capacity} glyphs"
    );

    let mut cache = cache_config(&raster, tiny());
    let old = lower_ids(&mut cache, 33..33 + capacity);
    cache.begin_frame();
    let new = lower_ids(&mut cache, 500..500 + capacity).items.len();

    assert_eq!(new, capacity as usize);
    assert!(old.tiles.iter().all(|&key| !cache.is_resident(key)));
    assert!(cache.stats().tiles_evicted >= u64::from(capacity));
}

#[test]
fn an_evicted_glyph_is_rasterised_again_when_next_lowered() {
    let raster = FakeRaster::shared();
    let mut cache = cache_config(&raster, tiny());
    lower_ids(&mut cache, 1..2);
    cache.begin_frame();
    cache.trim(0);

    let again = lower_ids(&mut cache, 1..2);

    assert_eq!(again.items.len(), 1);
    assert_eq!(raster.count(), 2);
    assert_eq!(cache.stats().glyphs_rebuilt, 1);
}

#[test]
fn forgetting_drops_pinned_tiles_and_destroys_every_page() {
    let raster = FakeRaster::shared();
    let mut cache = cache_config(&raster, ContentConfig::default());
    let lowering = lower_ids(&mut cache, 1..5);
    cache.pin(&lowering.tiles);
    drain(&mut cache);

    let freed = cache.forget_caches();

    assert_eq!(freed, 1024 * 1024);
    assert_eq!(cache.memory().atlas_bytes, 0);
    assert_eq!(cache.memory().atlas_pinned_bytes, 0);
    assert!(lowering.tiles.iter().all(|&key| !cache.is_resident(key)));
    assert!(matches!(drain(&mut cache)[..], [PageOp::Destroy { .. }]));
    let relowered = lower_ids(&mut cache, 1..5);
    assert_eq!(relowered.items.len(), 4);
    assert_eq!(raster.count(), 8);
}

#[test]
fn a_budget_evicts_only_when_the_pages_exceed_it() {
    let raster = FakeRaster::shared();
    let mut cache = cache_config(&raster, tiny());
    lower_ids(&mut cache, 1..3);
    cache.begin_frame();

    assert_eq!(cache.enforce_budget(None), 0);
    assert_eq!(cache.enforce_budget(Some(32 * 32)), 0);
    assert_eq!(cache.enforce_budget(Some(0)), 32 * 32);
}

#[test]
fn a_device_reset_drops_every_tile_without_destroying_pages() {
    let raster = FakeRaster::shared();
    let mut cache = cache_config(&raster, ContentConfig::default());
    let lowering = lower_ids(&mut cache, 1..5);
    cache.pin(&lowering.tiles);
    drain(&mut cache);
    lower_ids(&mut cache, 5..7);

    cache.reset_device();

    assert!(
        drain(&mut cache).is_empty(),
        "the lost pages need no destruction"
    );
    assert_eq!(cache.memory().atlas_bytes, 0);
    lower_ids(&mut cache, 1..3);
    assert!(matches!(drain(&mut cache)[0], PageOp::Create { .. }));
}
