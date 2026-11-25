//! Groups that move with the content around them.

use iceblink::{DeviceRect, OrderEntry};
use rustc_hash::FxHashSet;

use crate::geometry::admits::admits;
use crate::geometry::bounds::of_pixels;
use crate::scene::Scene;
use crate::shift::rule::scan::paints_in;

/// Reports whether everything the order node at `slot` paints in `region` moves with scroll node
/// `scroll`, and the clip of its effect admits all of `region` before and after the move.
pub(super) fn moves_whole(
    scene: &Scene,
    slot: u32,
    moving: &FxHashSet<OrderEntry>,
    scroll: u32,
    region: DeviceRect,
) -> bool {
    let Some(record) = scene.orders.at(slot) else {
        return true;
    };
    if !clip_moves(
        scene,
        record.node.effect.map(|effect| effect.slot()),
        scroll,
        region,
    ) {
        return false;
    }

    record.entries.iter().all(|entry| {
        moving.contains(entry)
            || !paints_in(scene, *entry, region)
            || matches!(entry, OrderEntry::Node(id) if moves_whole(scene, id.slot(), moving, scroll, region))
    })
}

/// Reports whether the clip of the effect at `effect` admits all of `region` before and after a
/// move by scroll node `scroll`, or there is none.
fn clip_moves(scene: &Scene, effect: Option<u32>, scroll: u32, region: DeviceRect) -> bool {
    let clip = effect
        .and_then(|effect| scene.effects.at(effect))
        .and_then(|effect| effect.node.clip);
    clip.is_none_or(|clip| admits(scene, clip.slot(), of_pixels(region), Some(scroll)))
}
