//! Keys, and the generations and namespaces they carry.

pub mod generation;
pub mod namespace;

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU64;

pub use crate::arena::key::generation::Generation;
pub use crate::arena::key::namespace::NamespaceId;

/// A generation-checked handle to a value in an arena.
///
/// A key packs a slot index, a [`Generation`] and a [`NamespaceId`] into a non-zero `u64`, so an
/// `Option<Key<T>>` is eight bytes. A key resolves only in an arena of its namespace, and only
/// while its slot holds the value the key was issued for.
///
/// `T` is the type of the value and may be unsized. A key is `Copy`, `Send` and `Sync` for every
/// `T`.
///
/// ```
/// use iceblink_alloc::arena::{ChunkArena, NamespaceId};
///
/// let mut arena: ChunkArena<u32> = ChunkArena::new(NamespaceId::FIRST);
/// let key = arena.insert(7);
/// assert_eq!(key.namespace(), NamespaceId::FIRST);
/// assert_eq!(arena.get(key), Some(&7));
/// ```
#[repr(transparent)]
pub struct Key<T: ?Sized>(NonZeroU64, PhantomData<fn() -> T>);

impl<T: ?Sized> Key<T> {
    /// Packs a slot index, a generation and a namespace into a key.
    pub const fn new(index: u32, generation: Generation, namespace: NamespaceId) -> Self {
        let bits =
            index as u64 | ((generation.get() as u64) << 32) | ((namespace.get() as u64) << 48);
        match NonZeroU64::new(bits) {
            Some(bits) => Self(bits, PhantomData),
            None => panic!("a generation is never zero, so the packed form is never zero"),
        }
    }

    /// Returns the slot index.
    pub const fn index(self) -> u32 {
        self.0.get() as u32
    }

    /// Returns the generation of the occupant the key was issued for.
    pub const fn generation(self) -> Generation {
        match Generation::new(((self.0.get() >> 32) & 0xffff) as u16) {
            Some(generation) => generation,
            None => panic!("a key's generation is never zero"),
        }
    }

    /// Returns the namespace of the arena that issued the key.
    pub const fn namespace(self) -> NamespaceId {
        NamespaceId::new((self.0.get() >> 48) as u16)
    }

    /// Returns the packed form.
    ///
    /// The index takes the low 32 bits, the generation the next 16 and the namespace the top 16.
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }

    /// Unpacks the form [`Key::as_u64`] returns.
    ///
    /// Returns `None` if the generation bits are zero.
    pub const fn from_u64(bits: u64) -> Option<Self> {
        if ((bits >> 32) & 0xffff) == 0 {
            return None;
        }
        match NonZeroU64::new(bits) {
            Some(bits) => Some(Self(bits, PhantomData)),
            None => None,
        }
    }
}

// Derives would require `T: Clone`, `T: Ord` and so on, and `T` need not even be sized.
impl<T: ?Sized> Copy for Key<T> {}

impl<T: ?Sized> Clone for Key<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: ?Sized> Eq for Key<T> {}

impl<T: ?Sized> PartialOrd for Key<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for Key<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: ?Sized> Hash for Key<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: ?Sized> fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Key({}:{}@{})",
            self.index(),
            self.generation().get(),
            self.namespace().get()
        )
    }
}

/// A key that side tables index by its slot.
///
/// Side tables do not check the generation, so every occupant of a slot finds the same entry.
pub trait ArenaKey: Copy + Eq {
    /// Returns the slot index.
    fn index(self) -> u32;

    /// Returns the generation of the slot's occupant.
    fn generation(self) -> Generation;

    /// Returns the namespace of the arena the slot belongs to.
    fn namespace(self) -> NamespaceId;
}

impl<T: ?Sized> ArenaKey for Key<T> {
    fn index(self) -> u32 {
        Key::index(self)
    }

    fn generation(self) -> Generation {
        Key::generation(self)
    }

    fn namespace(self) -> NamespaceId {
        Key::namespace(self)
    }
}
