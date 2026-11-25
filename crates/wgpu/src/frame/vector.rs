//! The composite instances of a resourced vector pass.

use iceblink::geom::{Point, Rect};

use crate::pipeline::vector::VectorInstance;
use crate::vector::{ResourcedPass, ResourcedPlan};

/// Returns one instance for the whole pass, or one per path of an instanced pass.
///
/// The destination is in device pixels, and the source starts at the pass's place in its scratch.
pub(crate) fn instances_of(plan: &ResourcedPlan, pass: &ResourcedPass) -> Vec<VectorInstance> {
    let origin = pass.region.origin;
    let raster = pass.raster_region.origin;
    if !pass.instanced {
        return vec![VectorInstance::new(
            pass.region,
            (raster.x, raster.y),
            pass.clip,
        )];
    }
    plan.items_of(pass)
        .iter()
        .map(|item| {
            let bounds = Rect::new(
                Point::new(origin.x + item.ink.origin.x, origin.y + item.ink.origin.y),
                item.ink.size,
            );
            VectorInstance::new(
                bounds,
                (raster.x + item.ink.origin.x, raster.y + item.ink.origin.y),
                item.clip,
            )
        })
        .collect()
}
