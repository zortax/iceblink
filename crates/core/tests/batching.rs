//! Batching: the fewest draws that keep paint order.

mod common;

use iceblink::damage::DamageSet;
use iceblink::{BatchKind, DeviceRect};
use iceblink_core::plan::LaneKind;

use crate::common::plan::scene::Scene;
use crate::common::plan::{invariants, rect};

/// Returns an 8 by 8 rectangle at `x` on the top row.
fn at(x: i32) -> DeviceRect {
    rect(x, 0, 8, 8)
}

/// Plans `scene` whole, checks it, and returns its batch kinds.
fn kinds(scene: &mut Scene) -> Vec<BatchKind> {
    let planned = scene.plan(&DamageSet::full());
    invariants::check(scene, &DamageSet::full(), &planned);
    planned
        .frame
        .batches
        .iter()
        .map(|batch| batch.kind)
        .collect()
}

/// Returns a scene of shader quads, each at `x` with its shader and parameter slot.
fn shaded(pushed: &[(i32, u32, u32)]) -> Scene {
    let mut scene = Scene::sized([256, 64]);
    for (x, shader, params) in pushed {
        scene.keyed(LaneKind::ShaderQuads, at(*x), [*shader, *params]);
    }
    scene
}

#[test]
fn shader_quads_of_one_shader_and_parameter_slot_are_one_draw() {
    let mut scene = shaded(&[(0, 1, 3), (16, 1, 3), (32, 1, 3)]);
    assert_eq!(
        kinds(&mut scene),
        vec![BatchKind::ShaderQuads {
            shader: 1,
            params: 3
        }]
    );
}

#[test]
fn a_change_of_shader_or_parameters_breaks_the_run() {
    assert_eq!(kinds(&mut shaded(&[(0, 1, 3), (16, 2, 3)])).len(), 2);
    assert_eq!(kinds(&mut shaded(&[(0, 1, 3), (16, 1, 4)])).len(), 2);
}

#[test]
fn instances_pushed_interleaved_are_clustered_by_binding() {
    assert_eq!(
        kinds(&mut shaded(&[
            (0, 1, 3),
            (16, 2, 3),
            (32, 1, 3),
            (48, 2, 3)
        ]))
        .len(),
        2
    );

    let mut sprites = Scene::sized([256, 64]);
    for (x, page) in [(0, 4), (16, 7), (32, 4), (48, 7)] {
        sprites.keyed(LaneKind::Mono, at(x), [page, x as u32]);
    }
    assert_eq!(
        kinds(&mut sprites),
        vec![BatchKind::Mono { page: 4 }, BatchKind::Mono { page: 7 }]
    );
}

#[test]
fn a_shader_quad_never_shares_a_draw_with_a_quad() {
    let mut scene = Scene::sized([256, 64]);
    scene.quad(at(0));
    scene.keyed(LaneKind::ShaderQuads, at(16), [1, 0]);
    assert_eq!(kinds(&mut scene).len(), 2);
}

#[test]
fn disjoint_content_of_two_kinds_batches_by_kind() {
    let mut scene = Scene::sized([256, 64]);
    for x in [0, 32, 64] {
        scene.quad(rect(x, 0, 24, 24));
        scene.keyed(LaneKind::Mono, rect(x + 4, 4, 8, 8), [0, 0]);
    }
    assert_eq!(
        kinds(&mut scene),
        vec![BatchKind::Quads, BatchKind::Mono { page: 0 }],
        "every glyph sits on its own background, so one draw per kind keeps the order"
    );
}

#[test]
fn a_batch_ink_is_the_union_of_its_instances_cut_to_the_output() {
    let mut scene = Scene::sized([100, 100]);
    scene.quad(rect(10, 10, 10, 10));
    scene.quad(rect(90, 50, 40, 10));
    let planned = scene.plan(&DamageSet::full());
    assert_eq!(planned.frame.batches[0].ink, rect(10, 10, 90, 50));
}

#[test]
fn a_group_encloses_its_content_and_later_content_stays_outside() {
    let mut scene = Scene::sized([256, 64]);
    scene.quad(at(0));
    scene.begin_group(rect(0, 0, 64, 64));
    scene.quad(at(16));
    scene.end_group();
    scene.quad(at(200));
    assert_eq!(
        kinds(&mut scene),
        vec![
            BatchKind::Quads,
            BatchKind::GroupBegin(0),
            BatchKind::Quads,
            BatchKind::GroupEnd(0),
            BatchKind::Quads,
        ]
    );
}

