//! The state a transaction builds before it is checked as a whole.

use std::collections::{HashMap, HashSet};

use iceblink::FontRegistry;
use iceblink::{AnyId, ChunkKey, ClientId, Command, CommitCounters, OrderEntry, OrderId, OutputId};

use crate::naive::commit::{Committed, Context};
use crate::naive::scene::Client;

/// A copy of one client's state with a transaction's commands applied so far.
pub(crate) struct Draft<'a> {
    pub(crate) client_id: ClientId,
    pub(crate) client: Client,
    pub(crate) roots: HashMap<OutputId, (ClientId, OrderId)>,
    pub(crate) outputs: &'a HashSet<OutputId>,
    pub(crate) fonts: &'a dyn FontRegistry,
    versions: &'a mut u64,
    /// The chunks the transaction recorded, with the index of the last command that did.
    pub(crate) recorded: HashMap<ChunkKey, u32>,
    /// The objects the transaction removed, with the index of the command that did.
    pub(crate) removed: Vec<(AnyId, u32)>,
    /// The commands that place entries in the order tree and that shape its parent chains.
    pub(crate) placing: Placing,
    pub(crate) counters: CommitCounters,
}

/// The commands of a transaction that a cycle or an entry in two places is attributed to.
#[derive(Default)]
pub(crate) struct Placing {
    /// The earliest command that created or wrote each transform, by slot.
    pub(crate) transforms: HashMap<u32, u32>,
    /// The earliest command that created or wrote each clip, by slot.
    pub(crate) clips: HashMap<u32, u32>,
    /// Every entry a splice inserted, with the splice's index.
    pub(crate) entries: Vec<(u32, OrderEntry)>,
    /// Every root a `set_root` wrote, with its index.
    pub(crate) roots: Vec<(u32, OrderId)>,
}

impl Placing {
    /// Notes what `command`, the command at `index`, places or writes.
    pub(crate) fn note(&mut self, command: &Command, index: u32, entries: &[OrderEntry]) {
        match command {
            Command::CreateTransform(id, _)
            | Command::SetTransform(id, _)
            | Command::SetTransformNode(id, _) => {
                self.transforms.entry(id.slot()).or_insert(index);
            }
            Command::CreateClip(id, _) | Command::SetClip(id, _) => {
                self.clips.entry(id.slot()).or_insert(index);
            }
            Command::Splice { insert, .. } => {
                let inserted = entries.get(insert.indices()).unwrap_or_default();
                self.entries
                    .extend(inserted.iter().map(|entry| (index, *entry)));
            }
            Command::SetRoot(_, Some(root)) => self.roots.push((index, *root)),
            _ => {}
        }
    }
}

impl<'a> Draft<'a> {
    pub(crate) fn new(
        client_id: ClientId,
        client: Client,
        roots: HashMap<OutputId, (ClientId, OrderId)>,
        context: Context<'a>,
    ) -> Self {
        Self {
            client_id,
            client,
            roots,
            outputs: context.outputs,
            fonts: context.fonts,
            versions: context.versions,
            recorded: HashMap::new(),
            removed: Vec::new(),
            placing: Placing::default(),
            counters: CommitCounters::default(),
        }
    }

    /// Returns a version no image has had.
    pub(crate) fn next_version(&mut self) -> u64 {
        *self.versions += 1;
        *self.versions
    }

    pub(crate) fn into_committed(self) -> Committed {
        Committed {
            client: self.client_id,
            state: self.client,
            roots: self.roots,
            removed: self.removed.into_iter().map(|(id, _)| id).collect(),
            counters: self.counters,
        }
    }
}
