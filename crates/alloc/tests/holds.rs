//! Hold counts, conditional removal and the held totals of the report.

mod common;

use common::atlas::{atlas, mono, square};
use iceblink_alloc::atlas::AtlasReport;

#[test]
fn a_hold_count_never_underflows() {
    let (mut atlas, _sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(4), || vec![0; 16])
        .unwrap();

    for _ in 0..10 {
        assert!(atlas.release(mono(1)));
    }
    assert_eq!(atlas.refs(mono(1)), Some(0));

    atlas.retain(mono(1));
    assert_eq!(atlas.refs(mono(1)), Some(1));
    atlas.release(mono(1));
    atlas.release(mono(1));
    assert_eq!(atlas.refs(mono(1)), Some(0));

    assert!(!atlas.retain(mono(2)));
    assert!(!atlas.release(mono(2)));
    assert_eq!(atlas.refs(mono(2)), None);
}

#[test]
fn a_conditional_removal_respects_holds() {
    let (mut atlas, _sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(4), || vec![0; 16])
        .unwrap();
    atlas.retain(mono(1));

    assert!(!atlas.remove_if_unreferenced(mono(1)));
    assert!(atlas.contains(mono(1)));

    atlas.release(mono(1));
    assert!(atlas.remove_if_unreferenced(mono(1)));
    assert!(atlas.is_fully_reclaimed());
}

#[test]
fn the_report_counts_the_tile_bytes_of_held_entries() {
    let (mut atlas, _sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(8), || vec![1; 64])
        .unwrap();
    atlas
        .get_or_insert(mono(2), square(4), || vec![2; 16])
        .unwrap();
    assert_eq!(atlas.report().referenced_bytes, 0);

    atlas.retain(mono(1));
    let held = atlas.report();
    assert_eq!(held.referenced_tiles, 1);
    assert_eq!(held.referenced_bytes, 64);

    atlas.retain(mono(2));
    assert_eq!(atlas.report().referenced_bytes, 80);

    atlas.release(mono(1));
    assert_eq!(atlas.report().referenced_bytes, 16);
}

#[test]
fn a_second_hold_on_one_entry_counts_once() {
    let (mut atlas, _sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(8), || vec![1; 64])
        .unwrap();
    atlas.retain(mono(1));
    atlas.retain(mono(1));
    assert_eq!(atlas.report().referenced_tiles, 1);
    assert_eq!(atlas.report().referenced_bytes, 64);

    atlas.release(mono(1));
    assert_eq!(atlas.report().referenced_bytes, 64);
    atlas.release(mono(1));
    assert_eq!(atlas.report().referenced_bytes, 0);
}

#[test]
fn releasing_an_unheld_entry_leaves_other_holds_counted() {
    let (mut atlas, _sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(8), || vec![1; 64])
        .unwrap();
    atlas
        .get_or_insert(mono(2), square(4), || vec![2; 16])
        .unwrap();
    atlas.retain(mono(1));

    atlas.release(mono(2));
    assert_eq!(atlas.report().referenced_tiles, 1);
    assert_eq!(atlas.report().referenced_bytes, 64);
}

#[test]
fn removed_and_cleared_entries_stop_counting_as_held() {
    let (mut atlas, _sink) = atlas();
    atlas
        .get_or_insert(mono(1), square(8), || vec![1; 64])
        .unwrap();
    atlas.retain(mono(1));
    assert_eq!(atlas.report().referenced_bytes, 64);

    assert!(atlas.remove(mono(1)));
    assert_eq!(atlas.report().referenced_tiles, 0);
    assert_eq!(atlas.report().referenced_bytes, 0);

    atlas
        .get_or_insert(mono(2), square(8), || vec![1; 64])
        .unwrap();
    atlas.retain(mono(2));
    atlas.clear();
    assert_eq!(atlas.report(), AtlasReport::default());
}
