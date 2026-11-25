//! Transform, scroll, clip and effect nodes, and paint and parameter slots.

use iceblink::{
    ClipId, ClipNode, EffectId, EffectNode, Matrix4, Paint, PaintId, ParamsBlock, ParamsId,
    RejectReason, ScrollId, ScrollNode, TransformId, TransformNode,
};

use crate::naive::commit::check::{creatable, finite, live, nodes};
use crate::naive::commit::draft::Draft;

pub(super) fn create_transform(
    draft: &mut Draft<'_>,
    id: TransformId,
    node: TransformNode,
) -> Result<(), RejectReason> {
    creatable(&draft.client.transforms, id)?;
    nodes::transform(&draft.client, &node)?;
    draft.client.transforms.create(id, node)
}

pub(super) fn set_transform(
    draft: &mut Draft<'_>,
    id: TransformId,
    local: Matrix4,
) -> Result<(), RejectReason> {
    live(&draft.client.transforms, id)?;
    finite::matrix(&local)?;
    let node = draft
        .client
        .transforms
        .get_mut(id)
        .ok_or(RejectReason::StaleId)?;
    node.local = local;
    draft.counters.property_writes += 1;
    Ok(())
}

pub(super) fn set_transform_node(
    draft: &mut Draft<'_>,
    id: TransformId,
    node: TransformNode,
) -> Result<(), RejectReason> {
    if !draft.client.transforms.is_live(id) {
        return Err(RejectReason::StaleId);
    }

    nodes::transform(&draft.client, &node)?;
    *draft.client.transforms.get_mut(id).expect("checked above") = node;
    draft.counters.property_writes += 1;
    Ok(())
}

pub(super) fn create_scroll(
    draft: &mut Draft<'_>,
    id: ScrollId,
    node: ScrollNode,
) -> Result<(), RejectReason> {
    creatable(&draft.client.scrolls, id)?;
    nodes::scroll(&draft.client, &node)?;
    draft.client.scrolls.create(id, node)
}

pub(super) fn set_scroll_node(
    draft: &mut Draft<'_>,
    id: ScrollId,
    node: ScrollNode,
) -> Result<(), RejectReason> {
    if !draft.client.scrolls.is_live(id) {
        return Err(RejectReason::StaleId);
    }

    nodes::scroll(&draft.client, &node)?;
    *draft.client.scrolls.get_mut(id).expect("checked above") = node;
    draft.counters.property_writes += 1;
    Ok(())
}

/// Writes a scroll node's offset, for `scroll_to` and `acknowledge_scroll`.
///
/// The compositor moves no scroll node on its own, so an acknowledged offset is the new base
/// offset like a written one.
pub(super) fn scroll_to(
    draft: &mut Draft<'_>,
    id: ScrollId,
    offset: [f32; 2],
) -> Result<(), RejectReason> {
    let node = draft
        .client
        .scrolls
        .get_mut(id)
        .ok_or(RejectReason::StaleId)?;
    finite::numbers(&offset)?;
    node.offset = offset;
    draft.counters.property_writes += 1;
    Ok(())
}

pub(super) fn create_clip(
    draft: &mut Draft<'_>,
    id: ClipId,
    node: ClipNode,
) -> Result<(), RejectReason> {
    creatable(&draft.client.clips, id)?;
    nodes::clip(&draft.client, &node)?;
    draft.client.clips.create(id, node)
}

pub(super) fn set_clip(
    draft: &mut Draft<'_>,
    id: ClipId,
    node: ClipNode,
) -> Result<(), RejectReason> {
    if !draft.client.clips.is_live(id) {
        return Err(RejectReason::StaleId);
    }

    nodes::clip(&draft.client, &node)?;
    *draft.client.clips.get_mut(id).expect("checked above") = node;
    draft.counters.property_writes += 1;
    Ok(())
}

pub(super) fn create_effect(
    draft: &mut Draft<'_>,
    id: EffectId,
    node: EffectNode,
) -> Result<(), RejectReason> {
    creatable(&draft.client.effects, id)?;
    nodes::effect(&draft.client, &node)?;
    draft.client.effects.create(id, node)
}

pub(super) fn set_effect(
    draft: &mut Draft<'_>,
    id: EffectId,
    node: EffectNode,
) -> Result<(), RejectReason> {
    if !draft.client.effects.is_live(id) {
        return Err(RejectReason::StaleId);
    }

    nodes::effect(&draft.client, &node)?;
    *draft.client.effects.get_mut(id).expect("checked above") = node;
    draft.counters.property_writes += 1;
    Ok(())
}

pub(super) fn create_paint(
    draft: &mut Draft<'_>,
    id: PaintId,
    paint: Paint,
) -> Result<(), RejectReason> {
    creatable(&draft.client.paints, id)?;
    nodes::paint(&paint)?;
    draft.client.paints.create(id, paint)
}

pub(super) fn set_paint(
    draft: &mut Draft<'_>,
    id: PaintId,
    paint: Paint,
) -> Result<(), RejectReason> {
    let held = draft
        .client
        .paints
        .get_mut(id)
        .ok_or(RejectReason::StaleId)?;
    if held.kind() != paint.kind() {
        return Err(RejectReason::PaintKindChanged);
    }
    nodes::paint(&paint)?;

    *held = paint;
    draft.counters.paint_writes += 1;
    Ok(())
}

pub(super) fn create_params(
    draft: &mut Draft<'_>,
    id: ParamsId,
    block: ParamsBlock,
) -> Result<(), RejectReason> {
    creatable(&draft.client.params, id)?;
    nodes::params(&block)?;
    draft.client.params.create(id, block)
}

pub(super) fn set_params(
    draft: &mut Draft<'_>,
    id: ParamsId,
    block: ParamsBlock,
) -> Result<(), RejectReason> {
    let held = draft
        .client
        .params
        .get_mut(id)
        .ok_or(RejectReason::StaleId)?;
    nodes::params(&block)?;
    *held = block;
    draft.counters.paint_writes += 1;
    Ok(())
}
