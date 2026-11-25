//! Counting frames of the route histories, and forgetting the histories that no longer decide a
//! route.

use iceblink::{ChunkKey, ClientId};

use crate::content::mask::{MaskCache, PathOwner};

impl MaskCache {
    /// Counts a frame, and forgets the histories whose masks did not differ for eight frames.
    ///
    /// The owners of the frame paths among them are appended to `returning`.
    pub(crate) fn count_frame(&mut self, returning: &mut Vec<PathOwner>) {
        self.frame = self.frame.wrapping_add(1);

        let frame = self.frame;
        let mut waiting = false;
        self.histories.retain(|owner, history| {
            if !history.stale(frame) {
                waiting |= history.declined;
                return true;
            }
            if history.declined {
                returning.push(*owner);
            }
            false
        });
        self.waiting = waiting;
    }

    /// Reports whether a path that became a frame path keeps its history.
    pub(crate) fn waiting(&self) -> bool {
        self.waiting
    }

    /// Forgets the history of the path at `position` of chunk `chunk` of `client`.
    pub(crate) fn forget_path(&mut self, client: ClientId, chunk: ChunkKey, position: u32) {
        self.histories.remove(&PathOwner {
            client,
            chunk,
            position,
        });
    }

    /// Forgets the history of every path of `client`.
    pub(crate) fn forget_client(&mut self, client: ClientId) {
        self.histories.retain(|owner, _| owner.client != client);
    }
}
