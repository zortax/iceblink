//! Successor arenas that continue a namespace.

use crate::arena::chunk::ChunkArena;
use crate::arena::chunk::alloc::Slots;
use crate::arena::key::{Generation, NamespaceId};

/// The next generation of every slot of an arena, for a new arena in the same namespace.
///
/// An arena created with [`ChunkArena::succeeding`] starts each slot at its generation in the
/// succession, so no key of the old arena resolves in the new one.
///
/// ```
/// use iceblink_alloc::arena::{ChunkArena, NamespaceId};
///
/// let mut old: ChunkArena<&str> = ChunkArena::new(NamespaceId::FIRST);
/// let stale = old.insert("closed");
/// let succession = old.succession();
/// drop(old);
///
/// let mut new: ChunkArena<&str> = ChunkArena::succeeding(NamespaceId::FIRST, &succession);
/// let fresh = new.insert("open");
/// assert_eq!(fresh.index(), stale.index());
/// assert_eq!(new.get(stale), None);
/// assert_eq!(new.get(fresh), Some(&"open"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Succession {
    /// The generation of each slot's next occupant, or `None` for a retired slot.
    generations: Box<[Option<Generation>]>,
}

impl Succession {
    /// Returns the number of slots covered.
    pub fn len(&self) -> u32 {
        self.generations.len() as u32
    }

    /// Reports whether no slot is covered.
    pub fn is_empty(&self) -> bool {
        self.generations.is_empty()
    }

    /// Returns the generation of the next occupant of slot `index`.
    ///
    /// Returns [`Generation::FIRST`] for a slot past the end, and `None` for a retired slot.
    pub fn next_generation(&self, index: u32) -> Option<Generation> {
        match self.generations.get(index as usize) {
            Some(generation) => *generation,
            None => Some(Generation::FIRST),
        }
    }

    /// Returns the number of retired slots.
    pub fn retired(&self) -> u32 {
        self.generations
            .iter()
            .filter(|slot| slot.is_none())
            .count() as u32
    }
}

impl<T> ChunkArena<T> {
    /// Returns the generations a successor arena starts its slots at.
    ///
    /// A slot that holds a value advances to its next generation, and retires after
    /// [`Generation::LAST`].
    pub fn succession(&self) -> Succession {
        Succession {
            generations: self.slots.successors().into_boxed_slice(),
        }
    }

    /// Creates an empty arena in `namespace` whose slots start at the generations of
    /// `succession`.
    ///
    /// No key of the arena `succession` came from resolves in the new arena. Retired slots stay
    /// retired, and the lowest free slot is used first.
    pub fn succeeding(namespace: NamespaceId, succession: &Succession) -> Self {
        Self {
            blocks: Vec::new(),
            slots: Slots::succeeding(&succession.generations),
            namespace,
        }
    }
}
