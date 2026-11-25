//! Client-side minting of ids.

mod slots;

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use self::slots::Slots;
use crate::id::kind::KIND_COUNT;
use crate::id::{AnyId, ClientId, Id, IdKind};

/// The ids of one client, minted without a round trip to the compositor.
///
/// Clones share one set of tables, so the client's transactions and the compositor mint and free
/// ids of the same client from any thread. Every kind of id follows these rules:
///
/// - A minted id takes the lowest free index of its kind, or one past the highest index ever
///   minted when none is free.
/// - The first generation of an index is 1.
/// - A freed index is minted again with its generation increased by one.
/// - An index whose generation is `u32::MAX` is retired once freed, and never minted again.
#[derive(Clone)]
pub struct IdSpace(Arc<IdAllocator>);

/// The tables an `IdSpace` shares between its clones.
struct IdAllocator {
    client: ClientId,
    kinds: [Mutex<Slots>; KIND_COUNT],
}

impl IdSpace {
    /// Creates an empty id space for `client`.
    pub fn new(client: ClientId) -> Self {
        Self(Arc::new(IdAllocator {
            client,
            kinds: Default::default(),
        }))
    }

    /// Returns the client the ids belong to.
    pub fn client(&self) -> ClientId {
        self.0.client
    }

    /// Mints a fresh id of kind `T`.
    ///
    /// # Panics
    ///
    /// Panics if every index of kind `T` below 2³¹ is live or retired.
    pub fn mint<T: IdKind>(&self) -> Id<T> {
        let (index, generation) = self.kind(T::KIND).mint();
        Id::new(index, generation)
    }

    /// Frees ids so their indices can be minted again with the next generation.
    ///
    /// An id whose generation is not the current one of its index is ignored, so freeing an id
    /// twice frees it once. An id at generation `u32::MAX` retires its index.
    pub fn free(&self, ids: &[AnyId]) {
        for id in ids {
            let (index, generation) = id.parts();
            self.kind(id.kind()).free(index, generation);
        }
    }

    /// Reports whether `self` and `other` are clones of one id space.
    pub fn same(&self, other: &IdSpace) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn kind(&self, kind: usize) -> MutexGuard<'_, Slots> {
        // The tables hold no invariant a panic between two writes could break.
        self.0.kinds[kind]
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl fmt::Debug for IdSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IdSpace").field(&self.0.client).finish()
    }
}
