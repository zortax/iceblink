//! Atlas invariants under random sequences of inserts, holds, releases and removals.

mod common;

use common::atlas::{mono, no_tile_overlaps, square};
use iceblink_alloc::atlas::{Atlas, AtlasLimits, MemorySink};
use proptest::prelude::*;

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn tiles_never_overlap_and_all_space_returns(
        operations in prop::collection::vec(
            (0u64..16, 0u8..4, 4i32..24),
            0..400,
        )
    ) {
        let mut atlas = Atlas::new(AtlasLimits {
            texture_size: 128,
            max_texture_size: 256,
            max_textures_per_pool: 8,
            soft_bytes: None,
        });
        let mut sink = MemorySink::new();

        for (handle, operation, side) in operations {
            let key = mono(handle);
            match operation {
                0 => {
                    let bytes = (side * side) as usize;
                    let _ = atlas.get_or_insert(key, square(side), || vec![0; bytes]);
                }
                1 => { atlas.retain(key); }
                2 => { atlas.release(key); }
                _ => { atlas.remove(key); }
            }
            prop_assert!(no_tile_overlaps(&atlas));
        }

        // A key may take any number of holds.
        for handle in 0..16u64 {
            while atlas.refs(mono(handle)).is_some_and(|held| held > 0) {
                atlas.release(mono(handle));
            }
        }
        prop_assert_eq!(atlas.report().referenced_tiles, 0);
        prop_assert_eq!(atlas.report().referenced_bytes, 0);

        atlas.begin_frame();
        atlas.evict_all_unused();
        prop_assert!(atlas.is_empty());
        prop_assert!(atlas.is_fully_reclaimed());
        atlas.flush_uploads(&mut sink).unwrap();
        prop_assert_eq!(sink.live_textures(), 0);
    }
}
