//! The transform nodes whose slight scale writes keep changing, and when they come to rest.

use rustc_hash::FxHashMap;

use crate::content::stretches;
use crate::scene::Scene;

/// How long a transform node keeps moving after the client's latest frame before the write.
pub(crate) const HOLD_NS: u64 = 100_000_000;

/// The moving transform nodes, and when each comes to rest.
///
/// A node moves from the commit that changes the linear part of its world matrix to one that lets
/// glyph tiles stretch, until the first frame presented [`HOLD_NS`] or more after the client's
/// latest frame before that commit.
#[derive(Debug, Default)]
pub(crate) struct Motion {
    /// The latest present time of a frame that drew the client.
    present_ns: u64,
    /// The present time from which each moving node rests, by slot.
    until: FxHashMap<u32, u64>,
}

impl Motion {
    /// Marks the node at `slot` moving for the hold after the client's latest frame.
    pub(crate) fn start(&mut self, slot: u32) {
        self.until
            .insert(slot, self.present_ns.saturating_add(HOLD_NS));
    }

    /// Forgets the motion of the node at `slot`.
    pub(crate) fn stop(&mut self, slot: u32) {
        self.until.remove(&slot);
    }

    /// Advances to a frame presented at `present_ns`, and moves the slots of the nodes that come
    /// to rest at it into `rested`, in no particular order.
    pub(crate) fn advance(&mut self, present_ns: u64, rested: &mut Vec<u32>) {
        self.present_ns = self.present_ns.max(present_ns);
        if self.until.is_empty() {
            return;
        }

        let now = self.present_ns;
        self.until.retain(|&slot, &mut until| {
            let resting = until <= now;
            if resting {
                rested.push(slot);
            }
            !resting
        });
    }
}

/// Returns the present time from which the next moving transform node of `scene` rests, or
/// `None` when none moves.
pub(crate) fn wake(scene: &Scene) -> Option<u64> {
    scene
        .motion
        .until
        .iter()
        .filter(|&(&slot, _)| {
            scene
                .transforms
                .at(slot)
                .is_some_and(|record| record.moving)
        })
        .map(|(_, &until)| until)
        .min()
}

/// Advances the motion of `scene` to a frame presented at `present_ns`, and returns the slots of
/// the nodes that come to rest at it under a world matrix that lets glyph tiles stretch.
pub(crate) fn rest(scene: &mut Scene, present_ns: u64) -> Vec<u32> {
    let mut rested = Vec::new();
    scene.motion.advance(present_ns, &mut rested);

    rested.retain(|&slot| {
        let Some(record) = scene.transforms.at_mut(slot) else {
            return false;
        };
        let stretched = record.moving && stretches(&record.world);
        record.moving = false;
        stretched
    });
    rested
}
