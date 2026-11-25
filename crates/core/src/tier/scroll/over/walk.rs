//! The walk over the paint order that finds the first item of each port's content.

use iceblink::{ChunkKey, OrderEntry, SpaceRef};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::hit::refs;
use crate::scene::Scene;
use crate::scene::content::applied_scrolls;
use crate::tier::View;

/// What a walk over the paint order found.
pub(super) struct Found {
    /// The ports whose content draws no item.
    pub(super) blank: SmallVec<[u32; 4]>,
    /// The space of the first item of the port whose first item comes last after the hit.
    pub(super) front: Option<u32>,
}

/// Walks the paint order under `root` from first to last until the first item of every port in
/// `ports` is found, and returns what it found.
///
/// `hit` is the chunk and item index of the hit, or `None` when there is none.
pub(super) fn walk(
    scene: &Scene,
    view: View<'_>,
    root: u32,
    hit: Option<(ChunkKey, u32)>,
    ports: SmallVec<[u32; 4]>,
) -> Found {
    let mut walk = Forward {
        scene,
        view,
        hit,
        ports,
        chains: FxHashMap::default(),
        front: None,
        passed: hit.is_none(),
        done: false,
    };
    walk.node(root, 0);
    Found {
        blank: walk.ports,
        front: walk.front,
    }
}

/// A walk over the paint order from first to last, until the first item of every port is found.
struct Forward<'a> {
    scene: &'a Scene,
    view: View<'a>,
    /// The chunk and item index of the hit.
    hit: Option<(ChunkKey, u32)>,
    /// The slots of the ports whose first item is not found yet.
    ports: SmallVec<[u32; 4]>,
    /// The slots of the scroll nodes each visited space applies, by slot.
    chains: FxHashMap<u32, SmallVec<[u32; 4]>>,
    /// The space of the first item of the port found last after the hit.
    front: Option<u32>,
    /// Whether the walk passed the hit, or there is no hit.
    passed: bool,
    done: bool,
}

impl Forward<'_> {
    fn node(&mut self, slot: u32, depth: usize) {
        let scene = self.scene;
        let Some(record) = scene.orders.at(slot) else {
            return;
        };
        if depth > scene.orders.span() {
            return;
        }

        for entry in &record.entries {
            match *entry {
                OrderEntry::Chunk(key) => self.chunk(key),
                OrderEntry::Node(id) => self.node(id.slot(), depth + 1),
            }
            if self.done {
                return;
            }
        }
    }

    fn chunk(&mut self, key: ChunkKey) {
        let Some(record) = self.scene.chunks.get(&key) else {
            return;
        };
        let recorded = &record.recorded;
        for (position, &op) in recorded.ops.iter().enumerate() {
            if self.hit == Some((key, position as u32)) {
                self.passed = true;
            }

            let Some((space, _)) = refs(recorded, op) else {
                continue;
            };
            let space = if space == SpaceRef::INHERIT {
                recorded.props.space.slot()
            } else {
                space.0
            };
            let chain = self
                .chains
                .entry(space)
                .or_insert_with(|| applied_scrolls(self.view.transforms, space));
            let before = self.ports.len();
            self.ports.retain(|port| !chain.contains(port));
            if self.ports.len() < before && self.passed {
                self.front = Some(space);
            }
            if self.ports.is_empty() {
                self.done = true;
                return;
            }
        }
    }
}
