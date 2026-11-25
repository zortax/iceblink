//! Blocks: instances ordered among themselves and placed with one query.

mod common;

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;
use iceblink_core::plan::{BoundsTree, LaneKind, local_orders};

use crate::common::plan::scene::Scene;
use crate::common::plan::{invariants, rect};

fn local(inks: &[DeviceRect]) -> Vec<u32> {
    let mut tree = BoundsTree::new();
    let mut out = Vec::new();
    local_orders(inks, &mut tree, &mut out);
    out
}

fn wide(x: i32, width: i32) -> DeviceRect {
    rect(x, 0, width, 10)
}

#[test]
fn disjoint_rectangles_share_the_lowest_local_order() {
    assert_eq!(
        local(&[wide(0, 8), wide(10, 8), wide(20, 8)]),
        vec![1, 1, 1]
    );
}

#[test]
fn a_rectangle_sorts_above_every_earlier_one_it_overlaps() {
    let inks = [wide(0, 10), wide(8, 10), wide(16, 10), wide(30, 4)];
    assert_eq!(local(&inks), vec![1, 2, 3, 1]);
}

#[test]
fn a_wide_rectangle_is_reached_past_its_narrow_neighbours() {
    let inks = [wide(0, 40), wide(10, 2), wide(20, 2), wide(30, 2)];
    assert_eq!(local(&inks), vec![1, 2, 2, 2]);
}

#[test]
fn a_sequence_that_steps_back_is_ordered_like_the_tree() {
    let inks = [wide(10, 10), wide(0, 15), wide(30, 5)];
    assert_eq!(local(&inks), vec![1, 2, 1]);
}

#[test]
fn a_block_sits_above_what_it_overlaps_and_its_disjoint_members_share_an_order() {
    let mut scene = Scene::sized([400, 400]);
    scene.quad(wide(0, 100));
    scene.block(LaneKind::Quads, &[wide(0, 8), wide(10, 8), wide(20, 8)]);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(
        planned.orders(LaneKind::Quads),
        vec![Some(1), Some(2), Some(2), Some(2)]
    );
}

#[test]
fn later_content_over_a_block_sorts_above_all_of_it() {
    let mut scene = Scene::sized([400, 400]);
    scene.block(LaneKind::Quads, &[wide(0, 10), wide(8, 10), wide(30, 4)]);
    scene.quad(wide(31, 1));
    scene.quad(wide(200, 10));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(
        planned.orders(LaneKind::Quads),
        vec![Some(1), Some(2), Some(1), Some(3), Some(1)],
        "the block's leaf carries its highest order, and content beside it keeps the low one"
    );
}

#[test]
fn a_culled_member_leaves_the_block_ordered() {
    let mut scene = Scene::sized([15, 400]);
    scene.block(LaneKind::Quads, &[wide(0, 10), wide(20, 10), wide(28, 10)]);
    scene.quad(wide(5, 5));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(
        planned.orders(LaneKind::Quads),
        vec![Some(1), None, None, Some(2)]
    );
    assert_eq!(
        planned.stats.culled, 2,
        "the members off the output are culled"
    );
}

#[test]
fn a_block_files_one_leaf() {
    let mut scene = Scene::sized([400, 400]);
    let inks: Vec<_> = (0..40).map(|at| wide(at * 10, 8)).collect();
    scene.block(LaneKind::Mono, &inks);
    assert_eq!(scene.plan(&DamageSet::full()).stats.order_inserts, 1);
}

#[test]
fn a_replayed_block_keeps_its_local_orders_above_what_lies_under_it() {
    let inks = [wide(0, 10), wide(8, 10), wide(30, 4)];
    let locals = local(&inks);
    assert_eq!(locals, vec![1, 2, 1]);

    let mut scene = Scene::sized([400, 400]);
    scene.quad(wide(0, 50));
    scene.block_with(LaneKind::Quads, &inks, &locals);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(
        planned.orders(LaneKind::Quads),
        vec![Some(1), Some(2), Some(3), Some(2)]
    );
}

#[test]
fn a_block_leaves_no_two_overlapping_instances_at_one_order() {
    let mut scene = Scene::sized([400, 400]);
    scene.quad(wide(0, 200));
    let inks: Vec<_> = (0..30).map(|at| wide(at * 6, 8)).collect();
    scene.block(LaneKind::Quads, &inks);
    scene.quad(wide(50, 4));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
}

#[test]
fn a_replayed_stack_keeps_its_own_ordering() {
    let inks = [rect(0, 0, 20, 20), rect(5, 5, 20, 20), rect(10, 10, 20, 20)];
    let mut scene = Scene::sized([400, 400]);
    scene.block(LaneKind::Quads, &inks);
    scene.quad(rect(0, 0, 3, 3));
    scene.quad(rect(300, 300, 10, 10));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(
        planned.orders(LaneKind::Quads),
        vec![Some(1), Some(2), Some(3), Some(4), Some(1)],
        "a quad over the bottom of the stack sorts above all of it, and one beside it stays low"
    );
}

#[test]
fn a_member_culled_when_its_orders_were_computed_is_ordered_when_it_is_drawn() {
    let recorded = [rect(0, 0, 20, 20), rect(0, 100, 20, 20)];
    let locals = local(&recorded);
    assert_eq!(locals, vec![1, 1]);

    let mut scene = Scene::sized([400, 40]);
    let moved = [rect(0, -90, 20, 20), rect(0, 10, 20, 20)];
    scene.block_with(LaneKind::Quads, &moved, &locals);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(planned.orders(LaneKind::Quads), vec![None, Some(1)]);
}

#[test]
fn a_block_of_sixty_four_overlapping_instances_costs_one_insert() {
    let inks: Vec<_> = (0..64).map(|at| rect(at, at, 20, 20)).collect();
    let mut scene = Scene::sized([400, 400]);
    scene.block(LaneKind::Quads, &inks);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(planned.stats.order_inserts, 1);
    assert_eq!(planned.frame.lanes.quads.drawn(), 64);
}
