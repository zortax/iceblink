//! Frame validation of the draw sequence and the plans: batches, groups, vector passes, mark bins
//! and opaque covers.

mod common;

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath};
use iceblink::{
    BatchKind, BlendMode, FRAME_NO_CLIP, FrameGroup, FrameMarkPayload, FramePath, ItemSource,
    MarkBin, MarkFlags, MarkPayload, OpaqueCover, PassKey, PlannedPass, PlannedPath,
};
use iceblink_testing::validate::Rule;
use iceblink_testing::validate_frame;

use crate::common::frames::{
    SIDE, batch, first, marks, object, push_quad, quad, rect, rules, with_quad,
};

#[test]
fn a_batch_range_stays_inside_its_lane() {
    let mut frame = with_quad();
    frame.batches[0].range = 0..2;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::BatchRanges]);
}

#[test]
fn the_batches_of_a_lane_draw_each_source_once_in_order() {
    let mut frame = with_quad();
    push_quad(&mut frame, quad([8.0, 8.0, 8.0, 8.0]));
    frame.batches.swap(0, 1);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::BatchCoverage]);

    frame.batches.truncate(1);
    frame.batches[0].range = 0..1;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::BatchCoverage]);
}

/// A group over the whole output with nothing but opacity.
fn group() -> FrameGroup {
    FrameGroup {
        bounds: [0.0, 0.0, SIDE as f32, SIDE as f32],
        source: [0.0, 0.0, SIDE as f32, SIDE as f32],
        clip: FRAME_NO_CLIP,
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
    }
}

#[test]
fn group_markers_pair_up_and_nest() {
    let mut frame = first();
    frame.groups = vec![group(), group()];
    frame.batches = vec![
        object(BatchKind::GroupBegin(0)),
        object(BatchKind::GroupBegin(1)),
        object(BatchKind::GroupEnd(1)),
        object(BatchKind::GroupEnd(0)),
    ];
    assert_eq!(validate_frame(&frame), Ok(()));

    frame.batches.swap(2, 3);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::GroupNesting]);
}

#[test]
fn a_group_lies_inside_the_output() {
    let mut frame = first();
    frame.groups = vec![group()];
    frame.batches = vec![
        object(BatchKind::GroupBegin(0)),
        object(BatchKind::GroupEnd(0)),
    ];
    frame.groups[0].bounds = [-4.0, 8.0, 16.0, 16.0];
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::GroupBounds]);

    frame.groups[0].bounds = [8.0, 8.0, SIDE as f32, 16.0];
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::GroupBounds]);
}

#[test]
fn a_batch_names_an_object_that_exists() {
    let mut frame = first();
    frame.batches.push(object(BatchKind::External(0)));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::BatchObjects]);
}

/// A frame with one path planned into one pass with `region`.
fn planned(region: iceblink::DeviceRect) -> iceblink::Frame {
    let mut frame = first();
    frame.paths.push(FramePath {
        path: Arc::new(BezPath::new()),
        transform: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: None,
        stroke: None,
        clips: Arc::from(Vec::new()),
        ink: rect(4, 4, 8, 8),
    });
    frame.vector_plan.items.push(PlannedPath {
        path: 0,
        residual: FRAME_NO_CLIP,
        clip: FRAME_NO_CLIP,
        ink: rect(4, 4, 8, 8),
    });
    frame.vector_plan.passes.push(PlannedPass {
        items: 0..1,
        region,
        clip: FRAME_NO_CLIP,
        instanced: false,
        clamped: false,
        overhangs: false,
        key: PassKey::NONE,
    });
    frame.batches.push(object(BatchKind::Paths { pass: 0 }));
    frame
}

#[test]
fn a_pass_region_is_on_the_tile_grid_and_holds_its_damaged_ink() {
    assert_eq!(validate_frame(&planned(rect(0, 0, 16, 16))), Ok(()));
    assert_eq!(
        rules(validate_frame(&planned(rect(4, 4, 16, 16)))),
        vec![Rule::VectorPlan]
    );
    assert_eq!(
        rules(validate_frame(&planned(rect(16, 0, 16, 16)))),
        vec![Rule::VectorPlan]
    );
}

#[test]
fn every_path_is_planned_once() {
    let mut frame = planned(rect(0, 0, 16, 16));
    frame.vector_plan.items.push(frame.vector_plan.items[0]);
    frame.vector_plan.passes[0].items = 0..2;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::VectorPlan]);
}

