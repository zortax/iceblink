//! The atlas tiles an output's last frame drew, which stay pinned until a frame draws them no
//! more.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::content::TileKey;
use crate::shared_content::Content;

/// The tiles of the lowerings an output's last frame and the frame being built drew, by
/// revision, with the content generation they belong to.
#[derive(Debug, Default)]
pub(crate) struct DrawnPins {
    last: FxHashMap<u64, (u64, Arc<[TileKey]>)>,
    next: FxHashMap<u64, (u64, Arc<[TileKey]>)>,
}

impl DrawnPins {
    /// Pins the tiles of revision `revision` for the frame being built, unless the last frame or
    /// this one already pinned them.
    pub(crate) fn hold(
        &mut self,
        revision: u64,
        epoch: u64,
        tiles: &Arc<[TileKey]>,
        content: &mut Content,
    ) {
        if self.next.contains_key(&revision) {
            return;
        }
        if !self.last.contains_key(&revision) && epoch == content.epoch {
            content.cache.pin(tiles);
        }
        self.next.insert(revision, (epoch, Arc::clone(tiles)));
    }

    /// Ends the frame being built: unpins the tiles the last frame drew and this one did not.
    pub(crate) fn settle(&mut self, content: &mut Content) {
        for (revision, (epoch, tiles)) in self.last.drain() {
            if !self.next.contains_key(&revision) {
                content.unpin(epoch, &tiles);
            }
        }
        core::mem::swap(&mut self.last, &mut self.next);
    }

    /// Unpins every tile the output holds.
    pub(crate) fn release(&mut self, content: &mut Content) {
        // The first settle releases the last frame's pins, the second the current frame's.
        self.settle(content);
        self.settle(content);
    }
}
