//! Packing vector passes into scratch layers and rounds.

use iceblink::DeviceRect;
use iceblink::geom::{Point, Rect, Size};
use iceblink_wgpu::vector::{Capacity, Layering, VectorTarget};

fn region(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

/// Asserts that no two passes in one layer of one round share a texel.
fn assert_disjoint(layering: &Layering, count: usize) {
    for one in 0..count {
        for two in one + 1..count {
            let first = layering.placement(one).unwrap();
            let second = layering.placement(two).unwrap();
            if first.round == second.round && first.target == second.target {
                assert!(
                    first.bin.intersection(second.bin).is_none(),
                    "{one} and {two}"
                );
            }
        }
    }
}

#[test]
fn passes_stacked_over_one_point_share_a_layer_in_bins_of_their_own() {
    let regions: Vec<_> = (0..500).map(|_| region(0, 0, 64, 64)).collect();

    let layering = Layering::of(&regions, Capacity::DEFAULT);

    assert_eq!(layering.layers(), 1);
    assert_eq!(layering.rounds(), 1);
    assert_disjoint(&layering, regions.len());
}

#[test]
fn a_bin_has_the_size_of_its_region() {
    let regions = [region(16, 32, 48, 80), region(0, 0, 16, 16)];

    let layering = Layering::of(&regions, Capacity::DEFAULT);

    for (index, region) in regions.iter().enumerate() {
        assert_eq!(layering.placement(index).unwrap().bin.size, region.size);
    }
}

#[test]
fn far_apart_passes_are_packed_without_the_space_between_them() {
    let regions = [
        region(10_000, 20_000, 16, 16),
        region(30_000, 40_000, 16, 16),
    ];

    let (width, height) = Layering::of(&regions, Capacity::DEFAULT).extent();

    assert!(width <= 32 && height <= 32);
}

#[test]
fn a_full_layer_opens_the_next_one() {
    let capacity = Capacity {
        side: 64,
        layers: 4,
    };
    let regions: Vec<_> = (0..8).map(|_| region(0, 0, 64, 32)).collect();

    let layering = Layering::of(&regions, capacity);

    assert_eq!(layering.layers(), 4);
    assert_eq!(layering.rounds(), 1);
    assert_eq!(layering.target(2), VectorTarget(1));
    assert_disjoint(&layering, regions.len());
}

#[test]
fn a_full_round_starts_the_next_one_in_pass_order() {
    let capacity = Capacity {
        side: 32,
        layers: 2,
    };
    let regions: Vec<_> = (0..7).map(|_| region(0, 0, 32, 32)).collect();

    let layering = Layering::of(&regions, capacity);

    let rounds: Vec<_> = (0..regions.len())
        .map(|index| layering.placement(index).unwrap().round)
        .collect();
    assert_eq!(rounds, [0, 0, 1, 1, 2, 2, 3]);
    assert_eq!(layering.layers(), 2);
    assert_disjoint(&layering, regions.len());
}

#[test]
fn a_pass_larger_than_a_layer_widens_the_layer() {
    let capacity = Capacity {
        side: 64,
        layers: 1,
    };
    let regions = [region(0, 0, 640, 480), region(0, 0, 16, 16)];

    let layering = Layering::of(&regions, capacity);

    let (width, height) = layering.extent();
    assert!(width >= 640 && height >= 480);
    assert_disjoint(&layering, regions.len());
}

#[test]
fn an_empty_region_takes_no_space_and_no_passes_take_no_rounds() {
    let layering = Layering::of(
        &[region(0, 0, 0, 0), region(0, 0, 16, 16)],
        Capacity::DEFAULT,
    );
    assert_eq!(layering.extent(), (16, 16));

    let empty = Layering::of(&[], Capacity::DEFAULT);
    assert_eq!(empty.rounds(), 0);
    assert_eq!(empty.layers(), 0);
    assert_eq!(empty.target(0), VectorTarget::NONE);
}
