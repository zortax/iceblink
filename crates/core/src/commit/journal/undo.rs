//! Rolling back a commit's changes to primary state.

use iceblink::OrderEntry;

use crate::commit::journal::{
    Change, ChunkChange, Deferred, Journal, Op, OrderChange, OrderNodeChange, Splice, place,
    unplace,
};
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::record::OrderRecord;
use crate::scene::slab::Slab;
use crate::scene::stored::Stored;

impl Journal {
    /// Undoes every change to `scene`, newest first.
    ///
    /// Root writes stay in `roots`, since they changed the outputs.
    #[cold]
    pub(crate) fn roll_back(&mut self, scene: &mut Scene) {
        undo_orders(self, scene);
        undo_chunks(&mut self.chunks, &mut self.replaced, scene);
        undo(&mut self.transforms, &mut scene.transforms);
        undo(&mut self.scrolls, &mut scene.scrolls);
        undo(&mut self.clips, &mut scene.clips);
        undo(&mut self.effects, &mut scene.effects);
        undo(&mut self.paints, &mut scene.paints);
        undo(&mut self.params, &mut scene.params);
        undo(&mut self.images, &mut scene.images);
        undo(&mut self.textures, &mut scene.textures);
        undo(&mut self.shaders, &mut scene.shaders);
        self.deferred = Deferred::default();
    }
}

/// Undoes every change to one slab, newest first.
fn undo<R: Stored>(changes: &mut Vec<Change<R>>, slab: &mut Slab<R>) {
    while let Some(change) = changes.pop() {
        undo_one(change, slab);
    }
}

fn undo_one<R: Stored>(change: Change<R>, slab: &mut Slab<R>) {
    match change.op {
        Op::Created(vacancy) => {
            slab.uncreate(change.slot, vacancy);
        }
        Op::Removed(record) => slab.restore(change.slot, record),
        Op::Wrote(value) => {
            if let Some(record) = slab.at_mut(change.slot) {
                *record.value_mut() = value;
            }
        }
    }
}

/// Undoes a change to an order node, whose removed record is the last of `removed`.
fn undo_order(
    change: OrderNodeChange,
    removed: &mut Vec<OrderRecord>,
    slab: &mut Slab<OrderRecord>,
) {
    match change.op {
        Op::Created(vacancy) => {
            slab.uncreate(change.slot, vacancy);
        }
        Op::Removed(_) => {
            if let Some(record) = removed.pop() {
                slab.restore(change.slot, record);
            }
        }
        Op::Wrote(node) => {
            if let Some(record) = slab.at_mut(change.slot) {
                record.node = node;
            }
        }
    }
}

/// Undoes every change to the chunk store, newest first.
fn undo_chunks(
    changes: &mut Vec<ChunkChange>,
    replaced: &mut [Option<ChunkRecord>],
    scene: &mut Scene,
) {
    while let Some(change) = changes.pop() {
        match change {
            ChunkChange::Replaced { key, before, .. } => match before
                .and_then(|index| replaced.get_mut(index as usize))
                .and_then(Option::take)
            {
                Some(record) => {
                    scene.chunk_bytes += record.derived.counted;
                    if let Some(displaced) = scene.chunks.insert(key, record) {
                        scene.uncount(&displaced);
                    }
                }
                None => {
                    if let Some(removed) = scene.chunks.remove(&key) {
                        scene.uncount(&removed);
                    }
                }
            },
            ChunkChange::Offset { key, before } => {
                if let Some(record) = scene.chunks.get_mut(&key) {
                    record.offset = before;
                }
            }
            ChunkChange::Props { key, before } => {
                if let Some(record) = scene.chunks.get_mut(&key) {
                    record.recorded.props = before;
                }
            }
        }
    }
}

/// Undoes every change to the order tree, newest first.
// Undoing newest first makes each node change and each splice's removed entries the last of
// their lists.
fn undo_orders(journal: &mut Journal, scene: &mut Scene) {
    while let Some(change) = journal.orders.pop() {
        match change {
            OrderChange::Node => {
                if let Some(change) = journal.order_nodes.pop() {
                    undo_order(change, &mut journal.removed_orders, &mut scene.orders);
                }
            }
            OrderChange::Splice(splice) => {
                let start = (splice.removed.start as usize).min(journal.removed_entries.len());
                let removed = journal.removed_entries.split_off(start);
                undo_splice(&splice, removed, scene);
            }
        }
    }
}

/// Undoes one splice, putting back the `removed` entries.
fn undo_splice(splice: &Splice, removed: Vec<OrderEntry>, scene: &mut Scene) {
    let Some(record) = scene.orders.at_mut(splice.node) else {
        return;
    };

    let at = splice.at as usize;
    let inserted: Vec<OrderEntry> = record
        .entries
        .splice(at..at + splice.inserted as usize, removed.iter().copied())
        .collect();

    for entry in inserted {
        unplace(scene, entry, splice.node);
    }
    for entry in removed {
        place(scene, entry, splice.node);
    }
}
