//! Frame slots: the table indices that instances refer to.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::Hash;

use iceblink::{ChunkKey, ClientId, TableDelta};

/// Frame slots of one kind of object.
///
/// An object keeps its slot for as long as consecutive frames draw it. Slots of objects a frame
/// did not draw are freed when the frame is finished, and the lowest free slot is taken first.
pub(crate) struct Interner<K> {
    slots: HashMap<K, u32>,
    free: BinaryHeap<Reverse<u32>>,
    next: u32,
    used: HashSet<K>,
}

impl<K: Clone + Eq + Hash> Interner<K> {
    pub(crate) fn new() -> Self {
        Self {
            slots: HashMap::new(),
            free: BinaryHeap::new(),
            next: 0,
            used: HashSet::new(),
        }
    }

    /// Returns the slot of `key`, and whether this frame used it for the first time.
    pub(crate) fn slot(&mut self, key: &K) -> (u32, bool) {
        let first = self.used.insert(key.clone());
        if let Some(slot) = self.slots.get(key) {
            return (*slot, first);
        }

        let slot = match self.free.pop() {
            Some(Reverse(slot)) => slot,
            None => {
                self.next += 1;
                self.next - 1
            }
        };
        self.slots.insert(key.clone(), slot);
        (slot, first)
    }

    /// Frees the slots of every key the finished frame did not use.
    pub(crate) fn finish_frame(&mut self) {
        let used = std::mem::take(&mut self.used);
        let free = &mut self.free;
        self.slots.retain(|key, slot| {
            let keep = used.contains(key);
            if !keep {
                free.push(Reverse(*slot));
            }
            keep
        });
    }
}

/// The values of one frame table, gathered while a frame is lowered.
pub(crate) struct TableValues<T> {
    values: Vec<Option<T>>,
    /// The value of every slot that was not set.
    unused: T,
}

impl<T: Clone> TableValues<T> {
    pub(crate) fn new(unused: T) -> Self {
        Self {
            values: Vec::new(),
            unused,
        }
    }

    pub(crate) fn set(&mut self, slot: u32, value: T) {
        let slot = slot as usize;
        if slot >= self.values.len() {
            self.values.resize(slot + 1, None);
        }
        self.values[slot] = Some(value);
    }

    /// Writes the whole table into `delta`.
    pub(crate) fn replace(self, delta: &mut TableDelta<T>) {
        delta.replace = true;
        delta.all.clear();
        let unused = self.unused;
        delta.all.extend(
            self.values
                .into_iter()
                .map(|value| value.unwrap_or_else(|| unused.clone())),
        );
        delta.changed.clear();
    }
}

/// The client transform node a frame space slot stands for.
pub(crate) type SpaceKey = (ClientId, u32);

/// The clip that one link of a frame clip chain stands for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LinkId {
    /// A clip node, by its client and slot.
    Node(ClientId, u32),
    /// A chunk-local clip, by its chunk and index.
    Local(ClientId, ChunkKey, u32),
}

/// The paint a frame paint slot stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PaintKey {
    /// A client's paint slot.
    Slot(ClientId, u32),
    /// An inline colour, as its packed bytes.
    Inline(u32),
}

/// The frame slots of one output, kept from frame to frame.
pub(crate) struct OutputSlots {
    pub(crate) spaces: Interner<SpaceKey>,
    pub(crate) clips: Interner<Vec<LinkId>>,
    pub(crate) paints: Interner<PaintKey>,
    pub(crate) params: Interner<(ClientId, u32)>,
}

impl OutputSlots {
    pub(crate) fn new() -> Self {
        Self {
            spaces: Interner::new(),
            clips: Interner::new(),
            paints: Interner::new(),
            params: Interner::new(),
        }
    }

    pub(crate) fn finish_frame(&mut self) {
        self.spaces.finish_frame();
        self.clips.finish_frame();
        self.paints.finish_frame();
        self.params.finish_frame();
    }
}
