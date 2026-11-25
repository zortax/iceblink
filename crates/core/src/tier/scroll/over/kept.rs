//! The routing start kept from the last gesture.

use iceblink::{Anchor, ChunkKey, OrderEntry, ScrollId, TransformNode};
use smallvec::SmallVec;

use crate::commit::journal::{Journal, Op, OrderChange};
use crate::commit::validate::ChunkNet;
use crate::scene::Scene;
use crate::scene::kind::{Kind, ObjectRef};
use crate::scene::record::OrderRecord;
use crate::small::SmallMap;

/// The start the last routed gesture took, kept until a commit changes what it depends on.
///
/// A gesture with the same root, hit, ports holding its position and scroll nodes in the hit's
/// chain takes the same start.
#[derive(Debug, Default)]
pub(crate) struct Origins {
    held: Option<(Key, Option<u32>)>,
}

/// What a routing start depends on besides the scene's structure.
#[derive(Debug, PartialEq)]
pub(super) struct Key {
    pub(super) root: u32,
    /// The chunk and item index of the hit.
    pub(super) hit: Option<(ChunkKey, u32)>,
    /// The slots of the ports that hold the position, outside the hit's chain.
    pub(super) ports: SmallVec<[u32; 4]>,
    /// The scroll nodes in the hit's chain.
    pub(super) below: SmallVec<[ScrollId; 4]>,
}

impl Origins {
    /// Returns the start kept for `key`, if one is.
    pub(super) fn get(&self, key: &Key) -> Option<Option<u32>> {
        self.held
            .as_ref()
            .filter(|(held, _)| held == key)
            .map(|&(_, start)| start)
    }

    /// Keeps `start` as the start for `key`.
    pub(super) fn keep(&mut self, key: Key, start: Option<u32>) {
        self.held = Some((key, start));
    }

    /// Forgets the kept start.
    pub(crate) fn forget(&mut self) {
        self.held = None;
    }

    /// Reports whether the kept start still applies after the commit of `journal`.
    ///
    /// `nets` holds the chunk keys the commit changed, with the objects their chunks referred to
    /// before and after, and `entries` the entries it spliced in. The start still applies unless the
    /// commit does one of these:
    ///
    /// 1. It writes a root, creates or removes a scroll node, or moves a port to another space.
    /// 2. It gives a transform node another parent, scroll node or anchor, or removes the start.
    /// 3. It changes the hit's chunk, or a chunk that draws in a kept port before or after the
    ///    commit.
    /// 4. It splices in or out an entry whose subtree holds such a chunk.
    ///
    /// A chunk draws in a port when it refers to a transform node the port's scroll node applies
    /// to. Without a kept start there is nothing to forget, and the result is `true`.
    pub(crate) fn survives(
        &self,
        scene: &Scene,
        journal: &Journal,
        nets: &[ChunkNet],
        entries: &[OrderEntry],
    ) -> bool {
        let Some((key, start)) = &self.held else {
            return true;
        };
        if restructures(scene, journal, *start) {
            return false;
        }

        let mut ports = Ports {
            scene,
            journal,
            key,
            spaces: SmallMap::default(),
        };
        let hit = key.hit.map(|(chunk, _)| chunk);
        for net in nets {
            if Some(net.key) == hit {
                return false;
            }
            let after = net.after.as_deref().unwrap_or_default();
            let before = net.before.as_deref().unwrap_or_default();
            if ports.refers(after) || (!same_spaces(before, after) && ports.refers(before)) {
                return false;
            }
        }

        !journal.orders.iter().any(|change| match change {
            OrderChange::Splice(splice) => {
                let inserted = entries.get(splice.entries.indices()).unwrap_or(entries);
                journal
                    .removed(splice)
                    .iter()
                    .chain(inserted)
                    .any(|&entry| ports.reaches(entry, 0))
            }
            OrderChange::Node => false,
        })
    }
}

/// Reports whether `journal` writes a root, creates or removes a scroll node, moves a port to
/// another space, gives a transform node another parent, scroll node or anchor, or removes the
/// transform node `start`.
fn restructures(scene: &Scene, journal: &Journal, start: Option<u32>) -> bool {
    !journal.roots.is_empty()
        || journal.scrolls.iter().any(|change| match &change.op {
            Op::Created(_) | Op::Removed(_) => true,
            Op::Wrote(before) => scene
                .scrolls
                .at(change.slot)
                .is_some_and(|record| record.node.space != before.space),
        })
        || journal.transforms.iter().any(|change| match &change.op {
            Op::Wrote(before) => scene.transforms.at(change.slot).is_some_and(|record| {
                record.node.parent != before.parent
                    || record.node.scroll != before.scroll
                    || record.node.anchor != before.anchor
            }),
            Op::Removed(_) => Some(change.slot) == start,
            Op::Created(_) => false,
        })
}

