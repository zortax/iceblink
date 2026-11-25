//! The net effect of a commit on each changed object.

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::commit::journal::{Change, Op};
use crate::scene::record::Links;
use crate::scene::slab::Slab;
use crate::scene::stored::Stored;

/// The most objects of one kind whose changes are merged by a linear search.
const LINEAR: usize = 16;

/// The most nets room is made for before the changes are read.
const RESERVED: usize = 64;

/// The net changes of one kind of object, of which a small commit makes a few.
pub(crate) type Nets<'a, R> = SmallVec<[Net<'a, R>; 2]>;

/// What one slot held before a commit and what it holds after.
///
/// A commit can remove the object of a slot and create another at the slot. The net then reports
/// both the removal and the create.
pub(crate) struct Net<'a, R: Stored> {
    pub(crate) slot: u32,
    /// The value before the commit, or `None` when the slot held no object before it.
    pub(crate) before: Option<&'a R::Value>,
    /// The bookkeeping from before the commit, or `None` when the slot held no object before it.
    pub(crate) links: Option<&'a Links>,
    /// The last command that removed an object at the slot.
    pub(crate) removed: Option<u32>,
    /// Whether the commit created the object the slot holds after it.
    pub(crate) created: bool,
}

impl<R: Stored> Net<'_, R> {
    /// Reports whether the commit removed an object at the slot and created the object the slot
    /// holds after it.
    pub(crate) fn replaced(&self) -> bool {
        self.removed.is_some() && self.created
    }
}

/// Returns the net change of every object `changes` touch, in the order first touched.
#[inline]
pub(crate) fn nets<'a, R: Stored + 'a>(
    changes: impl IntoIterator<Item = &'a Change<R>>,
    slab: &'a Slab<R>,
) -> Nets<'a, R> {
    let changes = changes.into_iter();
    // Most commits change a few kinds of object, so the others return here.
    if changes.size_hint().1 == Some(0) {
        return Nets::new();
    }
    merged(changes, slab)
}

/// Returns the net change of every object `changes` touch, as [`nets`] does.
fn merged<'a, R: Stored + 'a>(
    changes: impl Iterator<Item = &'a Change<R>>,
    slab: &'a Slab<R>,
) -> Nets<'a, R> {
    let mut index: FxHashMap<u32, usize> = FxHashMap::default();
    let mut nets = Nets::new();

    // Most changes touch an object of their own, so the nets are sized once for all of them.
    let changed = changes.size_hint().1.unwrap_or(0);
    nets.reserve(changed.min(RESERVED));

    for change in changes {
        let found = if nets.len() <= LINEAR {
            nets.iter().position(|net| net.slot == change.slot)
        } else {
            index.get(&change.slot).copied()
        };
        let position = found.unwrap_or_else(|| {
            // The links of a written object are read once every change is seen, since the slab
            // holds a new object at the slot when the commit replaced it.
            let before = match &change.op {
                Op::Created(_) => None,
                Op::Removed(record) => Some(record.value()),
                Op::Wrote(value) => Some(value),
            };
            nets.push(Net {
                slot: change.slot,
                before,
                links: None,
                removed: None,
                created: false,
            });
            // The index takes over from the linear search once the nets outgrow it.
            if nets.len() == LINEAR + 1 {
                index.reserve(changed);
                index.extend(nets.iter().enumerate().map(|(at, net)| (net.slot, at)));
            } else if nets.len() > LINEAR + 1 {
                index.insert(change.slot, nets.len() - 1);
            }
            nets.len() - 1
        });

        let net = &mut nets[position];
        if let Op::Removed(record) = &change.op {
            net.removed = Some(change.command);
            // The first removal takes out the object from before the commit.
            if net.before.is_some() && net.links.is_none() {
                net.links = Some(record.links());
            }
        }
    }

    for net in &mut nets {
        let live = slab.at(net.slot);
        if net.before.is_some() && net.links.is_none() {
            net.links = live.map(Stored::links);
        }
        // A removed object never returns, so an object after a removal is a new one.
        net.created = live.is_some() && (net.before.is_none() || net.removed.is_some());
    }

    nets
}
