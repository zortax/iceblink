//! Scroll offset changes waiting for the next frame, which a shift may answer.

use iceblink::damage::DamageSet;
use iceblink::smallvec::SmallVec;
use iceblink::{ClientId, DeviceRect, OrderEntry};
use rustc_hash::FxHashSet;

use crate::derive::spread::Spread;
use crate::geometry::coverage::Partial;
use crate::scene::Scene;
use crate::scene::travelled::item_inks;

/// How many rectangles a `Moving` holds before it absorbs them into its set.
const HELD: usize = 1024;

/// A scroll node's offset change since the last frame of one output.
#[derive(Debug)]
pub(crate) struct PendingScroll {
    pub(crate) client: ClientId,
    /// The scroll node's slot.
    pub(crate) scroll: u32,
    /// How far the content that moves with the node moved on the device, or `None` when it did
    /// not move rigidly.
    pub(crate) by: Option<[f32; 2]>,
    /// The old and new ink of the content that moves with the node, grown by the spread of the
    /// groups around it.
    pub(crate) moving: Moving,
    /// The old and new ink of the content under the node that does not move with it, grown by the
    /// spread of the groups around it.
    pub(crate) still: DamageSet,
    /// Whether the ink of the moving content lies inside the scroll port before and after, and
    /// every still clip link that cuts it holds the port.
    pub(crate) contained: bool,
    /// Where the still clip links of the moving content cover the scroll port in part.
    pub(crate) partial: Partial,
    /// Whether moving content with ink is drawn inside a group whose filters spread it.
    pub(crate) spreads: bool,
    /// The chunks with ink that moved with the node, and the order nodes it carries that did.
    pub(crate) entries: FxHashSet<OrderEntry>,
    /// The output's damage at the end of each commit that moved the node again.
    pub(crate) late: SmallVec<[Late; 1]>,
}

/// An output's damage at the end of a commit that moved a scroll node again since the last frame.
///
/// Among its rectangles are those measured after the node's earlier moves, which a shift carries
/// by the rest of its move.
#[derive(Debug)]
pub(crate) struct Late {
    pub(crate) damage: DamageSet,
    /// How far the content that moves with the node had moved before the commit.
    pub(crate) before: [f32; 2],
}

/// The damage of moving content, which a frame needs only when it refuses the shift.
///
/// Rectangles are absorbed into a damage set when the damage is needed or once many are held.
/// Content a scroll node carries is held as entries, and its inks are measured only when the
/// damage is needed.
#[derive(Debug)]
pub(crate) struct Moving {
    set: DamageSet,
    held: Vec<DeviceRect>,
    deferred: Option<Deferred>,
}

/// Entries a scroll node carries, whose inks moved from the node's travel `before` to its
/// current travel.
#[derive(Debug)]
struct Deferred {
    scroll: u32,
    before: [i32; 2],
    entries: Vec<(OrderEntry, Spread)>,
}

impl Moving {
    /// Returns empty damage cut to `bound`.
    pub(crate) fn new(bound: DeviceRect) -> Self {
        let mut set = DamageSet::new();
        set.set_bound(Some(bound));
        Self {
            set,
            held: Vec::new(),
            deferred: None,
        }
    }

    /// Adds the damage of `entry`, which scroll node `scroll` carries, at the node's travel
    /// `before` and at its travel when the damage is measured, grown by `spread`.
    ///
    /// The first entry of a scroll node sets `before`, and later entries keep it.
    pub(crate) fn defer(
        &mut self,
        scroll: u32,
        before: [i32; 2],
        entry: OrderEntry,
        spread: Spread,
    ) {
        self.deferred
            .get_or_insert_with(|| Deferred {
                scroll,
                before,
                entries: Vec::new(),
            })
            .entries
            .push((entry, spread));
    }

    /// Measures the deferred entries in `scene`, adds their damage and returns how many item inks
    /// it measured.
    pub(crate) fn measure(&mut self, scene: &Scene) -> u64 {
        let Some(deferred) = self.deferred.take() else {
            return 0;
        };
        let mut measured = 0;
        let after = scene.travel(deferred.scroll);
        let mut inks = Vec::new();

        for (entry, spread) in deferred.entries {
            inks.clear();
            item_inks(scene, entry, deferred.before, &mut inks);
            item_inks(scene, entry, after, &mut inks);
            measured += inks.len() as u64;
            for ink in inks.iter().flatten() {
                self.absorb(spread.grow(*ink));
            }
        }

        measured
    }

    /// Adds `rect` to the damage.
    pub(crate) fn absorb(&mut self, rect: DeviceRect) {
        self.held.push(rect);
        if self.held.len() >= HELD {
            for rect in self.held.drain(..) {
                self.set.absorb(rect);
            }
        }
    }

    /// Adds the damage to `damage`, with the deferred entries measured in `scene`, the scene they
    /// belong to, and returns how many item inks it measured.
    ///
    /// Without that scene, deferred entries make `damage` full.
    pub(crate) fn absorb_into(&mut self, scene: Option<&Scene>, damage: &mut DamageSet) -> u64 {
        let measured = match scene {
            Some(scene) => self.measure(scene),
            None if self.deferred.is_some() => {
                self.deferred = None;
                damage.set_full();
                0
            }
            None => 0,
        };

        damage.absorb_set(&self.set);
        for &rect in &self.held {
            damage.absorb(rect);
        }
        measured
    }
}
