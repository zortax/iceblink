//! The vector-pass coalescing rules, measured on the dashboard, avatar and other plan fixtures.

mod common;

use iceblink::damage::DamageSet;
use iceblink::{BatchKind, FRAME_NO_CLIP, VectorPlan};
use iceblink_core::plan::pass::uncut;
use iceblink_core::plan::{Overlap, PassWarning};

use crate::common::plan::fixture::{
    across_a_group, avatars, badged_cards, dashboard, falling_order, label_field, stacked_area,
    wide,
};
use crate::common::plan::scene::Scene;
use crate::common::plan::{damaged, invariants, rect};

/// Returns a damage set covering the given cards of the dashboard grid.
fn cards(indices: &[i32]) -> DamageSet {
    let mut damage = DamageSet::new();
    for index in indices {
        damage.absorb(rect(
            16 + (index % 5) * 380,
            16 + (index / 5) * 268,
            360,
            248,
        ));
    }
    damage
}

/// Returns how many passes `scene` plans against `damage` under `overlap`.
fn passes(scene: &mut Scene, damage: &DamageSet, overlap: Overlap) -> usize {
    let planned = scene.plan_with(damage, overlap);
    invariants::check(scene, damage, &planned);
    planned.passes()
}

#[test]
fn the_three_readings_of_rule_three_cost_twenty_four_and_one_pass() {
    let full = DamageSet::full();
    let mut scene = dashboard(false);
    assert_eq!(
        passes(&mut scene, &full, Overlap::BoundingBoxOrderBlind),
        20
    );
    assert_eq!(passes(&mut scene, &full, Overlap::BoundingBox), 4);
    assert_eq!(passes(&mut scene, &full, Overlap::PerItemInk), 1);
    assert_eq!(Overlap::default(), Overlap::PerItemInk);
}

#[test]
fn a_legend_drawn_over_its_chart_costs_a_pass_per_region_under_every_reading() {
    let full = DamageSet::full();
    let mut scene = dashboard(true);
    for reading in [
        Overlap::BoundingBoxOrderBlind,
        Overlap::BoundingBox,
        Overlap::PerItemInk,
    ] {
        assert_eq!(passes(&mut scene, &full, reading), 20, "{reading:?}");
    }
    let never = scene.plan_with(&full, Overlap::Never);
    assert_eq!(never.passes(), 1, "no two charts overlap each other");
    assert!(never.frame.vector_plan.passes[0].instanced);
}

#[test]
fn the_pass_count_follows_the_damaged_regions() {
    let mut scene = dashboard(true);
    let one = scene.plan(&cards(&[0]));
    assert_eq!(one.passes(), 1);
    assert_eq!(
        one.stats.paths_culled, 19,
        "paths the damage misses are dropped"
    );

    let three = scene.plan(&cards(&[0, 7, 13]));
    assert_eq!(three.passes(), 3);
    assert_eq!(three.stats.paths_culled, 17);
    invariants::check(&scene, &cards(&[0, 7, 13]), &three);
}

#[test]
fn a_legend_drawn_under_its_chart_never_forces_a_pass() {
    let planned = dashboard(false).plan(&DamageSet::full());
    assert_eq!(planned.passes(), 1);
    assert_eq!(planned.stats.clip_layers, 20, "one card clip per chart");
}

#[test]
fn an_avatar_row_costs_one_pass_and_absorbs_its_clips() {
    let mut scene = avatars();
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let plan = &planned.frame.vector_plan;
    assert_eq!(plan.passes.len(), 1);
    assert_eq!(plan.items.len(), 12);
    assert_eq!(
        planned.stats.clip_layers, 12,
        "each rounded clip becomes a layer"
    );
    let pass = &plan.passes[0];
    assert_ne!(pass.clip, FRAME_NO_CLIP, "the row's clip is shared");
    assert!(
        plan.items_of(pass)
            .iter()
            .all(|item| item.residual != FRAME_NO_CLIP)
    );
}

#[test]
fn a_pass_whose_paths_overlap_is_not_instanced() {
    let planned = stacked_area().plan(&DamageSet::full());
    let plan = &planned.frame.vector_plan;
    assert_eq!(
        plan.passes.len(),
        1,
        "nothing intervenes, so the bands share a pass"
    );
    assert_eq!(plan.items.len(), 5);
    assert!(!plan.passes[0].instanced);

    assert!(avatars().plan(&DamageSet::full()).frame.vector_plan.passes[0].instanced);
}

#[test]
fn a_frame_with_no_paths_plans_no_passes() {
    let mut scene = Scene::new();
    for index in 0..61 {
        scene.quad(rect(index * 4, 0, 3, 3));
    }
    assert!(scene.plan(&DamageSet::full()).frame.vector_plan.is_empty());
}

