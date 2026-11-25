//! Instances of one chunk, emitted as a block the planner orders with one insertion.

use iceblink::DeviceRect;

use crate::frame::walk::Walk;
use crate::plan::Instance;

/// An instance waiting for the rest of its block.
#[derive(Clone, Copy, Debug)]
pub(super) struct Pending {
    pub(super) instance: Instance,
    /// The pixels the instance makes opaque, for a quad that is an opaque cover.
    pub(super) cover: Option<DeviceRect>,
    /// The instance's order among its chunk's instances, counting from one.
    pub(super) local: u32,
}

impl Walk<'_> {
    /// Holds an instance until the block it belongs to is complete.
    pub(super) fn pend(&mut self, instance: Instance, cover: Option<DeviceRect>, local: u32) {
        self.scratch.pending.push(Pending {
            instance,
            cover,
            local,
        });
    }

    /// Emits the held instances, as a block when there are several.
    ///
    /// The held instances belong to one chunk. Their local orders hold for any subset of the
    /// chunk's instances, so they are moved down to start at one.
    pub(crate) fn flush(&mut self) {
        let scratch = &mut *self.scratch;
        match scratch.pending.len() {
            0 => return,
            1 => emit(&mut scratch.emission, scratch.pending[0], 0),
            _ => {
                let lowest = scratch.pending.iter().map(|pending| pending.local).min();
                let below = lowest.unwrap_or(1).saturating_sub(1);
                scratch.emission.begin_block();
                for pending in &scratch.pending {
                    emit(&mut scratch.emission, *pending, pending.local - below);
                }
                scratch.emission.end_block();
            }
        }

        scratch.pending.clear();
    }
}

/// Pushes one instance with its local order in its block, or zero outside a block.
fn emit(emission: &mut crate::plan::Emission, pending: Pending, local: u32) {
    let instance = pending.instance.at_local(local);
    match pending.cover {
        Some(cover) => emission.push_cover(instance, cover),
        None => emission.push(instance),
    }
}
