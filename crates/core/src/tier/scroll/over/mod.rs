//! The scroll ports drawn over the frontmost hit at a device point.

mod kept;
mod walk;

pub(crate) use self::kept::Origins;

use iceblink::ScrollId;
use smallvec::SmallVec;

use self::kept::Key;
use self::walk::{Found, walk};
use crate::hit::Hit;
use crate::scene::Scene;
use crate::scene::content::{applied_scrolls, draws_in};
use crate::tier::View;
use crate::tier::scroll::route::candidates;
use crate::tier::scroll::state::{Geometry, behaviour};

/// Returns the transform node a gesture at `position` is routed from: a space of the frontmost
/// port drawn over `hit`, or the space of `hit` when no port is.
///
/// A port holds the position when the compositor scrolls its node, and when it and the ports of
/// every scroll node that applies to its space contain `position`. A port's content is every item
/// drawn in a space that applies its scroll node. A port that holds the position is drawn over
/// the hit in these cases:
///
/// - If its content draws no item in the paint order under `root`, it is drawn over a missing
///   hit, and over a hit when every port that holds the position and applies to the hit's space
///   also applies to the port's space. It is then in front of every port whose content draws an
///   item, and of several such ports the innermost is in front. The gesture routes from a space
///   that applies its scroll node.
/// - If the first item of its content comes after the hit, or there is no hit, it is drawn over
///   the hit. The frontmost of these ports is the one whose first item comes last, and the gesture
///   routes from the space of that item.
///
/// The start is kept in `origins` for the next gesture with the same root, hit, ports and hit's
/// chain. `walks` counts the walks over the paint order that find the first items of ports.
pub(crate) fn origin(
    scene: &Scene,
    origins: &mut Origins,
    walks: &mut u64,
    root: u32,
    hit: Option<Hit>,
    position: [f32; 2],
) -> Option<u32> {
    let view = View {
        transforms: &scene.transforms,
        scrolls: &scene.scrolls,
        effects: &scene.effects,
    };

    let below = hit.map_or_else(SmallVec::new, |hit| candidates(&view, hit.space, position));
    let ports = held_ports(scene, &view, &below, position);
    if ports.is_empty() {
        return hit.map(|hit| hit.space);
    }

    let key = Key {
        root,
        hit: hit.map(|hit| (hit.result.chunk, hit.result.item)),
        ports,
        below,
    };
    if let Some(start) = origins.get(&key) {
        return start;
    }

    let found = find(scene, view, &key, walks);
    let start = match innermost_blank(&view, &found.blank, &key.below) {
        Some(space) => Some(space),
        None => found.front.or(hit.map(|hit| hit.space)),
    };
    origins.keep(key, start);
    start
}

/// Returns the slots of the ports that hold `position`, other than those of the hit's chain
/// `below`.
fn held_ports(
    scene: &Scene,
    view: &View<'_>,
    below: &[ScrollId],
    position: [f32; 2],
) -> SmallVec<[u32; 4]> {
    let mut ports = SmallVec::new();
    scene.scrolls.each(|slot, _, record| {
        let held = behaviour(record).is_some()
            && Geometry::of(view, record).is_some_and(|geometry| geometry.holds(position))
            && inside(view, record.node.space.slot(), position);
        if held && !below.iter().any(|id| id.slot() == slot) {
            ports.push(slot);
        }
    });
    ports
}

/// Returns the ports of `key` whose content draws no item, and the space of the first item of the
/// frontmost port drawn over the hit.
///
/// A walk over the paint order is counted in `walks` when the ports need one.
fn find(scene: &Scene, view: View<'_>, key: &Key, walks: &mut u64) -> Found {
    // A port no chunk draws in is blank without a walk.
    let (blank, drawn): (SmallVec<[u32; 4]>, SmallVec<[u32; 4]>) = key
        .ports
        .iter()
        .copied()
        .partition(|&slot| !draws_in(scene, slot));
    let mut found = Found { blank, front: None };
    if !drawn.is_empty() {
        *walks += 1;
        let walked = walk(scene, view, key.root, key.hit, drawn);
        found.blank.extend(walked.blank);
        found.front = walked.front;
    }
    found
}

/// Returns a space that applies the innermost of the `blank` ports whose chain holds every
/// scroll node of `below`, or `None` when there is none.
fn innermost_blank(view: &View<'_>, blank: &[u32], below: &[ScrollId]) -> Option<u32> {
    blank
        .iter()
        .filter_map(|&slot| {
            let around = applied_scrolls(view.transforms, view.scrolls.at(slot)?.node.space.slot());
            if !below.iter().all(|id| around.contains(&id.slot())) {
                return None;
            }
            Some((around.len(), applier(view, slot)?))
        })
        .max_by_key(|&(depth, _)| depth)
        .map(|(_, space)| space)
}

/// Returns a transform node that applies the scroll node at `slot`.
fn applier(view: &View<'_>, slot: u32) -> Option<u32> {
    let record = view.scrolls.at(slot)?;
    record.dependents.iter().copied().find(|&node| {
        view.transforms
            .at(node)
            .and_then(|transform| transform.node.scroll)
            .is_some_and(|scroll| scroll.slot() == slot)
    })
}

/// Reports whether the port of every scroll node that applies to the transform node `space`
/// holds `position`.
fn inside(view: &View<'_>, space: u32, position: [f32; 2]) -> bool {
    applied_scrolls(view.transforms, space).iter().all(|&slot| {
        view.scrolls
            .at(slot)
            .and_then(|record| Geometry::of(view, record))
            .is_some_and(|geometry| geometry.holds(position))
    })
}
