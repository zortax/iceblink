//! Where the clip links that stay still while a scroll node moves cover its port in part.

use iceblink::{DeviceRect, OrderEntry};

use crate::geometry::admits::{Moved, keeps_whole};
use crate::geometry::bounds::of_pixels;
use crate::geometry::coverage::Partial;
use crate::scene::Scene;
use crate::scene::inks::Carry;
use crate::scene::kind::Kind;

/// Returns where the clip links entry `entry` is drawn through cover `port` in part, when every
/// link either follows scroll node `scroll` or holds all of `port`.
///
/// Links that follow another scroll node stay where they are while `scroll` moves, so they count
/// as still. A link whose space follows several offsets counts as one that follows `scroll` or
/// none as `moved` reports. A chunk is checked through the clip nodes it refers to, and an order
/// node through its carry. The result for an order node that `scroll` does not carry is `None`.
pub(super) fn keeps_port(
    scene: &Scene,
    entry: OrderEntry,
    scroll: u32,
    port: DeviceRect,
    moved: Moved<'_>,
) -> Option<Partial> {
    match entry {
        OrderEntry::Chunk(key) => {
            let record = scene.chunks.get(&key)?;
            let clips = record.derived.deps.iter();
            clips.filter(|object| object.kind == Kind::Clip).try_fold(
                Partial::NONE,
                |partial, object| {
                    let added =
                        keeps_whole(scene, object.slot, of_pixels(port), scroll, true, moved)?;
                    Some(partial.and(added))
                },
            )
        }
        OrderEntry::Node(id) => {
            let carry = scene.orders.at(id.slot())?.ink.carry()?;
            (carry.scroll == scroll).then(|| carry_keeps(scene, carry, port))?
        }
    }
}

/// Returns where the still clip links of carried content cover `port` in part, when they hold
/// all of it.
pub(super) fn carry_keeps(scene: &Scene, carry: Carry, port: DeviceRect) -> Option<Partial> {
    scene
        .still_partial(carry.still)
        .filter(|_| scene.still_pixels(carry.still).contains_rect(port))
}
