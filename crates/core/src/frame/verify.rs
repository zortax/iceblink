//! Verification mode's comparison of cached state with a full rebuild.

use iceblink::{ChunkKey, DeviceRect, OrderEntry};
use rustc_hash::FxHashMap;

use crate::derive::carry::carried;
use crate::derive::inks::chunk_inks;
use crate::derive::order::{NodeSum, Summed, exact_ink, group_region, isolates};
use crate::frame::lower::lowered_bytes;
use crate::scene::Scene;
use crate::scene::inks::{ChunkInks, NodeInk};
use crate::scene::rows::Rows;

/// A chunk's inks rebuilt from scratch.
struct Chunk {
    inks: ChunkInks,
    /// The union of the item inks on the device.
    union: Option<DeviceRect>,
}

/// An order node's ink rebuilt from scratch.
#[derive(Clone, Default)]
struct Node {
    summed: Summed,
    /// The union of the subtree's item inks, or the written region when the node isolates.
    exact: Option<DeviceRect>,
}

/// Recomputes every chunk ink and order node ink of `scene` from scratch, and returns how many
/// cached values differ from the rebuilt ones.
///
/// Each of these counts once:
///
/// - A chunk whose device inks differ from those `chunk_inks` computes.
/// - A chunk whose carrying scroll node or carried content differs.
/// - The counted chunk bytes, when they are not the sum of the chunks' bytes.
/// - An order node whose stored ink, watchers or rows differ from those summed from the rebuilt
///   entries, or whose exact ink differs from the union of the rebuilt item inks.
pub(crate) fn mismatches(scene: &Scene) -> u64 {
    let mut found = 0;
    let rebuilt = chunks(scene, &mut found);

    let bytes: u64 = scene.chunks.values().map(lowered_bytes).sum();
    if bytes != scene.chunk_bytes {
        tracing::warn!(
            bytes,
            counted = scene.chunk_bytes,
            "counted chunk bytes differ"
        );
        found += 1;
    }

    found + nodes(scene, &rebuilt)
}

/// Rebuilds the inks of every chunk of `scene`, and adds to `found` how many cached inks and
/// carries differ from the rebuilt ones.
fn chunks(scene: &Scene, found: &mut u64) -> FxHashMap<ChunkKey, Chunk> {
    let mut locals = Vec::new();
    let mut rebuilt: FxHashMap<ChunkKey, Chunk> = FxHashMap::default();
    for (key, record) in &scene.chunks {
        let mut inks = Vec::new();
        let unions = chunk_inks(scene, record, &mut locals, &mut inks);
        let union = unions.ink;
        let cached = scene.device_inks(&record.derived.inks);
        if union != cached.union() || !inks.iter().copied().eq(cached.items()) {
            tracing::warn!(?key, "cached chunk ink differs from a rebuild");
            *found += 1;
        }

        let mut items = Vec::new();
        let fresh = match carried(scene, record, &mut items) {
            Some(carry) => ChunkInks::carried(items, carry),
            None => ChunkInks::on_device(inks, unions.reach),
        };
        if fresh != record.derived.inks {
            tracing::warn!(?key, "cached chunk carry differs from a rebuild");
            *found += 1;
        }

        rebuilt.insert(*key, Chunk { inks: fresh, union });
    }
    rebuilt
}

/// Returns how many order nodes of `scene` hold inks, watchers or rows that differ from those
/// rebuilt from the chunk inks `rebuilt`.
fn nodes(scene: &Scene, rebuilt: &FxHashMap<ChunkKey, Chunk>) -> u64 {
    let mut found = 0;
    let mut nodes = FxHashMap::default();
    scene.orders.each(|slot, _, record| {
        let node = rebuild(scene, rebuilt, slot, &mut nodes, 0);
        let summed = &node.summed;
        if !holds(&record.ink, &summed.ink)
            || summed.watching != record.watching
            || !rows_hold(&record.ink, record.rows.as_ref(), summed.rows.as_ref())
            || node.exact != exact_ink(scene, slot)
        {
            tracing::warn!(slot, "cached order ink differs from a rebuild");
            found += 1;
        }
    });
    found
}

/// Rebuilds the ink of an order node's subtree from the rebuilt chunk inks.
fn rebuild(
    scene: &Scene,
    chunks: &FxHashMap<ChunkKey, Chunk>,
    slot: u32,
    nodes: &mut FxHashMap<u32, Node>,
    depth: usize,
) -> Node {
    if let Some(held) = nodes.get(&slot) {
        return held.clone();
    }

    let Some(record) = scene.orders.at(slot) else {
        return Node::default();
    };
    if depth > scene.orders.span() {
        return Node::default();
    }

    let isolating = isolates(scene, slot);
    let mut sum = NodeSum::default();
    let mut union: Option<DeviceRect> = None;
    for entry in &record.entries {
        let ink = match *entry {
            OrderEntry::Chunk(key) => match chunks.get(&key) {
                Some(chunk) => {
                    sum.add_chunk(scene, &chunk.inks);
                    chunk.union
                }
                None => {
                    sum.skip();
                    None
                }
            },
            OrderEntry::Node(id) => {
                let child = rebuild(scene, chunks, id.slot(), nodes, depth + 1);
                let device = if isolating {
                    child.exact
                } else {
                    scene.node_ink(&child.summed.ink)
                };
                sum.add_node(device, child.summed.ink);
                child.exact
            }
        };
        if let Some(ink) = ink {
            union = Some(union.map_or(ink, |held| held.union(ink)));
        }
    }

    let node = Node {
        summed: sum.finish(scene, slot),
        exact: group_region(scene, slot, union),
    };
    nodes.insert(slot, node.clone());
    node
}

/// Reports whether cached rows serve as the rebuilt ones: equal to them, or, under a wide ink,
/// left out or holding each of them.
fn rows_hold(ink: &NodeInk, cached: Option<&Rows>, rebuilt: Option<&Rows>) -> bool {
    if cached == rebuilt {
        return true;
    }
    match (cached, rebuilt) {
        (None, _) => ink.wide(),
        (Some(cached), Some(rebuilt)) => ink.wide() && cached.holds(rebuilt),
        (Some(_), None) => false,
    }
}

/// Reports whether a cached order ink stands for a rebuilt one: it equals it, or it is marked
/// wide, has the same carry and holds it.
fn holds(cached: &NodeInk, rebuilt: &NodeInk) -> bool {
    if cached == rebuilt {
        return true;
    }

    let holds = match (cached.content(), rebuilt.content()) {
        (Some(cached), Some(rebuilt)) => cached.contains_rect(rebuilt),
        (_, None) => true,
        (None, Some(_)) => false,
    };
    cached.wide() && cached.carry() == rebuilt.carry() && holds
}
