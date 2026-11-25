//! Clip chains made by adding an outer chain outside an inner one, and when they are released.

use iceblink::ClipEntry;
use rustc_hash::FxHashMap;

use crate::tables::clip;
use crate::tables::journal::TableVersion;
use crate::tables::table::FrameTable;

/// How many frames in a row a chain no lowering holds may go unasked before it is released.
const GRACE: u64 = 64;

/// The combined chains and what holds them.
///
/// A chain is held by each lowering that draws through it. A chain no lowering holds, such as the
/// clip of a group, is released once `GRACE` frames in a row did not ask for it.
#[derive(Debug, Default)]
pub(crate) struct CombinedChains {
    /// The slot of each chain, by the outer chain's slot and the inner chain's slot.
    slots: FxHashMap<(u32, u32), u32>,
    chains: FxHashMap<u32, Chain>,
    /// The chains in the order they were made, so each is rebuilt after its parts.
    order: Vec<u32>,
    /// The clip table version the chains were last rebuilt at.
    version: TableVersion,
    /// The build serial of the frame being built.
    frame: u64,
    /// The chains handed out in the current frame, in order.
    taken: Vec<u32>,
}

/// One combined chain, by its outer and inner chain, and what holds it.
#[derive(Debug)]
struct Chain {
    outer: u32,
    inner: u32,
    /// How many lowerings draw through the chain.
    holders: u32,
    /// The build serial of the last frame that asked for the chain.
    asked: u64,
}

impl CombinedChains {
    /// Starts the frame of build serial `serial`.
    pub(crate) fn begin(&mut self, serial: u64) {
        self.frame = serial;
    }

    /// Returns the slot of the chain `inner` with `outer` outside it, making it in `clips` when it
    /// does not exist.
    pub(crate) fn combine(
        &mut self,
        clips: &mut FrameTable<ClipEntry>,
        outer: u32,
        inner: u32,
    ) -> u32 {
        let slot = match self.slots.get(&(outer, inner)) {
            Some(&slot) => slot,
            None => {
                let slot = clips.allocate(entry(clips, outer, inner));
                self.slots.insert((outer, inner), slot);
                self.chains.insert(
                    slot,
                    Chain {
                        outer,
                        inner,
                        holders: 0,
                        asked: self.frame,
                    },
                );
                self.order.push(slot);
                slot
            }
        };

        if let Some(chain) = self.chains.get_mut(&slot) {
            chain.asked = self.frame;
        }
        self.taken.push(slot);
        slot
    }

    /// Returns a mark of the chains handed out so far, for [`CombinedChains::hold_taken`].
    pub(crate) fn mark(&self) -> usize {
        self.taken.len()
    }

    /// Holds every chain handed out since `mark`, and `context` when it is a combined chain, once
    /// each, and returns them.
    pub(crate) fn hold_taken(&mut self, mark: usize, context: u32) -> Box<[u32]> {
        if self.taken.len() <= mark && !self.chains.contains_key(&context) {
            return Box::default();
        }

        let mut held: Vec<u32> = self.taken.drain(mark.min(self.taken.len())..).collect();
        held.push(context);
        held.retain(|slot| self.chains.contains_key(slot));
        held.sort_unstable();
        held.dedup();

        for slot in &held {
            if let Some(chain) = self.chains.get_mut(slot) {
                chain.holders += 1;
            }
        }
        held.into_boxed_slice()
    }

    /// Lets go of chains [`CombinedChains::hold_taken`] returned.
    pub(crate) fn let_go(&mut self, held: &[u32]) {
        for slot in held {
            if let Some(chain) = self.chains.get_mut(slot) {
                chain.holders = chain.holders.saturating_sub(1);
            }
        }
    }

    /// Releases every chain no lowering holds and the last `GRACE` frames did not ask for.
    pub(crate) fn sweep(&mut self, clips: &mut FrameTable<ClipEntry>) {
        self.taken.clear();

        let frame = self.frame;
        let chains = &mut self.chains;
        let slots = &mut self.slots;
        self.order.retain(|slot| {
            let Some(chain) = chains.get(slot) else {
                return false;
            };
            if chain.holders > 0 || chain.asked + GRACE > frame {
                return true;
            }
            slots.remove(&(chain.outer, chain.inner));
            chains.remove(slot);
            clips.release(*slot, frame);
            false
        });
    }

    /// Rebuilds every chain whose parts may have changed since the last rebuild.
    pub(crate) fn refresh(&mut self, clips: &mut FrameTable<ClipEntry>) {
        if self.order.is_empty() || self.version == clips.version() {
            return;
        }

        for slot in &self.order {
            let chain = &self.chains[slot];
            let entry = entry(clips, chain.outer, chain.inner);
            clips.write(*slot, entry);
        }
        self.version = clips.version();
    }

    /// Returns how many combined chains exist.
    pub(crate) fn len(&self) -> usize {
        self.chains.len()
    }
}

/// Returns the entry of the chain `inner` with `outer` outside it, from the entries in `clips`.
fn entry(clips: &FrameTable<ClipEntry>, outer: u32, inner: u32) -> ClipEntry {
    let empty = ClipEntry::default();
    let outer = clips.get(outer).unwrap_or(&empty);
    clip::combined(outer, clips.get(inner))
}