#[test]
fn an_undamaged_frame_plans_nothing_and_counts_what_it_dropped() {
    let planned = dashboard(true).plan(&DamageSet::new());
    assert!(planned.frame.vector_plan.is_empty());
    assert_eq!(planned.stats.paths_culled, 20);
}

#[test]
fn a_path_with_a_mask_clip_gets_a_pass_of_its_own() {
    let mut scene = Scene::new();
    let masked = scene.clips.mask(FRAME_NO_CLIP);
    scene.plain_path(rect(0, 0, 20, 20));
    scene.path(rect(40, 0, 20, 20), masked);
    scene.plain_path(rect(80, 0, 20, 20));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let plan = &planned.frame.vector_plan;
    assert_eq!(
        plan.passes.len(),
        3,
        "the masked path neither joins nor is joined"
    );
    let pass = plan
        .passes
        .iter()
        .find(|pass| pass.clip == masked)
        .expect("the masked path's pass binds its clip");
    assert_eq!(plan.items_of(pass).len(), 1);
    assert_eq!(plan.items_of(pass)[0].residual, FRAME_NO_CLIP);
}

#[test]
fn a_pass_region_is_tile_aligned_and_its_path_inks_are_relative_to_it() {
    let planned = avatars().plan(&DamageSet::full());
    let plan = &planned.frame.vector_plan;
    let pass = &plan.passes[0];
    assert_eq!(pass.region.left() % 16, 0);
    assert_eq!(pass.region.top() % 16, 0);
    for item in plan.items_of(pass) {
        assert!(item.ink.left() >= 0 && item.ink.top() >= 0);
        assert!(item.ink.right() <= pass.region.size.width);
    }
}

#[test]
fn the_warning_fires_on_a_damaged_frame_and_stays_quiet_on_a_full_repaint() {
    let planned = dashboard(true).plan(&DamageSet::full());
    let plan = &planned.frame.vector_plan;
    assert!(PassWarning::of(plan, true).is_none());
    let warning = PassWarning::of(plan, false).expect("twenty passes on a damaged frame");
    assert_eq!(warning.passes, 20);
    assert!(warning.to_string().contains("20"));

    let quiet = dashboard(false).plan(&DamageSet::full());
    assert!(PassWarning::of(&quiet.frame.vector_plan, false).is_none());
}

#[test]
fn a_pass_ends_at_a_group_boundary_however_little_is_painted_across_it() {
    let mut scene = across_a_group();
    assert_eq!(
        passes(&mut scene, &DamageSet::full(), Overlap::PerItemInk),
        3
    );
    let planned = scene.plan(&DamageSet::full());
    assert!(
        planned
            .frame
            .vector_plan
            .passes
            .iter()
            .all(|pass| pass.items.len() == 1)
    );
}

#[test]
fn a_composite_is_drawn_after_the_backgrounds_of_every_path_in_its_pass() {
    let mut scene = falling_order();
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(
        planned.passes(),
        1,
        "nothing painted between the drawings meets either"
    );

    let orders = planned.orders(iceblink_core::plan::LaneKind::Quads);
    assert!(
        orders[4] < orders[3],
        "the right card's background sorts below the left card's innermost one: {orders:?}"
    );
    let composite = planned
        .frame
        .batches
        .iter()
        .position(|batch| matches!(batch.kind, BatchKind::Paths { .. }))
        .expect("the pass is composited");
    let last_quads = planned
        .frame
        .batches
        .iter()
        .rposition(|batch| batch.kind == BatchKind::Quads)
        .expect("the backgrounds are drawn");
    assert!(
        composite > last_quads,
        "the composite is drawn above every background"
    );
}

#[test]
fn a_pass_that_would_trap_an_instance_under_its_composite_is_split_per_path() {
    assert_eq!(
        passes(
            &mut falling_order(),
            &DamageSet::full(),
            Overlap::PerItemInk
        ),
        1
    );
    assert_eq!(
        passes(&mut badged_cards(), &DamageSet::full(), Overlap::PerItemInk),
        2
    );
}

#[test]
fn a_long_pass_past_many_backgrounds_plans_quickly() {
    let mut scene = label_field(8000);
    let started = std::time::Instant::now();
    let planned = scene.plan(&DamageSet::full());
    let elapsed = started.elapsed();
    assert_eq!(planned.passes(), 1);
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "planning took {elapsed:?}"
    );
}

#[test]
fn a_mark_over_a_path_splits_its_pass() {
    let mut scene = Scene::new();
    scene.plain_path(rect(0, 0, 40, 40));
    scene.mark(rect(10, 10, 20, 20), false);
    scene.plain_path(rect(10, 0, 40, 40));
    assert_eq!(
        passes(&mut scene, &DamageSet::full(), Overlap::default()),
        2
    );
}

