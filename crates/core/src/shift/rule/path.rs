//! Where the moving content sits in an output's order tree.

use iceblink::OrderEntry;
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::scene::Scene;

/// Returns the order nodes from an entry's parent up to `root`, or `None` when `root` does not
/// draw the entry.
pub(super) fn ancestors(scene: &Scene, entry: OrderEntry, root: u32) -> Option<SmallVec<[u32; 8]>> {
    let mut path = SmallVec::new();
    let parents = match entry {
        OrderEntry::Chunk(key) => scene.placements.get(&key),
        OrderEntry::Node(id) => scene.node_placements.get(id.slot()),
    };

    let mut current = *parents.first()?;
    loop {
        path.push(current);
        if current == root {
            return Some(path);
        }
        if path.len() > scene.orders.span() {
            return None;
        }
        current = scene.node_placements.first(current)?;
    }
}

/// Returns the index in `path` of the deepest node `path` and `other` share, both ending at the
/// root.
pub(super) fn shared(path: &[u32], other: &[u32]) -> usize {
    let mut index = path.len() - 1;
    let mut other_index = other.len() - 1;
    while index > 0 && other_index > 0 && path[index - 1] == other[other_index - 1] {
        index -= 1;
        other_index -= 1;
    }
    index
}

/// Returns the first and last position in the content node's sequence that hold moving content.
///
/// `paths` holds the ancestors of every moving entry, from its parent up to the root.
pub(super) fn content_range(
    scene: &Scene,
    content: u32,
    moving: &FxHashSet<OrderEntry>,
    paths: &[SmallVec<[u32; 8]>],
) -> Option<(usize, usize)> {
    // The entry of the content node that leads down to each moving entry.
    let mut leading: SmallVec<[u32; 8]> = paths
        .iter()
        .filter_map(|path| {
            let at = path.iter().position(|&node| node == content)?;
            at.checked_sub(1).map(|below| path[below])
        })
        .collect();
    leading.sort_unstable();
    leading.dedup();

    let entries = &scene.orders.at(content)?.entries;
    let mut range: Option<(usize, usize)> = None;
    for (position, entry) in entries.iter().enumerate() {
        let holds = moving.contains(entry)
            || matches!(entry, OrderEntry::Node(id) if leading.binary_search(&id.slot()).is_ok());
        if holds {
            range = Some(range.map_or((position, position), |(low, _)| (low, position)));
        }
    }
    range
}
