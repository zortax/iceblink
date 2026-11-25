//! Texture creation, destruction and reuse as the sink sees them.

mod common;

use common::atlas::{atlas, mono, square};
use iceblink::geom::Size;
use iceblink_alloc::atlas::{
    AtlasReport, MemorySink, TextureFormat, TextureId, TextureKind, TextureSink,
};

#[test]
fn a_texture_created_and_destroyed_between_flushes_never_reaches_the_sink() {
    let (mut atlas, mut sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(4), || vec![0; 16])
        .unwrap();
    assert_eq!(atlas.pending_textures(), 1);

    atlas.remove(mono(1));
    assert_eq!(atlas.pending_textures(), 0);
    atlas.flush_uploads(&mut sink).unwrap();
    assert_eq!(sink.textures_created(), 0);
}

#[test]
fn a_texture_slot_destroyed_and_created_again_replays_in_order() {
    let (mut atlas, mut sink) = atlas();
    let first = atlas
        .get_or_insert(mono(1), square(4), || vec![0; 16])
        .unwrap();
    atlas.flush_uploads(&mut sink).unwrap();

    atlas.remove(mono(1));
    let second = atlas
        .get_or_insert(mono(2), square(2048), || vec![0; 2048 * 2048])
        .unwrap();
    assert_eq!(second.texture, first.texture);
    assert_eq!(atlas.pending_textures(), 2);

    atlas.flush_uploads(&mut sink).unwrap();
    assert_eq!(sink.live_textures(), 1);
    assert_eq!(sink.size_of(second.texture), Some(Size::new(2048, 2048)));
}

#[test]
fn clearing_destroys_every_texture() {
    let (mut atlas, mut sink) = atlas();
    for handle in 0..8_u64 {
        atlas
            .get_or_insert(mono(handle), square(32), || vec![0; 1024])
            .unwrap();
    }
    atlas.flush_uploads(&mut sink).unwrap();
    assert!(sink.live_textures() > 0);

    atlas.clear();
    atlas.flush_uploads(&mut sink).unwrap();
    assert!(atlas.is_empty());
    assert_eq!(sink.live_textures(), 0);
    assert_eq!(atlas.report(), AtlasReport::default());
}

// A hundred thousand atlas cycles take hours under miri.
#[cfg_attr(miri, ignore)]
#[test]
fn a_hundred_thousand_insert_remove_cycles_reuse_one_texture() {
    let (mut atlas, mut sink) = atlas();
    // The held anchor keeps the texture alive across every cycle.
    let anchor = mono(u64::MAX);
    atlas
        .get_or_insert(anchor, square(16), || vec![0; 256])
        .unwrap();
    atlas.retain(anchor);

    for cycle in 0..100_000_u64 {
        let key = mono(cycle);
        atlas
            .get_or_insert(key, square(16), || vec![0; 256])
            .unwrap_or_else(|error| panic!("cycle {cycle} could not allocate: {error}"));
        assert!(atlas.remove(key));
    }

    assert_eq!(atlas.len(), 1);
    assert_eq!(atlas.report().textures, 1);
    atlas.flush_uploads(&mut sink).unwrap();
    assert_eq!(sink.textures_created(), 1);

    atlas.release(anchor);
    assert!(atlas.remove(anchor));
    assert!(atlas.is_fully_reclaimed());
    atlas.flush_uploads(&mut sink).unwrap();
    assert_eq!(sink.live_textures(), 0);
}

#[test]
fn the_memory_sink_refuses_a_texture_without_extent() {
    let mut sink = MemorySink::new();
    let texture = TextureId::new(TextureKind::Mono, 0);
    assert!(
        sink.create_texture(texture, Size::new(0, 4), TextureFormat::R8Unorm)
            .is_err()
    );
}
