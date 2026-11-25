//! World matrices of the transform nodes a commit made stale.

mod sticky;
mod thumb;

use iceblink::{Anchor, Matrix4, TransformNode};
use smallvec::SmallVec;

use crate::derive::links::Cause;
use crate::scene::Scene;
use crate::scene::follows::Follows;
use crate::small::{SmallMap, SmallSet};
use crate::tables::FrameTables;

/// A transform node whose world matrix changed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MovedSpace {
    pub(crate) slot: u32,
    pub(crate) before: Matrix4,
    pub(crate) after: Matrix4,
    pub(crate) cause: Cause,
}

/// The most transform nodes one commit moves that are tracked without allocating.
const INLINE: usize = 8;

/// The transform nodes whose world matrix changed, by slot.
#[derive(Debug, Default)]
pub(crate) struct WorldChanges {
    pub(crate) moved: SmallMap<u32, MovedSpace, INLINE>,
}

/// Recomputes the world matrices of `dirty` and their descendants, writes them to the space
/// table, and adds the nodes whose matrix changed to `changes`.
pub(crate) fn refresh(
    scene: &mut Scene,
    tables: &mut FrameTables,
    dirty: &[(u32, Cause)],
    changes: &mut WorldChanges,
) {
    let written: SmallSet<u32, INLINE> = dirty
        .iter()
        .filter(|(_, cause)| *cause == Cause::Write)
        .map(|&(slot, _)| (slot, ()))
        .collect();
    let mut roots: SmallVec<[(usize, u32, Cause); 4]> = dirty
        .iter()
        .filter(|(slot, _)| scene.transforms.at(*slot).is_some())
        .map(|&(slot, cause)| (depth(scene, slot), slot, cause))
        .collect();
    roots.sort_unstable_by_key(|&(depth, slot, _)| (depth, slot));

    let mut visited: SmallSet<u32, INLINE> = SmallSet::default();
    let mut stack: SmallVec<[(u32, Cause); INLINE]> = SmallVec::new();
    for (_, root, cause) in roots {
        if visited.contains(&root) {
            continue;
        }
        stack.push((root, cause));
        while let Some((slot, inherited)) = stack.pop() {
            if !visited.add(slot) {
                continue;
            }
            let cause = if written.contains(&slot) {
                Cause::Write
            } else {
                inherited
            };
            let Some((before, after)) = recompute(scene, slot) else {
                continue;
            };

            if before != after || cause == Cause::Write {
                let frame = scene.transforms.at(slot).map(|record| record.links.frame);
                if let Some(frame) = frame {
                    tables.spaces.write(frame, after);
                }
                changes.moved.insert(
                    slot,
                    MovedSpace {
                        slot,
                        before,
                        after,
                        cause,
                    },
                );
            }

            if let Some(record) = scene.transforms.at(slot) {
                stack.extend(record.children.iter().map(|&child| (child, cause)));
            }
        }
    }
}

/// Recomputes one node's matrices and what they follow from its parent's, and returns its old
/// and new world matrix.
fn recompute(scene: &mut Scene, slot: u32) -> Option<(Matrix4, Matrix4)> {
    let record = scene.transforms.at(slot)?;
    let node = record.node;
    let parent = node
        .parent
        .and_then(|parent| scene.transforms.at(parent.slot()));
    let (parent_world, parent_still) = parent.map_or((Matrix4::IDENTITY, Matrix4::IDENTITY), |p| {
        (p.world, p.still)
    });
    let parent_follows = parent.map_or((Follows::Still, Follows::Still), |p| {
        (p.follows, p.still_follows)
    });

    let offset = node
        .scroll
        .and_then(|scroll| scene.scrolls.at(scroll.slot()))
        .map_or([0.0; 2], |scroll| {
            let [x, y] = scroll.node.offset;
            [x + scroll.overscroll[0], y + scroll.overscroll[1]]
        });
    let scrolled = Matrix4::translation(-offset[0], -offset[1], 0.0);
    let anchored = match node.anchor {
        Anchor::Sticky(anchor) => sticky::offset(scene, anchor),
        Anchor::Thumb(anchor) => thumb::offset(scene, anchor),
        Anchor::Scrolls | Anchor::Fixed => [0.0; 2],
    };
    let stuck = Matrix4::translation(anchored[0], anchored[1], 0.0);
    let above = match node.anchor {
        Anchor::Fixed => parent_still,
        Anchor::Scrolls | Anchor::Sticky(_) | Anchor::Thumb(_) => parent_world,
    };

    let placed = node.local.then(&stuck);
    let world = scrolled.then(&placed).then(&above);
    let still = placed.then(&parent_still);

    let (follows, still_follows) = follows(&node, &world, parent_follows);

    let record = scene.transforms.at_mut(slot)?;
    let before = record.world;
    record.world = world;
    record.still = still;
    record.follows = follows;
    record.still_follows = still_follows;
    Some((before, world))
}

/// Returns the scroll offsets a node's world and still matrices follow, from those its parent's
/// follow.
fn follows(
    node: &TransformNode,
    world: &Matrix4,
    (parent_world, parent_still): (Follows, Follows),
) -> (Follows, Follows) {
    let own = node
        .scroll
        .map_or(Follows::Still, |scroll| Follows::Scroll(scroll.slot()));
    let sticks = match node.anchor {
        Anchor::Sticky(_) | Anchor::Thumb(_) => Follows::Mixed,
        Anchor::Scrolls | Anchor::Fixed => Follows::Still,
    };
    let above = match node.anchor {
        Anchor::Fixed => parent_still,
        Anchor::Scrolls | Anchor::Sticky(_) | Anchor::Thumb(_) => parent_world,
    };
    let follows = match own.and(sticks).and(above) {
        // A projective matrix can map an ink to the whole output, which no translation moves.
        Follows::Scroll(_) if !world.is_2d() => Follows::Mixed,
        follows => follows,
    };
    (follows, sticks.and(parent_still))
}

/// Returns how many ancestors a transform node has.
fn depth(scene: &Scene, slot: u32) -> usize {
    let mut depth = 0;
    let mut current = slot;
    while let Some(parent) = scene
        .transforms
        .at(current)
        .and_then(|record| record.node.parent)
    {
        depth += 1;
        current = parent.slot();
        if depth > scene.transforms.span() {
            break;
        }
    }
    depth
}
