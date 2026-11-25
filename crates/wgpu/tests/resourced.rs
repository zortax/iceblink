//! Round grouping of resourced vector plans.

use iceblink::geom::{Point, Rect, Size};
use iceblink::{PassKey, PlannedPass, PlannedPath, VectorPlan};
use iceblink_wgpu::vector::{Capacity, Layering, ResourcedPass, ResourcedPlan, VectorTarget};

#[test]
fn a_resourced_plan_groups_its_passes_by_round() {
    let pass = |items: core::ops::Range<u32>| PlannedPass {
        items,
        region: Rect::new(Point::new(0, 0), Size::new(16, 16)),
        clip: 0,
        instanced: false,
        clamped: false,
        overhangs: false,
        key: PassKey::NONE,
    };
    let path = |path| PlannedPath {
        path,
        residual: 0,
        clip: 0,
        ink: Rect::new(Point::new(0, 0), Size::new(16, 16)),
    };
    let plan = VectorPlan {
        items: vec![path(0), path(1), path(2)],
        passes: vec![pass(0..2), pass(2..3)],
    };
    let mut resourced = ResourcedPlan::resourcing(&plan);
    for (round, planned) in [0, 1].into_iter().zip(&plan.passes) {
        resourced.passes.push(ResourcedPass {
            region: planned.region,
            raster_region: planned.region,
            target: VectorTarget(0),
            round,
            items: planned.items.clone(),
            clip: planned.clip,
            instanced: planned.instanced,
            clamped: planned.clamped,
        });
    }

    assert_eq!(resourced.rounds(), 2);
    assert_eq!(resourced.round(1), 1..2);
    assert_eq!(resourced.round(2), 2..2);
    assert_eq!(resourced.items_of(&resourced.passes[0]).len(), 2);
    assert_eq!(plan.items_of(&plan.passes[1])[0].path, 2);
}

#[test]
fn a_placed_plan_keeps_one_pass_per_planned_pass_and_lists_each_target_once() {
    let region = |side| Rect::new(Point::new(0, 0), Size::new(side, side));
    let pass = |items: core::ops::Range<u32>, side, overhangs| PlannedPass {
        items,
        region: region(side),
        clip: 0,
        instanced: false,
        clamped: false,
        overhangs,
        key: PassKey::NONE,
    };
    let path = |path| PlannedPath {
        path,
        residual: 0,
        clip: 0,
        ink: region(16),
    };
    let plan = VectorPlan {
        items: vec![path(0), path(1), path(2)],
        passes: vec![
            pass(0..1, 64, false),
            pass(1..2, 64, true),
            pass(2..3, 64, false),
        ],
    };
    let regions: Vec<_> = plan.passes.iter().map(|pass| pass.region).collect();
    let capacity = Capacity {
        side: 64,
        layers: 2,
    };

    let resourced = ResourcedPlan::placed(&plan, &Layering::of(&regions, capacity));

    assert_eq!(resourced.passes.len(), 3);
    assert!(
        resourced.passes[1].clamped,
        "an overhanging pass is kept inside its bin"
    );
    assert_eq!(resourced.rounds(), 2);
    let mut targets = Vec::new();
    resourced.targets_of(0, &mut targets);
    assert_eq!(targets, [VectorTarget(0), VectorTarget(1)]);
    resourced.targets_of(1, &mut targets);
    assert_eq!(targets, [VectorTarget(0)]);
}
