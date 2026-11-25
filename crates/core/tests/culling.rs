//! Which objects a frame draws, and the counts it reports.

mod common;

use iceblink::damage::DamageSet;
use iceblink_core::plan::LaneKind;

use crate::common::plan::scene::Scene;
use crate::common::plan::{damaged, invariants, rect};

#[test]
fn instances_off_the_output_or_outside_the_damage_are_culled() {
    let mut scene = Scene::sized([100, 100]);
    scene.quad(rect(10, 10, 10, 10));
    scene.quad(rect(200, 10, 10, 10));
    scene.quad(rect(60, 60, 10, 10));
    scene.external(rect(60, 60, 4, 4));
    let damage = damaged(0, 0, 50, 50);
    let planned = scene.plan(&damage);
    invariants::check(&scene, &damage, &planned);
    assert_eq!(planned.frame.lanes.quads.drawn(), 1);
    assert_eq!(planned.stats.drawn, 1);
    assert_eq!(planned.stats.culled, 3);
}

#[test]
fn backdrops_and_groups_are_drawn_whatever_the_damage() {
    let mut scene = Scene::sized([100, 100]);
    scene.begin_group(rect(60, 60, 20, 20));
    scene.backdrop(rect(60, 60, 20, 20), false);
    scene.end_group();
    let damage = damaged(0, 0, 10, 10);
    let planned = scene.plan(&damage);
    invariants::check(&scene, &damage, &planned);
    assert_eq!(planned.frame.batches.len(), 3);
    assert_eq!(planned.stats.groups, 1);
}

#[test]
fn the_counts_describe_the_plan() {
    let mut scene = Scene::sized([200, 200]);
    scene.quad(rect(0, 0, 100, 100));
    scene.keyed(LaneKind::Mono, rect(10, 10, 8, 8), [0, 0]);
    scene.keyed(LaneKind::Mono, rect(20, 10, 8, 8), [1, 0]);
    scene.plain_path(rect(40, 40, 20, 20));
    scene.mark(rect(120, 120, 20, 20), true);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let stats = planned.stats;
    assert_eq!(stats.drawn, 5);
    assert_eq!(stats.culled, 0);
    assert_eq!(stats.order_inserts, 5);
    assert_eq!(stats.batches, planned.frame.batches.len() as u32);
    assert_eq!(stats.vector_passes, 1);
    assert_eq!(stats.mark_bins, 1);
    assert_eq!(stats.groups, 0);
}