#[test]
fn content_outside_its_group_bounds_still_sorts_inside_the_group() {
    let mut scene = Scene::sized([256, 64]);
    scene.begin_group(rect(0, 0, 16, 16));
    scene.quad(at(100));
    scene.end_group();
    assert_eq!(
        kinds(&mut scene),
        vec![
            BatchKind::GroupBegin(0),
            BatchKind::Quads,
            BatchKind::GroupEnd(0)
        ]
    );
}

#[test]
fn an_external_quad_and_a_backdrop_are_drawn_one_at_a_time() {
    let mut scene = Scene::sized([256, 64]);
    scene.external(at(0));
    scene.external(at(16));
    scene.backdrop(at(32), false);
    assert_eq!(
        kinds(&mut scene),
        vec![
            BatchKind::External(0),
            BatchKind::External(1),
            BatchKind::Backdrop(0)
        ]
    );
}

#[test]
fn batches_break_where_another_kind_must_come_first() {
    let mut scene = Scene::sized([256, 64]);
    scene.quad(rect(0, 0, 20, 20));
    scene.quad(rect(100, 0, 20, 20));
    scene.keyed(LaneKind::Lines, rect(0, 10, 20, 2), [0, 0]);
    scene.quad(rect(0, 10, 20, 4));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let batches: Vec<_> = planned
        .frame
        .batches
        .iter()
        .map(|batch| (batch.kind, batch.range.clone()))
        .collect();
    assert_eq!(
        batches,
        vec![
            (BatchKind::Quads, 0..2),
            (BatchKind::Lines, 0..1),
            (BatchKind::Quads, 2..3),
        ]
    );
}

#[test]
fn marks_batch_by_order() {
    let mut scene = Scene::sized([256, 64]);
    scene.mark(rect(2, 2, 16, 16), false);
    scene.mark(rect(102, 2, 16, 16), false);
    scene.quad(rect(0, 0, 20, 20));
    scene.mark(rect(2, 2, 16, 16), false);
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let batches: Vec<_> = planned
        .frame
        .batches
        .iter()
        .map(|batch| (batch.kind, batch.range.clone()))
        .collect();
    assert_eq!(
        batches,
        vec![
            (BatchKind::Marks, 0..2),
            (BatchKind::Quads, 0..1),
            (BatchKind::Marks, 2..3),
        ]
    );
}

#[test]
fn a_lane_lists_its_sources_in_draw_order() {
    let mut scene = Scene::sized([400, 400]);
    scene.quad(rect(0, 0, 40, 40));
    scene.quad(rect(20, 20, 40, 40));
    scene.quad(rect(300, 300, 40, 40));
    let planned = scene.plan(&DamageSet::full());
    let indices: Vec<u32> = planned
        .frame
        .lanes
        .quads
        .instances()
        .map(|source| source.index)
        .collect();
    assert_eq!(
        indices,
        vec![0, 2, 1],
        "the disjoint quad shares the lowest order"
    );
    let orders: Vec<u32> = planned.planner.orders(LaneKind::Quads).collect();
    assert_eq!(orders, vec![1, 1, 2]);
}

#[test]
fn content_after_a_group_sorts_above_its_end_however_far_away() {
    let mut scene = Scene::sized([400, 400]);
    scene.quad(rect(0, 0, 50, 50));
    scene.begin_group(rect(0, 0, 50, 50));
    scene.end_group();
    scene.quad(rect(300, 300, 10, 10));
    assert_eq!(
        kinds(&mut scene),
        vec![
            BatchKind::Quads,
            BatchKind::GroupBegin(0),
            BatchKind::GroupEnd(0),
            BatchKind::Quads,
        ]
    );
}

#[test]
fn a_later_quad_over_a_paths_overflow_is_drawn_after_it() {
    let mut scene = Scene::sized([400, 400]);
    scene.quad(rect(0, 0, 20, 20));
    scene.plain_path(rect(0, 0, 50, 20));
    scene.quad(rect(35, 5, 10, 10));
    let planned = scene.plan(&DamageSet::full());
    invariants::check(&scene, &DamageSet::full(), &planned);
    let kinds: Vec<_> = planned
        .frame
        .batches
        .iter()
        .map(|batch| batch.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            BatchKind::Quads,
            BatchKind::Paths { pass: 0 },
            BatchKind::Quads
        ]
    );
}