#[test]
fn every_union_mark_has_one_bin() {
    let mut frame = first();
    frame.lanes.marks.items.push(marks(1, MarkFlags::UNION));
    frame.lanes.marks.sources.push(ItemSource::transient(0));
    frame.lanes.mark_payloads.push(FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            discs: vec![[4.0, 4.0, 2.0, 0.0]],
            ..MarkPayload::default()
        }),
        cells: Arc::from(Vec::new()),
    });
    frame.batches.push(batch(BatchKind::Marks, 0..1));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::MarkPlan]);

    frame.mark_plan.bins.push(MarkBin {
        item: 0,
        region: rect(0, 0, 16, 16),
        at: [0, 0],
        page: 0,
    });
    frame.mark_plan.pages = 1;
    frame.mark_plan.extent = [16, 16];
    frame.mark_plan.full_damage = true;
    assert_eq!(validate_frame(&frame), Ok(()));
}

#[test]
fn an_opaque_cover_names_a_quad_drawn_outside_every_group() {
    let mut frame = with_quad();
    frame.opaque_covers.push(OpaqueCover {
        rect: rect(1, 1, 14, 14),
        batch: 0,
        at: 0,
    });
    assert_eq!(validate_frame(&frame), Ok(()));

    frame.groups.push(iceblink::FrameGroup {
        opacity: 1.0,
        ..group()
    });
    frame.batches.insert(0, object(BatchKind::GroupBegin(0)));
    frame.batches.push(object(BatchKind::GroupEnd(0)));
    frame.opaque_covers[0].batch = 1;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::OpaqueCovers]);
}

#[test]
fn a_group_or_an_external_may_go_undrawn() {
    let mut frame = first();
    frame.groups.push(group());
    frame.externals.push(iceblink::FrameExternal {
        quad: iceblink::ExternalQuad {
            bounds: [0.0, 0.0, 8.0, 8.0],
            texture: 0,
            opacity: 1.0,
            clip: iceblink::ClipRef(FRAME_NO_CLIP),
            space: iceblink::SpaceRef(0),
        },
        client: iceblink::ClientId(1),
        texture: iceblink::IdSpace::new(iceblink::ClientId(1)).mint::<iceblink::TextureDesc>(),
    });
    assert_eq!(validate_frame(&frame), Ok(()));

    frame.batches.push(object(BatchKind::External(0)));
    frame.batches.push(object(BatchKind::External(0)));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::BatchObjects]);
}

#[test]
fn a_path_outside_the_damage_may_go_unplanned() {
    let mut frame = planned(rect(0, 0, 16, 16));
    frame.vector_plan.items.clear();
    frame.vector_plan.passes.clear();
    frame.batches.clear();
    assert_eq!(validate_frame(&frame), Ok(()));
}

#[test]
fn a_full_damage_mark_plan_accepts_damage_that_covers_the_output() {
    let mut frame = first();
    let mut damage = iceblink::damage::DamageSet::new();
    damage.absorb(rect(0, 0, SIDE as i32, SIDE as i32));
    frame.damage.draw = damage;
    frame.mark_plan.full_damage = true;
    assert_eq!(validate_frame(&frame), Ok(()));

    let mut partial = iceblink::damage::DamageSet::new();
    partial.absorb(rect(0, 0, 8, 8));
    frame.damage.draw = partial;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::MarkPlan]);
}

#[test]
fn a_keyed_pass_is_unclamped() {
    let mut frame = planned(rect(0, 0, 16, 16));
    frame.vector_plan.passes[0].key = PassKey(7);
    assert_eq!(validate_frame(&frame), Ok(()));

    frame.vector_plan.passes[0].clamped = true;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::VectorPlan]);
}

#[test]
fn a_pass_key_is_never_issued_for_other_content() {
    let mut validator = iceblink_testing::FrameValidator::new();
    let mut frame = planned(rect(0, 0, 16, 16));
    frame.vector_plan.passes[0].key = PassKey(7);
    assert_eq!(validator.check(&frame), Ok(()));

    let mut next = planned(rect(0, 0, 16, 16));
    next.vector_plan.passes[0].key = PassKey(7);
    next.paths[0].transform = Affine::translate((1.0, 0.0));
    assert_eq!(rules(validator.check(&next)), vec![Rule::VectorPlan]);
}
