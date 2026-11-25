//! Choosing the snap point a motion comes to rest at.

use iceblink::{ScrollBehaviour, SnapKind};

/// Returns the offset on `axis` that motion projected to rest at `projection` snaps to, or
/// `None` when no snap point applies.
///
/// The snap points whose offset on the axis is a number count, clamped to `[0, max]`. The one
/// nearest the projection is taken. With `SnapKind::Proximity`, it must lie within a third of
/// the port's extent `extent` of the projection.
pub(crate) fn snap_target(
    behaviour: &ScrollBehaviour,
    axis: usize,
    projection: f64,
    max: f32,
    extent: f32,
) -> Option<f64> {
    let nearest = behaviour
        .snap
        .iter()
        .map(|point| point.offset[axis])
        .filter(|offset| !offset.is_nan())
        .map(|offset| f64::from(offset.clamp(0.0, max)))
        .min_by(|a, b| (a - projection).abs().total_cmp(&(b - projection).abs()))?;
    let near = match behaviour.snap_kind {
        SnapKind::Mandatory => true,
        SnapKind::Proximity => (nearest - projection).abs() <= f64::from(extent) / 3.0,
    };
    near.then_some(nearest)
}
