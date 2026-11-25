//! The draw orders of the bounds tree, checked against their definition.

mod common;

use iceblink::DeviceRect;
use iceblink_core::plan::{BoundsTree, DrawOrder};
use proptest::prelude::*;

use crate::common::plan::rect;

/// Draw orders by their definition, computed against every rectangle inserted so far.
#[derive(Default)]
struct Oracle {
    inserted: Vec<(DeviceRect, DrawOrder)>,
    order_floor: DrawOrder,
}

impl Oracle {
    fn insert(&mut self, bounds: DeviceRect) -> DrawOrder {
        let highest = self
            .inserted
            .iter()
            .filter(|(other, _)| other.intersects(bounds))
            .map(|(_, order)| *order)
            .max()
            .unwrap_or(0);
        let order = (highest + 1).max(self.order_floor);
        self.inserted.push((bounds, order));
        order
    }

    fn insert_above_all(&mut self, bounds: DeviceRect) -> DrawOrder {
        let order = self
            .inserted
            .iter()
            .map(|(_, order)| *order)
            .max()
            .unwrap_or(0)
            + 1;
        self.inserted.push((bounds, order));
        order
    }

    fn set_order_floor(&mut self, floor: DrawOrder) {
        self.order_floor = self.order_floor.max(floor);
    }
}

/// A seeded xorshift generator, so a failing run reproduces from its seed.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, limit: i32) -> i32 {
        (self.next() % limit as u64) as i32
    }
}

#[test]
fn overlapping_rectangles_step_and_disjoint_ones_do_not() {
    let mut tree = BoundsTree::new();
    assert_eq!(tree.insert(rect(0, 0, 10, 10)), 1);
    assert_eq!(tree.insert(rect(5, 5, 10, 10)), 2);
    assert_eq!(tree.insert(rect(8, 8, 10, 10)), 3);
    assert_eq!(tree.insert(rect(100, 100, 10, 10)), 1);
    assert_eq!(tree.max_order(), 3);
    assert_eq!(tree.inserts(), 4);
}

#[test]
fn touching_edges_do_not_overlap() {
    let mut tree = BoundsTree::new();
    assert_eq!(tree.insert(rect(0, 0, 10, 10)), 1);
    assert_eq!(tree.insert(rect(10, 0, 10, 10)), 1);
}

#[test]
fn an_insert_above_all_sorts_above_everything_it_does_not_overlap() {
    let mut tree = BoundsTree::new();
    tree.insert(rect(0, 0, 10, 10));
    tree.insert(rect(5, 5, 10, 10));
    assert_eq!(tree.insert_above_all(rect(500, 500, 1, 1)), 3);
}

#[test]
fn an_order_floor_keeps_later_rectangles_above_a_closed_group() {
    let mut tree = BoundsTree::new();
    tree.insert(rect(0, 0, 10, 10));
    let end = tree.insert_above_all(rect(0, 0, 10, 10));
    tree.set_order_floor(end);
    assert_eq!(tree.insert(rect(900, 900, 10, 10)), end);
    assert_eq!(tree.order_floor(), end);
}

#[test]
fn a_block_reserves_its_span_and_files_one_leaf() {
    let mut tree = BoundsTree::new();
    tree.insert(rect(0, 0, 100, 10));
    let base = tree.insert_block(rect(0, 0, 50, 10), 2);
    assert_eq!(base, 2);
    assert_eq!(tree.len(), 2);
    assert_eq!(
        tree.insert(rect(40, 0, 4, 4)),
        5,
        "an overlap sorts above the whole block"
    );
    assert_eq!(
        tree.insert(rect(60, 0, 4, 4)),
        2,
        "beside the block the low order stays"
    );
}

#[test]
fn clearing_returns_the_tree_to_its_initial_state() {
    let mut tree = BoundsTree::new();
    tree.insert(rect(0, 0, 10, 10));
    tree.set_order_floor(9);
    tree.clear();
    assert!(tree.is_empty());
    assert_eq!(tree.max_order(), 0);
    assert_eq!(tree.order_floor(), 0);
    assert_eq!(tree.inserts(), 0);
    assert_eq!(tree.insert(rect(0, 0, 10, 10)), 1);
}

#[test]
fn ten_thousand_random_rectangles_match_the_definition() {
    let mut generator = Xorshift(0x5eed_1337_c0ff_ee01);
    let mut tree = BoundsTree::new();
    let mut oracle = Oracle::default();
    for index in 0..10_000 {
        let bounds = rect(
            generator.below(1920),
            generator.below(1080),
            1 + generator.below(240),
            1 + generator.below(160),
        );
        let expected = oracle.insert(bounds);
        assert_eq!(
            tree.insert(bounds),
            expected,
            "rectangle {index} at {bounds:?}"
        );
    }
    assert_eq!(tree.len(), 10_000);
}

proptest! {
    #[test]
    fn the_tree_matches_the_definition_under_both_barriers(
        operations in prop::collection::vec(
            (0u8..8, 0i32..400, 0i32..400, 1i32..120, 1i32..120),
            1..200,
        )
    ) {
        let mut tree = BoundsTree::new();
        let mut oracle = Oracle::default();
        for (operation, x, y, width, height) in operations {
            let bounds = rect(x, y, width, height);
            match operation {
                0 => prop_assert_eq!(tree.insert_above_all(bounds), oracle.insert_above_all(bounds)),
                1 => {
                    let floor = tree.max_order();
                    tree.set_order_floor(floor);
                    oracle.set_order_floor(floor);
                }
                _ => prop_assert_eq!(tree.insert(bounds), oracle.insert(bounds)),
            }
        }
    }

    #[test]
    fn two_rectangles_at_equal_order_never_overlap(
        rectangles in prop::collection::vec((0i32..300, 0i32..300, 1i32..90, 1i32..90), 1..120)
    ) {
        let mut tree = BoundsTree::new();
        let assigned: Vec<(DeviceRect, DrawOrder)> = rectangles
            .into_iter()
            .map(|(x, y, width, height)| {
                let bounds = rect(x, y, width, height);
                (bounds, tree.insert(bounds))
            })
            .collect();
        for (index, (left, left_order)) in assigned.iter().enumerate() {
            for (right, right_order) in &assigned[index + 1..] {
                if left_order == right_order {
                    prop_assert!(!left.intersects(*right));
                }
            }
        }
    }
}
