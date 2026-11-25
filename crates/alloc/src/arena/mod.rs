//! Arenas whose values keep their address, and the generation-checked keys that identify them.
//!
//! [`ChunkArena`] stores values in fixed blocks that are never moved, so a reference into the
//! arena stays valid while other values are inserted and removed. A [`Key`] identifies each value
//! by its slot, the [`Generation`] of the slot's occupant and the [`NamespaceId`] of the arena. A
//! key resolves only to the value it was issued for.
//!
//! Removal is deferred. [`ChunkArena::remove`] marks a value removed, and the value stays readable
//! through its key until [`ChunkArena::recycle`] drops it. A slot is reused only after a recycle.
//!
//! Side tables store data per slot and are indexed by the same keys:
//!
//! - [`SlotVec`] is dense, with one entry per slot.
//! - [`PagedVec`] is sparse, with pages of [`PAGE_LEN`] entries allocated on first write.
//!
//! ```
//! use iceblink_alloc::arena::{ChunkArena, NamespaceId, PagedVec, SlotVec};
//!
//! let mut arena: ChunkArena<&str> = ChunkArena::new(NamespaceId::FIRST);
//! let mut depth: SlotVec<_, u32> = SlotVec::for_namespace(arena.namespace());
//! let mut label: PagedVec<_, Option<String>> = PagedVec::for_namespace(arena.namespace());
//!
//! let root = arena.insert("root");
//! let child = arena.insert("child");
//! depth.insert(root, 0);
//! depth.insert(child, 1);
//! *label.get_mut(root) = Some("the root".to_owned());
//!
//! assert_eq!(arena.get(child), Some(&"child"));
//! assert_eq!(depth.get(child), Some(&1));
//! assert_eq!(label.pages(), 1);
//!
//! arena.remove(child);
//! assert_eq!(arena.get(child), Some(&"child"));
//! arena.recycle();
//! assert_eq!(arena.get(child), None);
//! ```

pub mod chunk;
pub mod key;
pub mod paged_vec;
pub mod slot_vec;

pub use crate::arena::chunk::{BLOCK_LEN, ChunkArena, Succession};
pub use crate::arena::key::{ArenaKey, Generation, Key, NamespaceId};
pub use crate::arena::paged_vec::{PAGE_LEN, PagedVec};
pub use crate::arena::slot_vec::SlotVec;
