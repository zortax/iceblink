//! Rasterising the new glyphs of many runs at once.

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use common::content::{FakeRaster, cache_with, glyph, input, moved, run, solid};
use iceblink::{Glyph, GlyphFlags, GlyphRun};
use iceblink_core::content::{Lowering, RunJob};

/// Returns `count` distinct glyphs on one line.
fn line(count: u32) -> Vec<Glyph> {
    (0..count)
        .map(|id| glyph(id + 100, id as f32 * 9.0, 20.0))
        .collect()
}

#[test]
fn new_keys_are_rasterised_in_parallel_batches_and_then_hit() {
    let raster = Arc::new(FakeRaster {
        batch_delay: Duration::from_millis(5),
        ..FakeRaster::default()
    });
    let mut cache = cache_with(&raster);
    let runs: Vec<GlyphRun> = (0..4)
        .map(|index| run(12.0 + index as f32, solid(0), GlyphFlags::default()))
        .collect();
    let glyphs = line(100);
    let jobs: Vec<RunJob<'_>> = runs
        .iter()
        .map(|run| RunJob {
            input: input(run, &glyphs),
            target: moved(0.0, 0.0),
        })
        .collect();

    let rasterised = cache.prefetch_glyph_runs(&jobs);
    let mut out = Lowering::default();
    cache.lower_glyph_runs(&jobs, &mut out);

    assert_eq!(rasterised, 400);
    assert_eq!(
        raster.count(),
        400,
        "lowering after the prefetch rasterises nothing"
    );
    assert_eq!(out.items.len(), 400);
    let threads: HashSet<_> = raster
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|(_, thread)| *thread)
        .collect();
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    if cores > 1 {
        assert!(
            threads.len() > 1,
            "{} threads on {cores} cores",
            threads.len()
        );
        assert!(cache.stats().prefetch_batches > 1);
        assert!(!threads.contains(&std::thread::current().id()));
    }
    assert_eq!(cache.stats().glyphs_prefetched, 400);
}

#[test]
fn a_key_is_rasterised_once_however_many_runs_name_it() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(5, 0.0, 0.0), glyph(5, 8.0, 0.0), glyph(6, 16.25, 0.0)];
    let jobs = [
        RunJob {
            input: input(&run, &glyphs),
            target: moved(0.0, 0.0),
        },
        RunJob {
            input: input(&run, &glyphs),
            target: moved(32.0, 4.0),
        },
    ];

    assert_eq!(
        cache.prefetch_glyph_runs(&jobs),
        2,
        "glyph 5 at phase 0 and glyph 6 at phase 1"
    );
    assert_eq!(
        cache.prefetch_glyph_runs(&jobs),
        0,
        "every key now has a tile"
    );
}

#[test]
fn prefetched_tiles_land_where_serial_lowering_puts_them() {
    let glyphs = line(160);
    let run = run(14.0, solid(0), GlyphFlags::default());
    let jobs = [RunJob {
        input: input(&run, &glyphs),
        target: moved(0.0, 0.0),
    }];

    let parallel = {
        let raster = FakeRaster::shared();
        let mut cache = cache_with(&raster);
        let mut out = Lowering::default();
        cache.lower_glyph_runs(&jobs, &mut out);
        common::content::sprite_bounds(&out)
    };
    let serial = {
        let raster = FakeRaster::shared();
        let mut cache = cache_with(&raster);
        common::content::sprite_bounds(&common::content::lower(
            &mut cache,
            &jobs[0].input,
            &jobs[0].target,
        ))
    };
    assert_eq!(parallel, serial);
}

#[test]
fn a_run_with_other_glyphs_than_the_prefetched_one_lowers_its_own() {
    let run = run(14.0, solid(0), GlyphFlags::default());
    let prefetched = line(12);
    let lowered: Vec<Glyph> = prefetched
        .iter()
        .map(|held| glyph(held.id, held.x + 0.25, held.y))
        .collect();
    let target = moved(0.0, 0.0);

    let after_prefetch = {
        let raster = FakeRaster::shared();
        let mut cache = cache_with(&raster);
        cache.prefetch_glyph_runs(&[RunJob {
            input: input(&run, &prefetched),
            target,
        }]);
        common::content::sprite_bounds(&common::content::lower(
            &mut cache,
            &input(&run, &lowered),
            &target,
        ))
    };
    let alone = {
        let raster = FakeRaster::shared();
        let mut cache = cache_with(&raster);
        common::content::sprite_bounds(&common::content::lower(
            &mut cache,
            &input(&run, &lowered),
            &target,
        ))
    };
    assert_eq!(after_prefetch, alone);
}

#[test]
fn a_cache_and_an_image_store_can_move_to_another_thread() {
    fn movable<T: Send>(_: &T) {}
    let raster = FakeRaster::shared();
    movable(&cache_with(&raster));
    movable(&iceblink_core::content::ImageStore::new());
}