/// Reports whether two reference lists refer to the same transform nodes in the same order.
fn same_spaces(a: &[ObjectRef], b: &[ObjectRef]) -> bool {
    spaces(a).eq(spaces(b))
}

/// Returns the slots of the transform nodes in `deps`.
fn spaces(deps: &[ObjectRef]) -> impl Iterator<Item = u32> + '_ {
    deps.iter()
        .filter(|object| object.kind == Kind::Transform)
        .map(|object| object.slot)
}

/// The kept key read against one commit.
struct Ports<'a> {
    scene: &'a Scene,
    journal: &'a Journal,
    key: &'a Key,
    /// Whether a kept port's scroll node applies to each transform node looked up.
    spaces: SmallMap<u32, bool, 8>,
}

impl Ports<'_> {
    /// Reports whether `deps` refers to a transform node a kept port's scroll node applies to.
    fn refers(&mut self, deps: &[ObjectRef]) -> bool {
        spaces(deps).any(|space| self.inside(space))
    }

    /// Reports whether a kept port's scroll node applies to the transform node at `slot`.
    ///
    /// A transform node the commit removed is read from the journal, and so are its removed
    /// ancestors.
    fn inside(&mut self, slot: u32) -> bool {
        if let Some(&inside) = self.spaces.get(&slot) {
            return inside;
        }

        let mut inside = false;
        let mut current = Some(slot);
        let mut steps = 0;
        while let Some(at) = current {
            let Some(node) = self.transform(at) else {
                break;
            };
            if node
                .scroll
                .is_some_and(|id| self.key.ports.contains(&id.slot()))
            {
                inside = true;
                break;
            }
            if matches!(node.anchor, Anchor::Fixed) || steps > self.scene.transforms.span() {
                break;
            }
            current = node.parent.map(|parent| parent.slot());
            steps += 1;
        }

        self.spaces.insert(slot, inside);
        inside
    }

    /// Returns the transform node at `slot`, or the node the commit removed from it.
    fn transform(&self, slot: u32) -> Option<&TransformNode> {
        if let Some(record) = self.scene.transforms.at(slot) {
            return Some(&record.node);
        }
        self.journal
            .transforms
            .iter()
            .find_map(|change| match &change.op {
                Op::Removed(record) if change.slot == slot => Some(&record.node),
                _ => None,
            })
    }

    /// Reports whether the chunk at `key` is the hit's or draws in a kept port now.
    fn draws(&mut self, key: ChunkKey) -> bool {
        if self.key.hit.is_some_and(|(hit, _)| hit == key) {
            return true;
        }

        let scene = self.scene;
        scene
            .chunks
            .get(&key)
            .is_some_and(|record| self.refers(&record.derived.deps))
    }

    /// Reports whether the subtree of `entry` holds the hit's chunk or a chunk that draws in a
    /// kept port. An order node the commit removed is read from the journal.
    fn reaches(&mut self, entry: OrderEntry, depth: usize) -> bool {
        match entry {
            OrderEntry::Chunk(key) => self.draws(key),
            // A tree deeper than every node there is holds a cycle, so the walk stops.
            OrderEntry::Node(_) if depth > self.scene.orders.span() => false,
            OrderEntry::Node(id) => {
                let Some(record) = node(self.scene, self.journal, id.slot()) else {
                    return false;
                };
                record
                    .entries
                    .iter()
                    .any(|&entry| self.reaches(entry, depth + 1))
            }
        }
    }
}

/// Returns the order node at `slot`, or the record the commit of `journal` removed from it.
fn node<'a>(scene: &'a Scene, journal: &'a Journal, slot: u32) -> Option<&'a OrderRecord> {
    scene.orders.at(slot).or_else(|| {
        journal
            .order_nodes
            .iter()
            .find_map(|change| match change.op {
                Op::Removed(index) if change.slot == slot => journal.removed_order(index),
                _ => None,
            })
    })
}
