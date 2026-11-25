//! The mark plan: bins for union marks.

mod common;

use std::collections::HashMap;

use iceblink::MAX_MARK_PAGES;
use iceblink::damage::DamageSet;

use crate::common::plan::scene::{Planned, Scene};
use crate::common::plan::{damaged, invariants, rect};

/// Returns each bin's page and position, by the transient index of its mark.
fn placed(planned: &Planned) -> HashMap<u32, (u32, [u32; 2])> {
    let plan = &planned.frame.mark_plan;
    let sources: Vec<_> = planned.frame.lanes.marks.instances().collect();
    plan.bins
        .iter()
        .chain(&plan.overflow)
        .map(|bin| (sources[bin.item as usize].index, (bin.page, bin.at)))
        .collect()
}

#[test]
fn marks_without_union_take_no_bin() {
    let mut scene = Scene::sized([200, 200]);
    scene.mark(rect(10, 10, 40, 40), false);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert!(planned.frame.mark_plan.is_empty());
    assert_eq!(planned.frame.mark_plan.pages, 0);
}

#[test]
fn a_bin_holds_only_the_damaged_part_of_its_mark() {
    let mut scene = Scene::sized([200, 200]);
    scene.mark(rect(10, 10, 40, 40), true);
    scene.mark(rect(120, 120, 40, 40), true);
    let damage = damaged(0, 0, 30, 100);
    let planned = scene.plan(&damage);
    invariants::check(&scene, &damage, &planned);
    let plan = &planned.frame.mark_plan;
    let [bin] = plan.bins.as_slice() else {
        panic!("{:?}", plan.bins);
    };
    assert_eq!(bin.region, rect(10, 10, 20, 40));
    assert_eq!(
        planned.frame.lanes.marks.drawn(),
        1,
        "the other mark is culled"
    );
    assert!(!plan.full_damage);
}

#[test]
fn bins_fill_rows_and_move_to_a_new_page() {
    let mut scene = Scene::sized([100, 100]);
    for at in 0..3 {
        scene.mark(rect(at * 10, 0, 60, 60), true);
    }
    scene.mark(rect(0, 70, 30, 30), true);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let placed = placed(&planned);
    assert_eq!(placed[&0], (0, [0, 0]));
    assert_eq!(placed[&1], (1, [0, 0]));
    assert_eq!(placed[&2], (2, [0, 0]));
    assert_eq!(placed[&3], (2, [60, 0]));
    assert_eq!(planned.frame.mark_plan.pages, 3);
    assert_eq!(
        planned.frame.mark_plan.extent,
        [90, 60],
        "a page reaches as far as its bins"
    );
}

#[test]
fn marks_past_the_last_page_take_turns_on_the_spare_page() {
    let mut scene = Scene::sized([100, 100]);
    for at in 0..MAX_MARK_PAGES as i32 + 2 {
        scene.mark(rect(at, 0, 90, 90), true);
    }
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let plan = &planned.frame.mark_plan;
    assert_eq!(plan.bins.len(), MAX_MARK_PAGES as usize);
    assert_eq!(plan.overflow.len(), 2);
    assert_eq!(plan.pages, MAX_MARK_PAGES + 1);
    assert_eq!(planned.stats.mark_bins, MAX_MARK_PAGES + 2);
    let placed = placed(&planned);
    assert_eq!(placed[&8], (MAX_MARK_PAGES, [0, 0]));
    assert_eq!(placed[&9], (MAX_MARK_PAGES, [0, 0]));
}

#[test]
fn a_bin_never_exceeds_the_output() {
    let mut scene = Scene::sized([100, 80]);
    scene.mark(rect(-50, -50, 400, 400), true);
    let planned = scene.plan(&DamageSet::full());
    let [bin] = planned.frame.mark_plan.bins.as_slice() else {
        panic!("{:?}", planned.frame.mark_plan.bins);
    };
    assert_eq!(bin.region, rect(0, 0, 100, 80));
    assert_eq!(bin.at, [0, 0]);
}

#[test]
fn a_bin_inside_a_group_keeps_its_whole_ink() {
    let mut scene = Scene::sized([200, 200]);
    scene.begin_group(rect(0, 0, 200, 200));
    scene.mark(rect(10, 10, 40, 40), true);
    scene.end_group();
    let damage = damaged(0, 0, 20, 20);
    let planned = scene.plan(&damage);
    invariants::check(&scene, &damage, &planned);
    let [bin] = planned.frame.mark_plan.bins.as_slice() else {
        panic!("{:?}", planned.frame.mark_plan.bins);
    };
    assert_eq!(bin.region, rect(10, 10, 40, 40));
}

#[test]
fn a_bin_is_found_by_its_position_in_the_marks_lane() {
    let mut scene = Scene::sized([200, 200]);
    scene.mark(rect(0, 0, 50, 50), true);
    scene.mark(rect(10, 10, 50, 50), false);
    scene.mark(rect(100, 100, 20, 20), true);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let sources: Vec<_> = planned.frame.lanes.marks.instances().collect();
    let third = sources
        .iter()
        .position(|source| source.index == 2)
        .expect("the third mark is drawn");
    assert_eq!(
        third, 1,
        "the disjoint mark sorts before the overlapping one"
    );
    assert_eq!(
        planned
            .frame
            .mark_plan
            .bin(third as u32)
            .map(|bin| bin.region),
        Some(rect(100, 100, 20, 20))
    );
}

#[test]
fn a_replan_against_full_damage_holds_whole_inks() {
    let mut scene = Scene::sized([200, 200]);
    scene.mark(rect(10, 10, 40, 40), true);
    let damage = damaged(0, 0, 20, 20);
    let mut planned = scene.plan(&damage);
    let mut whole = Default::default();
    planned.planner.replan_marks(&DamageSet::full(), &mut whole);
    assert_eq!(planned.frame.mark_plan.bins[0].region, rect(10, 10, 10, 10));
    let iceblink::MarkPlan {
        bins, full_damage, ..
    } = whole;
    assert!(full_damage);
    assert_eq!(bins[0].region, rect(10, 10, 40, 40));
}