#[test]
fn a_pass_region_is_cut_to_the_damage() {
    let damage = damaged(100, 50, 100, 30);
    let planned = wide(false).plan(&damage);
    let pass = &planned.frame.vector_plan.passes[0];
    assert_eq!(pass.region, rect(96, 48, 112, 32));
    assert!(pass.clamped);
}

#[test]
fn a_cut_pass_cuts_its_path_inks() {
    let planned = wide(true).plan(&damaged(100, 50, 100, 30));
    let plan = &planned.frame.vector_plan;
    assert_eq!(plan.passes.len(), 1);
    let inks: Vec<_> = plan
        .items_of(&plan.passes[0])
        .iter()
        .map(|item| item.ink)
        .collect();
    assert_eq!(inks, vec![rect(0, 0, 112, 32), rect(54, 12, 58, 10)]);
}

#[test]
fn a_pass_inside_a_group_keeps_its_whole_region() {
    let planned = across_a_group().plan(&damaged(220, 40, 8, 8));
    let plan = &planned.frame.vector_plan;
    assert_eq!(
        plan.passes.len(),
        1,
        "only the drawing in the group is damaged"
    );
    assert_eq!(plan.passes[0].region, rect(208, 32, 48, 32));
    assert!(!plan.passes[0].clamped);
}

#[test]
fn a_frame_with_a_backdrop_keeps_whole_regions() {
    let mut scene = wide(false);
    scene.backdrop(rect(1200, 600, 100, 100), true);
    let planned = scene.plan(&damaged(100, 50, 100, 30));
    let pass = &planned.frame.vector_plan.passes[0];
    assert_eq!(pass.region, rect(0, 0, 800, 400));
    assert!(!pass.clamped);
}

#[test]
fn a_pass_past_the_edge_of_the_output_overhangs_and_is_not_cut() {
    let mut scene = Scene::new();
    scene.plain_path(rect(1880, 8, 80, 16));
    scene.plain_path(rect(16, 64, 32, 16));
    let planned = scene.plan(&DamageSet::full());
    let plan = &planned.frame.vector_plan;
    assert_eq!(plan.passes.len(), 1);
    assert!(plan.passes[0].overhangs);
    assert!(!plan.passes[0].clamped);

    let mut inside = Scene::new();
    inside.plain_path(rect(16, 64, 32, 16));
    assert!(!inside.plan(&DamageSet::full()).frame.vector_plan.passes[0].overhangs);
}

#[test]
fn a_path_whose_outline_reaches_past_its_clipped_ink_overhangs() {
    let mut scene = Scene::new();
    scene.reaching_path(rect(16, 64, 32, 16), rect(16, 64, 4000, 16), FRAME_NO_CLIP);
    let planned = scene.plan(&DamageSet::full());
    let pass = &planned.frame.vector_plan.passes[0];
    assert_eq!(pass.region, rect(16, 64, 32, 16));
    assert!(pass.overhangs);
}

#[test]
fn full_damage_cuts_nothing() {
    let planned = wide(true).plan(&DamageSet::full());
    let plan = &planned.frame.vector_plan;
    let pass = &plan.passes[0];
    assert_eq!(pass.region, rect(0, 0, 800, 400));
    assert!(!pass.clamped);
    assert_eq!(plan.items_of(pass)[1].ink, rect(150, 60, 300, 10));
}

#[test]
fn an_uncut_plan_is_the_plan_full_damage_makes() {
    let mut scene = wide(true);
    let cut = scene.plan(&damaged(100, 50, 100, 30));
    assert!(cut.frame.vector_plan.passes.iter().any(|pass| pass.clamped));
    let mut whole = VectorPlan::default();
    uncut(
        &cut.frame.vector_plan,
        scene.emission.path_inks(),
        scene.size,
        &mut whole,
    );
    assert_eq!(whole, scene.plan(&DamageSet::full()).frame.vector_plan);
}

#[test]
fn an_instance_over_a_path_of_a_closed_pass_sorts_above_its_composite() {
    let mut scene = Scene::new();
    scene.quad(rect(100, 0, 50, 50));
    scene.plain_path(rect(0, 0, 20, 20));
    scene.plain_path(rect(100, 0, 20, 20));
    scene.quad(rect(110, 10, 5, 5));
    scene.plain_path(rect(300, 0, 10, 10));
    scene.quad(rect(5, 5, 5, 5));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    assert_eq!(planned.passes(), 2);
    assert_eq!(
        planned.orders(iceblink_core::plan::LaneKind::Quads)[2],
        Some(3),
        "the quad over the first path sorts above the composite at order two"
    );
}
